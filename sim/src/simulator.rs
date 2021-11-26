use std::collections::{HashMap, HashSet};
use std::time::Instant;

use anyhow::{Context, Result};
use log::{debug, error, info};
use rand::thread_rng;
use rand::rngs::ThreadRng;

use load_census_data::parsing_error::{CensusError, ParseErrorType};

use crate::disease::{DiseaseModel, Exposure, Statistics};
use crate::disease::DiseaseStatus::Infected;
use crate::models::build_polygons_for_output_areas;
use crate::models::output_area::OutputArea;

const DEBUG_PRINT: u16 = 20;

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
        let mut output_areas: HashMap<String, OutputArea> = HashMap::new();

        let census_data = load_census_data::load_table_from_disk("../../data/tables/PopulationAndDensityPerEnglandOutputArea(144)-35645376-Records.csv".to_string()).context("Loading census table 144")?;
        let output_areas_polygons = build_polygons_for_output_areas("data/census_map_areas/England_oa_2011/england_oa_2011.shp").context("Loading polygons for output areas")?;
        let mut starting_population = 0;
        for (code, polygon) in output_areas_polygons.into_iter() {
            // TODO Add failure case
            let census_for_current_area = census_data.get(&code).ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Building output areas map".to_string(), key: code.to_string() } })?;
            starting_population += census_for_current_area.population_size as u32;
            output_areas.insert(code.to_string(), OutputArea::new(code.to_string(), polygon, census_for_current_area));
        }

        info!("Initialization completed in {} seconds", start.elapsed().as_secs_f32());
        Ok(Simulator { current_population: starting_population, output_areas, current_statistics: Statistics::default(), disease_model: DiseaseModel::covid(), rng: thread_rng() })
    }

    pub fn simulate(&mut self) -> anyhow::Result<()> {
        let start_time = Instant::now();
        info!("Starting simulation...");
        for time_step in 0..self.disease_model.max_time_step {
            self.execute_time_step()?;
            if time_step % DEBUG_PRINT == 0 {
                info!("{:?}       - {}",start_time.elapsed(), self.current_statistics);
            }
            if !self.current_statistics.disease_exists() {
                info!("Disease finished at {} as no one has the disease",time_step);
                break;
            }
        }
        Ok(())
    }
    pub fn execute_time_step(&mut self) -> anyhow::Result<()> {
        debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        let mut exposure_list: HashSet<Exposure> = HashSet::new();
        let mut statistics = Statistics::default();
        for (_, area) in &mut self.output_areas {
            for citizen in &mut area.citizens.values_mut() {
                citizen.execute_time_step(self.current_statistics.time_step(), &self.disease_model);
                statistics.add_citizen(&citizen.disease_status);
                if let Infected(_) = citizen.disease_status {
                    exposure_list.insert(Exposure::new(citizen.id(), citizen.current_position.clone()));
                }
            }
        }
        for exposure in exposure_list {
            let area = self.output_areas.get_mut(&exposure.output_area_code());
            match area {
                Some(area) => {
                    let buildings = &area.buildings[*(&exposure.area_classification())];
                    for building in buildings {
                        for citizen_id in building.occupants() {
                            let citizen = area.citizens.get_mut(&citizen_id);
                            match citizen {
                                Some(citizen) => {
                                    if citizen.expose(&self.disease_model, &mut self.rng) {
                                        statistics.citizen_exposed().context(format!("Exposing citizen {}", citizen_id))?;
                                    }
                                }
                                None => {
                                    error!("Citizen {}, does not exist in the expected area {}",citizen_id,area.code);
                                }
                            }
                        }
                    }
                }
                None => {
                    error!("Cannot find area {}, that had an exposure ({}) occurred in!",&exposure.output_area_code(),exposure);
                }
            }
        }
        self.current_statistics = statistics;
        Ok(())
    }
}