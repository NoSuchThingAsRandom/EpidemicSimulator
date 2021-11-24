use std::collections::HashMap;
use std::time::Instant;

use anyhow::{Context, Result};
use log::info;

use census_geography::output_area::OutputArea;
use load_census_data::parsing_error::{CensusError, ParseErrorType};

pub struct Simulator {
    /// The total size of the population
    pub current_population: u32,
    /// A list of all the sub areas containing agents
    pub output_areas: HashMap<String, OutputArea>,
}


impl Simulator {
    pub fn new() -> Result<Simulator> {
        let start = Instant::now();
        let mut output_areas: HashMap<String, OutputArea> = HashMap::new();

        let census_data = load_census_data::load_table_from_disk("../../data/tables/PopulationAndDensityPerEnglandOutputArea(144)-35645376-Records.csv".to_string()).context("Loading census table 144")?;
        let output_areas_polygons = census_geography::build_polygons_for_output_areas("data/census_map_areas/England_oa_2011/england_oa_2011.shp").context("Loading polygons for output areas")?;
        let mut starting_population = 0;
        for (code, polygon) in output_areas_polygons.into_iter() {
            // TODO Add failure case
            let census_for_current_area = census_data.get(&code).ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Building output areas map".to_string(), key: code.to_string() } })?;
            starting_population += census_for_current_area.population_size as u32;
            output_areas.insert(code.to_string(), OutputArea::new(code.to_string(), polygon, census_for_current_area));
        }

        info!("Initialization completed in {} seconds", start.elapsed().as_secs_f32());
        Ok(Simulator { current_population: starting_population, output_areas })
    }
}