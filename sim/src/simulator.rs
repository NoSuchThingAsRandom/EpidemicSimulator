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
use rand::prelude::IteratorRandom;
use rand::rngs::ThreadRng;
use rand::thread_rng;

use load_census_data::CensusData;
use load_census_data::parsing_error::{CensusError, ParseErrorType};
use load_census_data::tables::CensusTableNames;

use crate::config::{DEBUG_ITERATION_PRINT, STARTING_INFECTED_COUNT};
use crate::disease::{DiseaseModel, DiseaseStatus, Exposure, Statistics};
use crate::disease::DiseaseStatus::Infected;
use crate::models::build_polygons_for_output_areas;
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
        println!(
            "Size of polygons {:?}, amount of polygons {}",
            std::mem::size_of_val(&output_areas_polygons),
            output_areas_polygons.len()
        );
        info!("Loaded map data in {:?}", start.elapsed());
        let mut starting_population = 0;
        let mut index = 1;
        for entry in census_data.values() {
            // TODO Add failure case
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
            if index % DEBUG_ITERATION_PRINT == 0 {
                debug!("At index {} with time {:?}", index, start.elapsed());
            }
            index += 1;
        }
        info!("Built population in {:?}", start.elapsed());
        // Infect random citizens

        let starting_area_code = output_areas
            .keys()
            .choose(&mut rng)
            .ok_or_else(|| CensusError::ValueParsingError {
                source: ParseErrorType::IsEmpty {
                    message: "No output areas exist for seeding the disease".to_string(),
                },
            })
            .context("Initialisation of disease!")?
            .to_string();
        let starting_area = output_areas
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
                .choose(&mut rng)
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
        info!(
            "Initialization completed in {} seconds",
            start.elapsed().as_secs_f32()
        );
        Ok(Simulator {
            current_population: starting_population,
            output_areas,
            current_statistics: Statistics::default(),
            disease_model: DiseaseModel::covid(),
            rng: thread_rng(),
        })
    }

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
                    let building = &area.buildings[exposure.area_classification()].get_mut(&exposure.building_code()).context("Failed to retrieve exposure building ").unwrap();
                    for citizen_id in building.occupants() {
                        let citizen = area.citizens.get_mut(&citizen_id);
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
