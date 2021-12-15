/*
 * Epidemic Simulation Using Census Data (ESUCD)
 * Copyright (c)  2021. Sam Ralph
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
use uuid::Uuid;

use load_census_data::CensusData;
use load_census_data::parsing_error::{DataLoadingError, ParseErrorType};
use load_census_data::tables::CensusTableNames;
use load_census_data::tables::occupation_count::OccupationType;
use load_census_data::tables::population_and_density_per_output_area::AreaClassification;

use crate::config::{
    DEBUG_ITERATION_PRINT, get_memory_usage, STARTING_INFECTED_COUNT, WORKPLACE_BUILDING_SIZE,
};
use crate::disease::{DiseaseModel, DiseaseStatus, Exposure};
use crate::disease::DiseaseStatus::Infected;
use crate::interventions::{InterventionsEnabled, InterventionStatus};
use crate::models::build_polygons_for_output_areas;
use crate::models::building::{Building, BuildingCode, Workplace};
use crate::models::citizen::Citizen;
use crate::models::output_area::OutputArea;
use crate::statistics::Statistics;

pub struct Simulator {
    /// The total size of the population
    current_population: u32,
    /// A list of all the sub areas containing agents
    pub output_areas: HashMap<String, OutputArea>,
    /// The list of citizens who have a "home" in this area
    pub citizens: HashMap<Uuid, Citizen>,
    pub citizens_eligible_for_vaccine: Option<HashSet<Uuid>>,
    pub statistics: Statistics,
    interventions: InterventionStatus,
    disease_model: DiseaseModel,
    rng: ThreadRng,
}

/// Initialisation Methods
impl Simulator {
    pub fn new(census_data: CensusData) -> Result<Simulator> {
        let start = Instant::now();
        let mut rng = thread_rng();
        let disease_model = DiseaseModel::covid();
        let mut output_areas: HashMap<String, OutputArea> = HashMap::new();
        debug!("Current memory usage: {}", get_memory_usage()?);
        let mut output_areas_polygons =
            build_polygons_for_output_areas(CensusTableNames::OutputAreaMap.get_filename())
                .context("Loading polygons for output areas")?;
        info!("Loaded map data in {:?}", start.elapsed());
        let mut starting_population = 0;

        let mut citizens = HashMap::new();
        // Build the initial Output Areas and Households
        for entry in census_data.values() {
            let polygon = output_areas_polygons
                .remove(&entry.output_area_code)
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Building output areas map".to_string(),
                        key: entry.output_area_code.to_string(),
                    },
                })
                .context(format!(
                    "Loading polygon shape for area: {}",
                    entry.output_area_code.to_string()
                ))?;
            starting_population += entry.total_population_size() as u32;
            let mut new_area = OutputArea::new(entry.output_area_code.to_string(), polygon)
                .context("Failed to create Output Area")?;
            citizens.extend(
                new_area
                    .generate_citizens(entry, disease_model.mask_percentage, &mut rng)
                    .context("Failed to generate residents")?,
            );
            output_areas.insert(new_area.output_area_code.to_string(), new_area);
        }
        info!("Built residential population in {:?}", start.elapsed());
        debug!("Current memory usage: {}", get_memory_usage()?);

        let mut simulator = Simulator {
            current_population: starting_population,
            output_areas,
            citizens,
            citizens_eligible_for_vaccine: None,
            statistics: Statistics::default(),
            interventions: Default::default(),
            disease_model,
            rng: thread_rng(),
        };
        // Build the workplaces
        simulator
            .build_workplaces(census_data)
            .context("Failed to build workplaces")?;
        for citizen in simulator.citizens.values() {
            assert_ne!(citizen.household_code, citizen.workplace_code);
        }
        info!("Generated workplaces in {:?}", start.elapsed());
        debug!("Current memory usage: {}", get_memory_usage()?);
        // Infect random citizens
        simulator
            .apply_initial_infections()
            .context("Failed to create initial infections")?;

        info!(
            "Initialization completed in {} seconds",
            start.elapsed().as_secs_f32()
        );
        debug!("Current memory usage: {}", get_memory_usage()?);
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
    pub fn build_workplaces(&mut self, census_data: CensusData) -> anyhow::Result<()> {
        let areas: Vec<String> = self.output_areas.keys().cloned().collect();

        // Add Workplace Output Areas to Every Citizen
        let mut citizens_to_allocate: HashMap<String, Vec<Uuid>> = HashMap::new();
        for household_output_area_code in areas {
            let household_output_area = self
                .output_areas
                .get_mut(&household_output_area_code)
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Retrieving output area for building workplaces ".to_string(),
                        key: household_output_area_code.to_string(),
                    },
                })?;
            let household_census_data = census_data
                .get_output_area(household_output_area_code.to_string())
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Cannot retrieve Census Data for output area ".to_string(),
                        key: household_output_area_code.to_string(),
                    },
                })?;
            for citizen_id in household_output_area.get_residents() {
                let workplace_output_area_code = household_census_data
                    .get_random_workplace_area(&mut self.rng)
                    .context("Selecting a random workplace")?;
                if !citizens_to_allocate.contains_key(&workplace_output_area_code) {
                    citizens_to_allocate.insert(workplace_output_area_code.to_string(), Vec::new());
                }
                citizens_to_allocate
                    .get_mut(&workplace_output_area_code)
                    .ok_or_else(|| DataLoadingError::ValueParsingError {
                        source: ParseErrorType::MissingKey {
                            context: "Cannot retrieve Output Area to add Citizens to  ".to_string(),
                            key: workplace_output_area_code.to_string(),
                        },
                    })?
                    .push(citizen_id);
            }
        }
        // Create buildings for each Workplace output area
        for (workplace_area_code, mut to_allocate) in citizens_to_allocate {
            // Randomise the order of the citizens, to reduce the number of Citizens sharing household and Workplace output areas
            to_allocate.shuffle(&mut self.rng);

            // This is the Workplace list to allocate citizens to
            let mut current_workplaces_to_allocate: HashMap<OccupationType, Workplace> =
                HashMap::new();

            // This is the list of full workplaces that need to be added to the parent Output Area
            let mut workplace_buildings: HashMap<Uuid, Box<dyn Building>> = HashMap::new();
            for citizen_id in to_allocate {
                let citizen = self.citizens.get_mut(&citizen_id).ok_or_else(|| {
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
                        match workplace.add_citizen(citizen_id) {
                            Ok(_) => workplace,
                            Err(_) => {
                                workplace_buildings.insert(
                                    workplace.building_code().building_id(),
                                    Box::new(workplace),
                                );
                                // TODO Have better distribution of AreaClassification?
                                let mut workplace = Workplace::new(
                                    BuildingCode::new(
                                        workplace_area_code.clone(),
                                        AreaClassification::UrbanCity,
                                    ),
                                    WORKPLACE_BUILDING_SIZE,
                                    citizen.occupation(),
                                );
                                workplace.add_citizen(citizen_id).context(
                                    "Cannot add Citizen to freshly generated Workplace!",
                                )?;
                                workplace
                            }
                        }
                    }
                    None => {
                        // TODO Have better distribution of AreaClassification?
                        let mut workplace = Workplace::new(
                            BuildingCode::new(
                                workplace_area_code.clone(),
                                AreaClassification::UrbanCity,
                            ),
                            WORKPLACE_BUILDING_SIZE,
                            citizen.occupation(),
                        );
                        workplace.add_citizen(citizen_id)?;
                        workplace
                    }
                };
                citizen.set_workplace_code(workplace.building_code().clone());
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
                    workplace_buildings
                        .insert(workplace.building_code().building_id(), Box::new(workplace));
                });
            workplace_output_area.buildings[AreaClassification::UrbanCity]
                .extend(workplace_buildings);
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
        let exposures = self.generate_exposures()?;
        self.apply_exposures(exposures)?;
        self.apply_interventions()?;
        if !self.statistics.disease_exists() {
            info!("Disease finished as no one has the disease");
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn generate_exposures(&mut self) -> anyhow::Result<HashSet<Exposure>> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        let mut exposure_list: HashSet<Exposure> = HashSet::new();
        self.statistics.next();
        for citizen in &mut self.citizens.values_mut() {
            citizen.execute_time_step(
                self.statistics.time_step(),
                &self.disease_model,
                self.interventions.lockdown_enabled(),
            );
            self.statistics.add_citizen(&citizen.disease_status);
            if let Infected(_) = citizen.disease_status {
                exposure_list.insert(Exposure::new(
                    citizen.id(),
                    citizen.current_position.clone(),
                ));
            }
        }
        //debug!("There are {} exposures", exposure_list.len());
        Ok(exposure_list)
    }
    fn apply_exposures(&mut self, exposure_list: HashSet<Exposure>) -> anyhow::Result<()> {
        for exposure in exposure_list {
            let area = self.output_areas.get_mut(&exposure.output_area_code());
            match area {
                Some(area) => {
                    // TODO Sometime there's a weird bug here?
                    let building = &area.buildings[exposure.area_classification()]
                        .get_mut(&exposure.building_code())
                        .context(format!("Failed to retrieve exposure building {}", exposure))?;
                    let building = building.as_ref();
                    for citizen_id in building.occupants() {
                        let citizen = self.citizens.get_mut(citizen_id);
                        match citizen {
                            Some(citizen) => {
                                if citizen.expose(
                                    &self.disease_model,
                                    &self.interventions.mask_status,
                                    &mut self.rng,
                                ) {
                                    self.statistics
                                        .citizen_exposed(exposure.clone())
                                        .context(format!("Exposing citizen {}", citizen_id))?;

                                    if let Some(vaccine_list) =
                                    &mut self.citizens_eligible_for_vaccine
                                    {
                                        vaccine_list.remove(citizen_id);
                                    }
                                }
                            }
                            None => {
                                error!(
                                    "Citizen {}, does not exist in the expected area {}",
                                    citizen_id, area.output_area_code
                                );
                            }
                        }
                    }
                }

                None => {
                    error!(
                        "Cannot find area {}, that had an exposure ({}) occurred in!",
                        &exposure.output_area_code(),
                        exposure
                    );
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
                        citizen.1.current_position = home;
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
            let chosen: Vec<Uuid> = citizens
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
            for (_area_type, building_map) in area.1.buildings.iter() {
                writeln!(file, "      {:?}", area.0)?;
                for building in building_map.values() {
                    writeln!(file, "          {}", building)?;
                }
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
            let mut sub_areas = HashMap::new();

            for (area_type, mut building_map) in area.1.buildings {
                let mut buildings = HashMap::new();

                for (_code, building) in building_map.drain() {
                    buildings.insert(building.building_code().building_id().to_string(), building);
                }
                sub_areas.insert(area_type.to_string(), buildings);
            }
            output_area_json.insert(area.0, sub_areas);
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
