/*
 * Epidemic Simulation Using Census Data (ESUCD)
 * Copyright (c)  2022. Sam Ralph
 *
 * This file is part of ESUCD.
 *
 * ESUCD is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 3 of the License.
 *
 * ESUCD is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with ESUCD.  If not, see <https://www.gnu.org/licenses/>.
 *
 */

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{LineWriter, Write};
use std::time::Instant;

use anyhow::{Context, Result};
use log::{debug, error, info};
use rand::prelude::{IteratorRandom, SliceRandom};
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use load_census_data::CensusData;
use load_census_data::osm_parsing::{OSMRawBuildings, RawBuilding, TagClassifiedBuilding};
use load_census_data::parsing_error::{DataLoadingError, ParseErrorType};
use load_census_data::polygon_lookup::PolygonContainer;
use load_census_data::tables::occupation_count::OccupationType;

use crate::config::{
    DEBUG_ITERATION_PRINT, get_memory_usage, STARTING_INFECTED_COUNT};
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::disease::DiseaseStatus::Infected;
use crate::error::SimError;
use crate::interventions::{InterventionsEnabled, InterventionStatus};
use crate::models::building::{Building, BuildingID, BuildingType, Workplace};
use crate::models::citizen::{Citizen, CitizenID};
use crate::models::ID;
use crate::models::output_area::{OutputArea, OutputAreaID};
use crate::models::public_transport_route::{PublicTransport, PublicTransportID};
use crate::statistics::Statistics;

struct Timer {
    function_timer: Instant,
    code_block_timer: Instant,
}

impl Timer {
    #[inline]
    pub fn code_block_finished(&mut self, message: &str) -> anyhow::Result<()> {
        info!(
            "{} in {:.2} seconds. Total function time: {:.2} seconds, Memory usage: {}",
            message,
            self.code_block_timer.elapsed().as_secs_f64(),
            self.function_timer.elapsed().as_secs_f64(),
            get_memory_usage()?
        );
        self.code_block_timer = Instant::now();
        Ok(())
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            function_timer: Instant::now(),
            code_block_timer: Instant::now(),
        }
    }
}

/// On csgpu2 with 20? threads took 11 seconds as oppose to 57 seconds for single threaded version
fn parallel_assign_buildings_to_output_areas(building_locations: HashMap<TagClassifiedBuilding, Vec<RawBuilding>>, output_area_lookup: PolygonContainer<String>) -> HashMap<OutputAreaID, HashMap<TagClassifiedBuilding, Vec<RawBuilding>>> {
    building_locations.into_par_iter().map(|(building_type, possible_building_locations)|
        {
            // Try find Area Codes for the given building
            let area_codes = possible_building_locations.into_par_iter().filter_map(|building| {
                if let Ok(area_code) = output_area_lookup.find_polygon_for_point(&building.center()) {
                    let area_id = OutputAreaID::from_code(area_code.to_string());
                    return Some((area_id, vec![building]));
                }
                None
            }
                                                                                    // Group By Area Code
            ).fold(HashMap::new, |mut a: HashMap<OutputAreaID, Vec<RawBuilding>>, b| {
                let area_entry = a.entry(b.0.clone()).or_default();
                area_entry.extend(b.1);
                a
                // Combine into single hashmap
            }).reduce(HashMap::new, |mut a, b| {
                for (area, area_buildings) in b {
                    let area_entry = a.entry(area).or_default();
                    area_entry.extend(area_buildings)
                }
                a
            });
            (building_type, area_codes)
        }).
        // Group buildings per area, by Classification code
        fold(HashMap::new, |mut a: HashMap<
            OutputAreaID,
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>>, b| {
            for (area_code, buildings) in b.1 {
                let area_entry = a.entry(area_code).or_default();
                let class_entry = area_entry.entry(b.0).or_default();
                class_entry.extend(buildings);
            }
            a
        }).
        // Reduce to a single hashmap
        reduce(HashMap::new, |mut a, b| {
            for (area, classed_buildings) in b {
                let area_entry = a.entry(area).or_default();
                for (class, buildings) in classed_buildings {
                    let class_entry = area_entry.entry(class).or_default();
                    class_entry.extend(buildings);
                }
            }
            a
        })
}

