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
use std::time::Instant;

use anyhow::{Context, Result};
use log::{debug, error, info};
use rand::prelude::{IteratorRandom, SliceRandom};
use rand::rngs::ThreadRng;
use rand::thread_rng;
use uuid::Uuid;

use load_census_data::CensusData;
use load_census_data::parsing_error::{CensusError, ParseErrorType};
use load_census_data::tables::CensusTableNames;
use load_census_data::tables::occupation_count::OccupationType;
use load_census_data::tables::population_and_density_per_output_area::AreaClassification;

use crate::config::{DEBUG_ITERATION_PRINT, STARTING_INFECTED_COUNT, WORKPLACE_BUILDING_SIZE};
use crate::disease::{DiseaseModel, DiseaseStatus, Exposure, Statistics};
use crate::disease::DiseaseStatus::Infected;
use crate::models::build_polygons_for_output_areas;
use crate::models::building::{Building, BuildingCode, Workplace};
use crate::models::output_area::OutputArea;

pub struct Simulator {
    /// The total size of the population
    current_population: u32,
    /// A list of all the sub areas containing agents
    output_areas: HashMap<String, OutputArea>,
    current_statistics: Statistics,
    disease_model: DiseaseModel,
    rng: ThreadRng,
}

/// Initialisation Methods
impl Simulator {
    pub fn new() -> Result<Simulator> {
        let start = Instant::now();
        let mut rng = thread_rng();

        let mut output_areas: HashMap<String, OutputArea> = HashMap::new();
        info!("Loading data from disk...");
        let census_data = CensusData::load().context("Failed to load census data")?;
        info!("Loaded census data in {:?}", start.elapsed());
        let mut output_areas_polygons =
            build_polygons_for_output_areas(CensusTableNames::OutputAreaMap.get_filename())
                .context("Loading polygons for output areas")?;
        info!("Loaded map data in {:?}", start.elapsed());
        let mut starting_population = 0;

        // Build the initial Output Areas and Households
        for entry in census_data.values() {
            info!("{}",entry.output_area_code);
            let polygon = output_areas_polygons
                .remove(&entry.output_area_code)
                .ok_or_else(|| CensusError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Building output areas map".to_string(),
                        key: entry.output_area_code.to_string(),
                    },
                })?;
            starting_population += entry.total_population_size() as u32;
            let new = OutputArea::new(entry.output_area_code.to_string(), polygon, entry, &mut rng)?;


