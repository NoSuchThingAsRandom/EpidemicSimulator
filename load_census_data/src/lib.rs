#![allow(dead_code)]
#[macro_use]
extern crate enum_map;

use std::collections::HashMap;
use std::fs::File;

use log::info;

use tables::population_and_density_per_output_area::PopulationRecord;

use crate::parse_table::read_table;
use crate::parsing_error::CensusError;
use crate::tables::population_and_density_per_output_area::PreProcessingPopulationDensityRecord;

mod nomis_download;
pub mod parsing_error;
pub mod parse_table;
pub mod tables;

/// This loads a census data table from disk
pub fn load_table_from_disk(table_name: String) -> Result<HashMap<String, PopulationRecord>, CensusError> {
    info!("Loading census table: '{}'",table_name);
    let reader = csv::Reader::from_path(table_name)?;
    read_table::<File, PopulationRecord, PreProcessingPopulationDensityRecord>(reader)
}

pub async fn download_york_population() -> Result<(), CensusError> {
    let path = nomis_download::table_144_york_output_areas(20000);
    let fetcher = nomis_download::DataFetcher::default();
    fetcher.download_and_save_table(String::from("data/tables/york_population_144.csv"), path, 1000000, 20000).await?;
    Ok(())
}