//#[derive(Clone)]
pub struct Simulator {
    /// The total size of the population
    current_population: u32,
    /// A list of all the sub areas containing agents
    pub output_areas: HashMap<OutputAreaID, OutputArea>,
    /// The list of citizens who have a "home" in this area
    pub citizens: HashMap<CitizenID, Citizen>,
    pub citizens_eligible_for_vaccine: Option<HashSet<CitizenID>>,
    pub statistics: Statistics,
    interventions: InterventionStatus,
    disease_model: DiseaseModel,
    pub public_transport: HashMap<PublicTransportID, PublicTransport>,
    rng: ThreadRng,
}

/// Initialisation Methods
impl Simulator {
    pub fn new(census_data: CensusData, osm_data: OSMRawBuildings, output_areas_polygons: PolygonContainer<String>) -> Result<Simulator> {
        let mut timer = Timer::default();
        let mut rng = thread_rng();
        let disease_model = DiseaseModel::covid();
        let mut output_areas: HashMap<OutputAreaID, OutputArea> = HashMap::new();


        timer.code_block_finished("Loaded output map polygons")?;
        let mut starting_population = 0;

        let mut citizens = HashMap::new();
        // Build the initial Output Areas and Households
        for entry in census_data.values() {
            let output_id = OutputAreaID::from_code(entry.output_area_code.to_string());
            // TODO Remove polygons from grid
            let polygon = output_areas_polygons.polygons.get(output_id.code())
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Building output areas map".to_string(),
                        key: output_id.to_string(),
                    },
                })
                .context(format!("Loading polygon shape for area: {}", output_id))?;
            starting_population += entry.total_population_size() as u32;
            let new_area = OutputArea::new(output_id, polygon.clone(), disease_model.mask_percentage)
                .context("Failed to create Output Area")?;
            output_areas.insert(new_area.output_area_id.clone(), new_area);
        }
        timer.code_block_finished("Built Output Areas")?;


        debug!("Attempting to allocating {} possible buildings to {} Output Areas",osm_data.building_locations.iter().map(|(_k,v)|v.len()).sum::<usize>(),census_data.valid_areas.len());
        // Assign possible buildings to output areas
        let mut possible_buildings_per_area: HashMap<
            OutputAreaID,
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
        > = parallel_assign_buildings_to_output_areas(osm_data.building_locations, output_areas_polygons);
        let count: usize = possible_buildings_per_area.par_iter().map(|(_, classed_building)| classed_building.par_iter().map(|(_, buildings)| buildings.len()).sum::<usize>()).sum();

        timer.code_block_finished("Assigned Possible Buildings to Output Areas")?;
        output_areas.retain(|code, _area| possible_buildings_per_area.contains_key(code));
        debug!("{} Buildings have been assigned. {} Output Areas remaining (with buildings)",count,output_areas.len());

        // Generate Citizens
        let mut failed_output_areas = Vec::new();
        for (output_area_id, output_area) in output_areas.iter_mut() {
            let generate_citizen_closure = || -> anyhow::Result<()> {
                // Retrieve the Census Data
                let census_data = census_data
                    .for_output_area_code(output_area_id.code().to_string())
                    .ok_or_else(|| SimError::InitializationError {
                        message: format!(
                            "Cannot generate Citizens for Output Area {} as Census Data exists",
                            output_area_id
                        ),
                    })?;
                // Extract the possible buildings for this Output Area
                let possible_buildings = possible_buildings_per_area
                    .get_mut(output_area_id)
                    .ok_or_else(|| SimError::InitializationError {
                        message: format!(
                            "Cannot generate Citizens for Output Area {} as no buildings exist",
                            output_area_id
                        ),
                    })?;
                // Retrieve the Households for this Output Area
                let possible_households = possible_buildings
                    .remove(&TagClassifiedBuilding::Household)
                    .ok_or_else(|| SimError::InitializationError {
                        message: format!(
                            "Cannot generate Citizens for Output Area {} as no households exist",
                            output_area_id
                        ),
                    })?;
                citizens.extend(output_area.generate_citizens_with_households(
                    &mut rng,
                    census_data,
                    possible_households,
                )?);
                Ok(())
            }();
            if generate_citizen_closure.is_err() {
                failed_output_areas.push(output_area_id.clone());
                //error!("Failed to generate Citizens for area: {}        Error: {}",output_area_id, e);
            }
        }
        error!("Failed to generate Households and Citizens for {} Output Areas", failed_output_areas.len());
        for failed_area in failed_output_areas {
            if output_areas.remove(&failed_area).is_none() {
                error!("Failed to remove Output Area {}, which has no Citizens",failed_area);
            }
            if possible_buildings_per_area.remove(&failed_area).is_none() {
                error!("Failed to remove possible buildings for Output Area {}, which has no Citizens",failed_area);
            }
        }
        timer.code_block_finished("Generated Citizens and residences")?;

        // TODO Need a way of finding the closest building of type X to a point?


        // Build the workplaces
        // TODO Currently any buildings remaining are treated as Workplaces
        let possible_workplaces: HashMap<OutputAreaID, Vec<RawBuilding>> = possible_buildings_per_area.drain().filter_map(|(area, mut classified_buildings)| {
            let buildings: Vec<RawBuilding> = classified_buildings.drain().flat_map(|(_, a)| a).collect();
            if buildings.is_empty() {
                return None;
            }
            Some((area, buildings))
        }).collect();

        debug!("There are {} areas with workplace buildings",possible_workplaces.len());
        // Remove any areas that do not have any workplaces
        output_areas.retain(|code, data| {
            if !possible_workplaces.contains_key(code) {
                data.get_residents().iter().for_each(|id|
                    if citizens.remove(id).is_none() {
                        error!("Failed to remove citizen: {}",id);
                    });

                false
            } else {
                true
            }
        });
        let mut simulator = Simulator {
            current_population: starting_population,
            output_areas,
            citizens,
            citizens_eligible_for_vaccine: None,
            statistics: Statistics::default(),
            interventions: Default::default(),
            disease_model,
            public_transport: Default::default(),
            rng: thread_rng(),
        };
        info!("Starting to build workplaces");
        simulator
            .build_workplaces(census_data, possible_workplaces)
            .context("Failed to build workplaces")?;
        timer.code_block_finished("Generated workplaces")?;

        // Infect random citizens
        simulator
            .apply_initial_infections()
            .context("Failed to create initial infections")?;

        timer.code_block_finished("Initialization completed")?;
        debug!(
            "Starting Statistics:\n      There are {} total Citizens\n      {} Output Areas",
            simulator.citizens.len(),
            simulator.output_areas.len()
        );
        assert_eq!(
            simulator.citizens.len() as u32,
            simulator
                .output_areas
                .iter()
                .map(|area| area.1.total_residents)
                .sum::<u32>()
        );
        Ok(simulator)
    }

    /// Iterates through all Output Areas, and All Citizens in that Output Area
    ///
    /// Picks a Workplace Output Area, determined from Census Data Distribution
    ///
    /// Allocates that Citizen to the Workplace Building in that chosen Output Area
    pub fn build_workplaces(
        &mut self,
        census_data: CensusData,
        mut possible_buildings_per_area: HashMap<
            OutputAreaID, Vec<RawBuilding>>,
    ) -> anyhow::Result<()> {
        let areas: Vec<OutputAreaID> = self.output_areas.keys().cloned().collect();
        debug!("Assigning workplaces to {} output areas ",self.output_areas.len());
        // Add Workplace Output Areas to Every Citizen
        let mut citizens_to_allocate: HashMap<
            OutputAreaID,
            (Vec<CitizenID>, Vec<RawBuilding>),
        > = HashMap::new();
        let mut failed_output_areas = Vec::new();
        // Assign workplace areas to each Citizen, per Output area
        for (household_output_area_code, household_output_area) in &self.output_areas {

            // Retrieve the census data for the household output area
            let household_census_data = census_data
                .for_output_area_code(household_output_area_code.code().to_string())
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Cannot retrieve Census Data for output area ".to_string(),
                        key: household_output_area_code.to_string(),
                    },
                })?;

            for citizen_id in household_output_area.get_residents() {
                // Generate a workplace Output Area, and ensure it exists!
                let mut attempt_index = 0;
                let mut workplace_output_area_code = OutputAreaID::from_code("".to_string());
                while !(possible_buildings_per_area.contains_key(&workplace_output_area_code) && self.output_areas.contains_key(&workplace_output_area_code)) {
                    workplace_output_area_code = OutputAreaID::from_code(
                        household_census_data
                            .get_random_workplace_area(&mut self.rng)
                            .context("Selecting a random workplace")?,
                    );
                    attempt_index += 1;
                    if attempt_index == 10 {
                        workplace_output_area_code = OutputAreaID::from_code("".to_string());
                        break;
                    }
                }
                if workplace_output_area_code == OutputAreaID::from_code("".to_string()) {
                    //error!("Failed to find workplace area for household area {:?} ",household_output_area_code);
                    failed_output_areas.push(household_output_area_code.clone());
                    break;
                }
                // Initialise the workplace area if it doesn't exist
                if !citizens_to_allocate.contains_key(&workplace_output_area_code) {
                    let possible_workplaces = possible_buildings_per_area
                        .remove(&workplace_output_area_code)
                        .ok_or_else(|| SimError::InitializationError {
                            message: format!(
                                "Cannot generate Citizens for Output Area {} as no buildings exist",
                                workplace_output_area_code
                            ),
                        })?;
                    citizens_to_allocate.insert(
                        workplace_output_area_code.clone(),
                        (Vec::new(), possible_workplaces),
                    );
                }
                citizens_to_allocate
                    .get_mut(&workplace_output_area_code)
                    .ok_or_else(|| DataLoadingError::ValueParsingError {
                        source: ParseErrorType::MissingKey {
                            context: "Cannot retrieve Output Area to add Citizens to  ".to_string(),
                            key: workplace_output_area_code.to_string(),
                        },
                    })?
                    .0
                    .push(citizen_id);
            }
        }
        error!("Failed to find workplace area for {} household areas",failed_output_areas.len());
        /*        for failed_area in failed_output_areas {
                    if self.output_areas.remove(&failed_area).is_none() {
                        error!("Failed to remove Output Area {}, which has no Workplace Buildings",failed_area);
                    }
                }*/
        debug!("Creating workplace buildings");
        // Create buildings for each Workplace output area
        for (workplace_area_code, mut to_allocate) in citizens_to_allocate {
            // Randomise the order of the citizens, to reduce the number of Citizens sharing household and Workplace output areas
            to_allocate.0.shuffle(&mut self.rng);
            // TODO Check buildings are shuffled
            let mut possible_buildings = to_allocate.1.iter();
            let total_building_count = possible_buildings.len();
            let total_workers = to_allocate.0.len();

            // This is the Workplace list to allocate citizens to
            let mut current_workplaces_to_allocate: HashMap<OccupationType, Workplace> =
                HashMap::new();

            // This is the list of full workplaces that need to be added to the parent Output Area
            let mut workplace_buildings: HashMap<BuildingID, Box<dyn Building>> = HashMap::new();
            for (index, citizen_id) in to_allocate.0.iter().enumerate() {
                let citizen = self.citizens.get_mut(citizen_id).ok_or_else(|| {
                    DataLoadingError::ValueParsingError {
                        source: ParseErrorType::MissingKey {
                            context: "Cannot retrieve Citizen to assign Workplace ".to_string(),
                            key: citizen_id.to_string(),
                        },
                    }
                })?;

                // 3 Cases
                // Work place exists and Citizen can be added:
                //      Add Citizen to it
                // Work place exists and Citizen cannot be added:
                //      Save the current Workplace
                //      Generate a new Workplace
                //      Add a Citizen to the new Workplace
                // Work place doesn't exist
                //      Generate a new Workplace
                //      Add a Citizen to the new Workplace
                // Else
                let workplace = current_workplaces_to_allocate.remove(&citizen.occupation());
                let workplace = match workplace {
                    Some(mut workplace) => {
                        match workplace.add_citizen(*citizen_id) {
                            Ok(_) => workplace,
                            Err(_) => {
                                workplace_buildings
                                    .insert(workplace.id().clone(), Box::new(workplace));
                                // TODO Have better distribution of AreaClassification?
                                let mut workplace = Workplace::new(
                                    BuildingID::new(
                                        workplace_area_code.clone(),
                                        BuildingType::Workplace,
                                    ),
                                    *possible_buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces{} to assign workers{}/{} to in Output Area: {}", total_building_count, index, total_workers, workplace_area_code) })?,
                                    citizen.occupation());
                                workplace.add_citizen(*citizen_id).context(
                                    "Cannot add Citizen to freshly generated Workplace!",
                                )?;
                                workplace
                            }
                        }
                    }
                    None => {
                        let mut workplace = Workplace::new(
                            BuildingID::new(
                                workplace_area_code.clone(),
                                BuildingType::Workplace,
                            ),
                            *possible_buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces{} to assign workers{}/{} to in Output Area: {}", total_building_count, index, total_workers, workplace_area_code) })?,
                            citizen.occupation(),
                        );
                        workplace.add_citizen(*citizen_id).context("Cannot add Citizen to new workplace!")?;
                        workplace
                    }
                };
                citizen.set_workplace_code(workplace.id().clone());
                // Add the unfilled Workplace back to the allocator
                current_workplaces_to_allocate.insert(citizen.occupation(), workplace);
            }
            let workplace_output_area = self
                .output_areas
                .get_mut(&workplace_area_code)
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Retrieving output area for building workplaces ".to_string(),
                        key: workplace_area_code.to_string(),
                    },
                })?;
            // Add any leftover Workplaces to the Output Area
            current_workplaces_to_allocate
                .drain()
                .for_each(|(_, workplace)| {
                    workplace_buildings.insert(workplace.id().clone(), Box::new(workplace));
                });
            workplace_output_area.buildings.extend(workplace_buildings);
        }
        Ok(())
    }

    pub fn apply_initial_infections(&mut self) -> anyhow::Result<()> {
        for _ in 0..STARTING_INFECTED_COUNT {
            let citizen = self
                .citizens
                .values_mut()
                .choose(&mut self.rng)
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::IsEmpty {
                        message: "No citizens exist in the output areas for seeding the disease"
                            .to_string(),
                    },
                })
                .context("Initialisation of disease!")?;
            citizen.disease_status = DiseaseStatus::Infected(0);
        }
        Ok(())
    }
}

