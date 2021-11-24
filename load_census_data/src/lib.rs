#![allow(dead_code)]
#[macro_use]
extern crate enum_map;

use std::collections::HashMap;

use log::info;

use crate::parse_table::parse_table;
use crate::parsing_error::CensusError;
use crate::population_and_density_per_output_area::PopulationRecord;

mod nomis_download;
pub mod parsing_error;
pub mod population_and_density_per_output_area;
pub mod table_144_enum_values;
pub mod parse_table;

/// This loads a census data table from disk
pub fn load_table_from_disk(table_name: String) -> Result<HashMap<String, PopulationRecord>, CensusError> {
    info!("Loading census table: '{}'",table_name);
    let reader = csv::Reader::from_path("data/download/PopulationAndDensityPerEnglandOutputArea(144)-temp-Records.csv")?;
    parse_table(reader)
}
