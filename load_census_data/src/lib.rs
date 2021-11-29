#![allow(dead_code)]
#[macro_use]
extern crate enum_map;

use std::collections::HashMap;

use log::info;

use crate::parsing_error::CensusError;
use crate::tables::{PreProcessingTable, TableEntry};

mod nomis_download;
pub mod parse_table;
pub mod parsing_error;
pub mod tables;

/// This loads a census data table from disk
pub fn load_table_from_disk<T: TableEntry, U: 'static + PreProcessingTable>(
    table_name: &str,
) -> Result<HashMap<String, T>, CensusError> {
    info!("Loading census table: '{}'", table_name);
    let mut reader = csv::Reader::from_path(table_name)?;

    let data: Result<Vec<U>, csv::Error> = reader.deserialize().collect();
    let data = T::generate(data?)?;
    Ok(data)
}

pub async fn download_york_population() -> Result<(), CensusError> {
    let path = nomis_download::table_144_york_output_areas(20000);
    let fetcher = nomis_download::DataFetcher::default();
    fetcher
        .download_and_save_table(
            String::from("data/tables/york_population_144.csv"),
            path,
            1000000,
            20000,
        )
        .await?;
    Ok(())
}