/// Runtime Simulation Methods
impl Simulator {
    pub fn simulate(&mut self) -> anyhow::Result<()> {
        let mut start_time = Instant::now();
        info!("Starting simulation...");
        for time_step in 0..self.disease_model.max_time_step {
            if time_step % DEBUG_ITERATION_PRINT as u16 == 0 {
                println!("Completed {: >3} time steps, in: {: >6} seconds  Statistics: {},   Memory usage: {}", DEBUG_ITERATION_PRINT, format!("{:.2}", start_time.elapsed().as_secs_f64()), self.statistics, get_memory_usage()?);
                start_time = Instant::now();
            }
            if !self.step()? {
                debug!("{}", self.statistics);
                break;
            }
        }
        Ok(())
    }
    /// Applies a single time step to the simulation
    ///
    /// Returns False if it has finished
    pub fn step(&mut self) -> anyhow::Result<bool> {
        let mut start = Instant::now();
        // Reset public transport containers
        self.public_transport = Default::default();
        self.generate_and_apply_exposures()?;

        let exposure_time = start.elapsed().as_secs_f64();
        start = Instant::now();

        self.apply_interventions()?;

        let intervention_time = start.elapsed().as_secs_f64();
        let total = exposure_time + intervention_time;
        debug!("Generate Exposures: {:.3} seconds ({:.3}%), Apply Interventions: {:.3} seconds ({:.3}%)",exposure_time,exposure_time/total,intervention_time,intervention_time/total);
        if !self.statistics.disease_exists() {
            info!("Disease finished as no one has the disease");
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn generate_and_apply_exposures(&mut self) -> anyhow::Result<()> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        let mut building_exposure_list: HashMap<BuildingID, usize> = HashMap::new();
        self.statistics.next();

        // The list of Citizens on Public Transport, grouped by their origin and destination
        let mut public_transport_pre_generate: HashMap<
            (OutputAreaID, OutputAreaID),
            Vec<(CitizenID, bool)>,
        > = HashMap::new();

        // Generate exposures for fixed building positions
        for citizen in self.citizens.values_mut() {
            citizen.execute_time_step(
                self.statistics.time_step(),
                &self.disease_model,
                self.interventions.lockdown_enabled(),
            );
            self.statistics.add_citizen(&citizen.disease_status);

            // Either generate public transport session, or add exposure for fixed building position
            if let Some(travel) = &citizen.on_public_transport {
                let transport_session = public_transport_pre_generate
                    .entry(travel.clone())
                    .or_default();

                transport_session.push((citizen.id(), citizen.is_infected()));
            } else if let Infected(_) = citizen.disease_status {
                let entry = building_exposure_list
                    .entry(citizen.current_building_position.clone())
                    .or_insert(1);
                *entry += 1;
            }
        }

        // Apply Building Exposures
        for (building_id, exposure_count) in building_exposure_list {
            let area = self.output_areas.get(&building_id.output_area_code());
            match area {
                Some(area) => {
                    // TODO Sometime there's a weird bug here?
                    let building = &area.buildings.get(&building_id).context(format!(
                        "Failed to retrieve exposure building {}",
                        building_id
                    ))?;
                    let building = building.as_ref();
                    let occupants = building.occupants().clone();
                    self.expose_citizens(
                        occupants,
                        exposure_count,
                        ID::Building(building_id.clone()),
                    )?;
                }

                None => {
                    error!(
                        "Cannot find output area {}, that had an exposure occurred in!",
                        &building_id.output_area_code()
                    );
                }
            }
        }
        // Generate public transport routes
        for (route, mut citizens) in public_transport_pre_generate {
            citizens.shuffle(&mut self.rng);
            let mut current_bus = PublicTransport::new(route.0.clone(), route.1.clone());
            while let Some((citizen, is_infected)) = citizens.pop() {
                // If bus is full, generate a new one
                if current_bus.add_citizen(citizen).is_err() {
                    // Only need to save buses with exposures
                    if current_bus.exposure_count > 0 {
                        self.expose_citizens(
                            current_bus.occupants().clone(),
                            current_bus.exposure_count,
                            ID::PublicTransport(current_bus.id().clone()),
                        )?;
                    }
                    current_bus = PublicTransport::new(route.0.clone(), route.1.clone());
                    current_bus
                        .add_citizen(citizen)
                        .context("Failed to add Citizen to new bus")?;
                }
                if is_infected {
                    current_bus.exposure_count += 1;
                }
            }
            if current_bus.exposure_count > 0 {
                self.expose_citizens(
                    current_bus.occupants().clone(),
                    current_bus.exposure_count,
                    ID::PublicTransport(current_bus.id().clone()),
                )?;
            }
        }
        // Apply Public Transport Exposures
        //debug!("There are {} exposures", exposure_list.len());
        Ok(())
    }

    fn expose_citizens(
        &mut self,
        citizens: Vec<CitizenID>,
        exposure_count: usize,
        location: ID,
    ) -> anyhow::Result<()> {
        for citizen_id in citizens {
            let citizen = self.citizens.get_mut(&citizen_id);
            match citizen {
                Some(citizen) => {
                    if citizen.is_susceptible()
                        && citizen.expose(
                        exposure_count,
                        &self.disease_model,
                        &self.interventions.mask_status,
                        &mut self.rng,
                    )
                    {
                        self.statistics
                            .citizen_exposed(location.clone())
                            .context(format!("Exposing citizen {}", citizen_id))?;

                        if let Some(vaccine_list) = &mut self.citizens_eligible_for_vaccine {
                            vaccine_list.remove(&citizen_id);
                        }
                    }
                }
                None => {
                    error!("Citizen {}, does not exist!", citizen_id);
                }
            }
        }
        Ok(())
    }
    fn apply_interventions(&mut self) -> anyhow::Result<()> {
        let infected_percent = self.statistics.infected_percentage();
        //debug!("Infected percent: {}",infected_percent);
        let new_interventions = self.interventions.update_status(infected_percent);
        for intervention in new_interventions {
            match intervention {
                InterventionsEnabled::Lockdown => {
                    info!(
                        "Lockdown is enabled at hour {}",
                        self.statistics.time_step()
                    );
                    // Send every Citizen home
                    for mut citizen in &mut self.citizens {
                        let home = citizen.1.household_code.clone();
                        citizen.1.current_building_position = home;
                    }
                }
                InterventionsEnabled::Vaccination => {
                    info!(
                        "Starting vaccination program at hour: {}",
                        self.statistics.time_step()
                    );
                    let mut eligible = HashSet::new();
                    self.citizens.iter().for_each(|(id, citizen)| {
                        if citizen.disease_status == DiseaseStatus::Susceptible {
                            eligible.insert(*id);
                        }
                    });
                    self.citizens_eligible_for_vaccine = Some(eligible);
                }
                InterventionsEnabled::MaskWearing(status) => {
                    info!(
                        "Mask wearing status has changed: {} at hour {}",
                        status,
                        self.statistics.time_step()
                    )
                }
            }
        }
        if let Some(citizens) = &mut self.citizens_eligible_for_vaccine {
            let chosen: Vec<CitizenID> = citizens
                .iter()
                .choose_multiple(&mut self.rng, self.disease_model.vaccination_rate as usize)
                .iter()
                .map(|id| **id)
                .collect();
            for citizen_id in chosen {
                citizens.remove(&citizen_id);
                let citizen = self
                    .citizens
                    .get_mut(&citizen_id)
                    .context("Citizen '{}' due to be vaccinated, doesn't exist!")?;
                citizen.disease_status = DiseaseStatus::Vaccinated;
            }
        }

        Ok(())
    }
}

impl Simulator {
    pub fn error_dump(self) -> anyhow::Result<()> {
        println!("Creating Core Dump!");
        let file = File::create("crash.dump")?;
        let mut file = LineWriter::new(file);
        writeln!(file, "{}", self.statistics)?;
        for area in self.output_areas {
            writeln!(file, "Output Area: {}", area.0)?;
            for building in area.1.buildings.values() {
                writeln!(file, "      {}", building)?;
            }
        }
        writeln!(file, "\n\n\n----------\n\n\n")?;
        for citizen in self.citizens {
            writeln!(file, "    {}", citizen.1)?;
        }
        Ok(())
    }
    pub fn error_dump_json(self) -> anyhow::Result<()> {
        println!("Creating Core Dump!");
        let mut file = File::create("crash.json")?;
        use serde_json::json;

        let mut output_area_json = HashMap::new();
        for area in self.output_areas {
            output_area_json.insert(area.0, area.1.buildings);
        }
        let mut citizens = HashMap::new();
        for citizen in self.citizens {
            citizens.insert(citizen.0.to_string(), citizen.1);
        }
        file.write_all(
            json!({"citizens":citizens,"output_areas":output_area_json})
                .to_string()
                .as_ref(),
        )?;

        Ok(())
    }
}