            output_areas.insert(new.code.to_string(), new);
        }
        info!("Built residential population in {:?}", start.elapsed());


        let mut simulator = Simulator {
            current_population: starting_population,
            output_areas,
            current_statistics: Statistics::default(),
            disease_model: DiseaseModel::covid(),
            rng: thread_rng(),
        };
        println!("{:?}", simulator.output_areas);
        // Build the workplaces
        simulator.build_workplaces(census_data)?;
        println!("{:?}", simulator.output_areas);
        info!("Generated workplaces in {:?}",start.elapsed());
        // Infect random citizens
        simulator.apply_initial_infections()?;

        info!(
            "Initialization completed in {} seconds",
            start.elapsed().as_secs_f32()
        );
        Ok(simulator)
    }

    /// Iterates through all Output Areas, and All Citizens in that Output Area
    ///
    /// Picks a workplace Output Area, determined from Census Data Distribution
    ///
    /// Allocates that Citizen to the Workplace Building in that chosen Output Area
    pub fn build_workplaces(&mut self, census_data: CensusData) -> anyhow::Result<()> {
        let areas: Vec<String> = self.output_areas.keys().cloned().collect();

        // Add Workplace Output Areas to Every Citizen
        let mut citizens_to_allocate: HashMap<String, Vec<(String, Uuid)>> = HashMap::new();
        for household_output_area_code in areas {
            let household_output_area = self.output_areas.get_mut(&household_output_area_code).ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Retrieving output area for building workplaces ".to_string(), key: household_output_area_code.to_string() } })?;
            let household_census_data = census_data.get_output_area(&household_output_area_code).ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Cannot retrieve Census Data for output area ".to_string(), key: household_output_area_code.to_string() } })?;

            for citizen_id in household_output_area.citizens.keys() {
                let workplace_output_area_code = household_census_data.get_random_workplace_area(&mut self.rng)?;
                if !citizens_to_allocate.contains_key(&workplace_output_area_code) {
                    citizens_to_allocate.insert(workplace_output_area_code.to_string(), Vec::new());
                }
                citizens_to_allocate.get_mut(&workplace_output_area_code).unwrap().push((household_output_area_code.to_string(), *citizen_id));
            }
        }

        // Assign buildings for each workplace output area

        for (workplace_area_code, mut to_allocate) in citizens_to_allocate {
            // Randomise the order of the citizens, to reduce the number of Citizens sharing household and workplace output areas
            to_allocate.shuffle(&mut self.rng);

            // This is the workplace list to allocate citizens to
            let mut current_workplaces_to_allocate: HashMap<OccupationType, Workplace> = HashMap::new();

            // This is the list of full workplaces that need to be added to the parent Output Area
            let mut workplace_buildings: HashMap<Uuid, Box<dyn Building>> = HashMap::new();
            for (home_output_area_code, citizen_id) in to_allocate {
                let citizen = self.output_areas.get_mut(&home_output_area_code)
                    .ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Retrieving output area for building workplaces ".to_string(), key: home_output_area_code.to_string() } })?
                    .citizens.get_mut(&citizen_id)
                    .ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Cannot retrieve Citizen to assign workplace ".to_string(), key: citizen_id.to_string() } })?;

                // 3 Cases
                // Work place exists and Citizen can be added:
                //      Add Citizen to it
                // Work place exists and Citizen cannot be added:
                //      Save the current workplace
                //      Generate a new workplace
                //      Add a Citizen to the new workplace
                // Work place doesn't exist
                //      Generate a new workplace
                //      Add a Citizen to the new workplace
                // Else
                let workplace = current_workplaces_to_allocate.remove(&citizen.occupation());
                let workplace = match workplace {
                    Some(mut workplace) => {
                        match workplace.add_citizen(citizen_id) {
                            Ok(_) => workplace,
                            Err(_) => {
                                workplace_buildings.insert(workplace.building_code().building_id(), Box::new(workplace));
                                // TODO Have better distribution of AreaClassification?
                                let mut workplace = Workplace::new(BuildingCode::new(workplace_area_code.clone(), AreaClassification::UrbanCity), WORKPLACE_BUILDING_SIZE, citizen.occupation());
                                workplace.add_citizen(citizen_id).context("Cannot add Citizen to freshly generated Workplace!")?;
                                workplace
                            }
                        }
                    }
                    None => {
                        // TODO Have better distrubution of AreaClassification?
                        let mut workplace = Workplace::new(BuildingCode::new(workplace_area_code.clone(), AreaClassification::UrbanCity), WORKPLACE_BUILDING_SIZE, citizen.occupation());
                        workplace.add_citizen(citizen_id)?;
                        workplace
                    }
                };
                citizen.set_workplace_code(workplace.building_code().clone());
                // Add the unfilled workplace back to the allocator
                current_workplaces_to_allocate.insert(citizen.occupation(), workplace);
            }
            let workplace_output_area = self.output_areas.get_mut(&workplace_area_code)
                .ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Retrieving output area for building workplaces ".to_string(), key: workplace_area_code.to_string() } })?;
            workplace_output_area.buildings[AreaClassification::UrbanCity].extend(workplace_buildings);
        }
        Ok(())
    }

    pub fn apply_initial_infections(&mut self) -> anyhow::Result<()> {
        let starting_area_code = self.output_areas
            .keys()
            .choose(&mut self.rng)
            .ok_or_else(|| CensusError::ValueParsingError {
                source: ParseErrorType::IsEmpty {
                    message: "No output areas exist for seeding the disease".to_string(),
                },
            })
            .context("Initialisation of disease!")?
            .to_string();
        let starting_area = self.output_areas
            .get_mut(&starting_area_code)
            .ok_or_else(|| CensusError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: "Randomly chosen output area doesn't exist!".to_string(),
                    key: starting_area_code.to_string(),
                },
            })
            .context("Initialisation of disease!")?;
        for _ in 0..STARTING_INFECTED_COUNT {
            let chosen_citizen = *starting_area
                .citizens
                .keys()
                .choose(&mut self.rng)
                .ok_or_else(|| CensusError::ValueParsingError {
                    source: ParseErrorType::IsEmpty {
                        message: format!(
                            "No citizens exist in the output areas {} for seeding the disease",
                            starting_area_code
                        ),
                    },
                })
                .context("Initialisation of disease!")?;
            let citizen = starting_area
                .citizens
                .get_mut(&chosen_citizen)
                .ok_or_else(|| CensusError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Randomly chosen citizen exist!".to_string(),
                        key: chosen_citizen.to_string(),
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
        let start_time = Instant::now();
        info!("Starting simulation...");
        for time_step in 0..self.disease_model.max_time_step {
            self.execute_time_step()?;
            if time_step % DEBUG_ITERATION_PRINT as u16 == 0 {
                info!(
                    "{:?}       - {}",
                    start_time.elapsed(),
                    self.current_statistics
                );
            }
            if !self.current_statistics.disease_exists() {
                info!(
                    "Disease finished at {} as no one has the disease",
                    time_step
                );
                debug!("{}",self.current_statistics);
                break;
            }
        }
        Ok(())
    }
    pub fn execute_time_step(&mut self) -> anyhow::Result<()> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        let mut exposure_list: HashSet<Exposure> = HashSet::new();
        let mut statistics = Statistics::new(self.current_statistics.time_step() + 1);
        for (_, area) in &mut self.output_areas {
            for citizen in &mut area.citizens.values_mut() {
                citizen.execute_time_step(self.current_statistics.time_step(), &self.disease_model);
                statistics.add_citizen(&citizen.disease_status);
                if let Infected(_) = citizen.disease_status {
                    exposure_list.insert(Exposure::new(
                        citizen.id(),
                        citizen.current_position.clone(),
                    ));
                }
            }
        }
        debug!("There are {} exposures",exposure_list.len());
        for exposure in exposure_list {
            let area = self.output_areas.get_mut(&exposure.output_area_code());
            match area {
                Some(area) => {
                    let building = &area.buildings[exposure.area_classification()].get_mut(&exposure.building_code()).context(format!("Failed to retrieve exposure building {}", exposure));
                    if let Err(e) = building {
                        println!("Fuck: {}", e);
                    }
                    let building = building.as_ref().unwrap();
                    for citizen_id in building.occupants() {
                        let citizen = area.citizens.get_mut(citizen_id);
                        match citizen {
                            Some(citizen) => {
                                if citizen.expose(&self.disease_model, &mut self.rng) {
                                    statistics
                                        .citizen_exposed()
                                        .context(format!("Exposing citizen {}", citizen_id))?;
                                }
                            }
                            None => {
                                error!(
                                        "Citizen {}, does not exist in the expected area {}",
                                        citizen_id, area.code
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
        self.current_statistics = statistics;
        Ok(())
    }
}
