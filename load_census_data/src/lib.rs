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

#![allow(dead_code)]
#[macro_use]
extern crate enum_map;

use std::collections::HashMap;
use std::path::Path;

use log::{info, warn};
use rand::{Rng, RngCore};
use serde::de::Unexpected::Str;

use crate::nomis_download::{build_table_request_string, DataFetcher};
use crate::parsing_error::CensusError;
use crate::tables::{CensusTableNames, PreProcessingTable, TableEntry};
use crate::tables::employment_densities::EmploymentDensities;
use crate::tables::occupation_count::{OccupationCountRecord, PreProcessingOccupationCountRecord};
use crate::tables::population_and_density_per_output_area::{
    PopulationRecord, PreProcessingPopulationDensityRecord,
};

mod nomis_download;
pub mod parse_table;
pub mod parsing_error;
pub mod tables;

/// This is a container for all the Records relating to one Output Area for All Census Tables
pub struct CensusDataEntry<'a> {
    pub output_area_code: String,
    pub population_count: &'a PopulationRecord,
    pub occupation_count: &'a OccupationCountRecord,

    pub workplace_area_distribution: &'a HashMap<String, u32>,
    /// The sum of all the values in workplace_area_distrubution - Used for generating random areas
    workplace_area_distribution_total: u32,
    pub workplace_density: EmploymentDensities,
}

impl<'a> CensusDataEntry<'a> {
    pub fn total_population_size(&self) -> u16 {
        self.population_count.population_size
    }
    pub fn get_random_workplace_area(&self, rng: &mut dyn RngCore) -> Result<String, CensusError> {
        let chosen = rng.gen_range(0..self.workplace_area_distribution_total);
        let mut index = 0;
        for (area_code, value) in self.workplace_area_distribution.iter() {
            if index <= chosen && chosen <= index + *value {
                return Ok(area_code.to_string());
            }
            index += *value;
        }
        Err(CensusError::Misc {
            source: format!(
                "Allocating a output area failed, as chosen value ({}) is out of range (0..{})",
                chosen, self.workplace_area_distribution_total
            ),
        })
    }
}

/// This is a container for all the Census Data Tables
pub struct CensusData {
    pub population_counts: HashMap<String, PopulationRecord>,
    pub occupation_counts: HashMap<String, OccupationCountRecord>,
    pub workplace_density: EmploymentDensities,
    /// Residential Area -> Workplace Area -> Count
    pub residents_workplace: HashMap<String, HashMap<String, u32>>,
}

/// Initialization
impl CensusData {
    /// Attempts to load the given table from a file on disk
    ///
    /// If the file doesn't exist and data_fetcher exists, will attempt to download the table from the NOMIS api
    async fn fetch_workplace_table(census_directory: &str, region_code: &str, data_fetcher: &Option<DataFetcher>) -> Result<HashMap<String, HashMap<String, u32>>, CensusError> {
        let table_name = CensusTableNames::ResidentialAreaVsWorkplaceArea;
        let filename = String::new() + census_directory + region_code + "/" + table_name.get_filename();
        if !Path::new(&filename).exists() {
            warn!("Workplace table doesn't exist on disk!");
            if let Some(fetcher) = data_fetcher {
                info!("Fetching table {:?} from api",table_name);
                let request = build_table_request_string(table_name, region_code.to_string());
                fetcher.download_and_save_table(&filename, request, None).await?;
            }
        }
        CensusData::read_workplace_table(filename)
    }

    /// Loads the workplace table from disk
    fn read_workplace_table(filename: String) -> Result<HashMap<String, HashMap<String, u32>>, CensusError> {
        let mut workplace_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(filename)?; //.context("Cannot create CSV Reader for workplace areas")?;
        let headers = workplace_reader.headers()?.clone();
        let mut workplace_areas: HashMap<String, HashMap<String, u32>> =
            HashMap::with_capacity(headers.len());
        for record in workplace_reader.records() {
            let record = record?;
            let mut current_workplace: HashMap<String, u32> = HashMap::with_capacity(headers.len());
            let area = record.get(0).unwrap().to_string();
            for index in 1..headers.len() {
                let count = record.get(index).unwrap().parse()?;
                current_workplace.insert(headers.get(index).unwrap().to_string(), count);
            }
            workplace_areas.insert(area, current_workplace);
        }
        Ok(workplace_areas)
    }

    /// Attempts to load the given table from a file on disk
    ///
    /// If the file doesn't exist and data_fetcher exists, will attempt to download the table from the NOMIS api
    async fn fetch_generic_table<U: 'static + PreProcessingTable, T: TableEntry<U>>(census_directory: &str, region_code: &str, table_name: CensusTableNames, data_fetcher: &Option<DataFetcher>) -> Result<HashMap<String, T>, CensusError> {
        let filename = String::new() + census_directory + region_code + "/" + table_name.get_filename();
        info!("Fetching table '{}'",filename);
        if !Path::new(&filename).exists() {
            warn!("{:?} table doesn't exist on disk!",table_name);
            if let Some(fetcher) = data_fetcher {
                info!("Downloading table '{:?}' from api",table_name);
                let request = build_table_request_string(table_name, region_code.to_string());
                fetcher.download_and_save_table(&filename, request, None).await?;
            }
        }
        CensusData::read_generic_table_from_disk::<T, U>(&filename)
    }

    /// This loads a census data table from disk
    pub fn read_generic_table_from_disk<T: TableEntry<U>, U: 'static + PreProcessingTable>(
        table_name: &str,
    ) -> Result<HashMap<String, T>, CensusError> {
        info!("Reading census table: '{}' from disk", table_name);
        let mut reader = csv::Reader::from_path(table_name)?;

        let data: Result<Vec<U>, csv::Error> = reader.deserialize().collect();
        let data = T::generate(data?)?;
        Ok(data)
    }
    /// Attempts to load all the Census Tables stored on disk into memory
    pub async fn load_all_tables(census_directory: String, region_code: String, should_download: bool) -> Result<CensusData, CensusError> {
        let data_fetcher = if should_download { Some(DataFetcher::default()) } else { None };

        //<PopulationRecord<PreProcessingPopulationDensityRecord>,        PreProcessingPopulationDensityRecord>;
        //CensusData::fetch_generic_table::<
        //                 OccupationCount,
        //                 PreProcessingOccupationCountRecord>(&census_directory, &region_code, CensusTableNames::OccupationCount, &data_fetcher).await?
        Ok(CensusData {
            population_counts: CensusData::fetch_generic_table::<PreProcessingPopulationDensityRecord, PopulationRecord>(&census_directory, &region_code, CensusTableNames::PopulationDensity, &data_fetcher).await?,
            occupation_counts: CensusData::fetch_generic_table::<PreProcessingOccupationCountRecord, OccupationCountRecord>(&census_directory, &region_code, CensusTableNames::PopulationDensity, &data_fetcher).await?,
            workplace_density: EmploymentDensities {},
            residents_workplace: CensusData::fetch_workplace_table(&census_directory, &region_code, &data_fetcher).await?,
        })
    }
}

impl CensusData {
    /// Attempts to retrieve all records relating to the given output area code
    ///
    /// Will return None, if at least one table is missing the entry
    pub fn get_output_area(&self, code: &String) -> Option<CensusDataEntry> {
        let workplace_area_distribution = self.residents_workplace.get(code)?;
        let total = workplace_area_distribution.values().copied().sum();
        Some(CensusDataEntry {
            output_area_code: code.clone(),
            population_count: self.population_counts.get(code)?,
            occupation_count: self.occupation_counts.get(code)?,
            workplace_density: EmploymentDensities {},
            workplace_area_distribution,
            workplace_area_distribution_total: total,
        })
    }
    /// Returns an iterator over Output Areas
    pub fn values(&self) -> impl Iterator<Item=CensusDataEntry> {
        let keys = self.population_counts.keys();
        keys.filter_map(|key| {
            let data = self.get_output_area(key);
            if data.is_none() {
                warn!("Output Area: {} is incomplete", key);
            }
            data
        })
    }
}
