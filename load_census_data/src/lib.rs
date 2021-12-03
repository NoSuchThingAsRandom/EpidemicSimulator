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

use log::{info, warn};
use rand::{Rng, RngCore};

use crate::parsing_error::CensusError;
use crate::tables::{CensusTableNames, PreProcessingTable, TableEntry};
use crate::tables::employment_densities::EmploymentDensities;
use crate::tables::occupation_count::{OccupationCount, PreProcessingOccupationCountRecord};
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
    pub occupation_count: &'a OccupationCount,

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
    pub occupation_counts: HashMap<String, OccupationCount>,
    pub workplace_density: EmploymentDensities,
    /// Residential Area -> Workplace Area -> Count
    pub residents_workplace: HashMap<String, HashMap<String, u32>>,
}

impl CensusData {
    /// Attempts to load all the Census Tables stored on disk into memory
    pub fn load() -> Result<CensusData, CensusError> {
        let mut workplace_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(CensusTableNames::WorkLocations.get_filename())?; //.context("Cannot create CSV Reader for workplace areas")?;
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
        info!("Loaded workplace areas");
        Ok(CensusData {
            population_counts: CensusData::load_table_from_disk::<
                PopulationRecord,
                PreProcessingPopulationDensityRecord,
            >(CensusTableNames::PopulationDensity.get_filename())?,
            occupation_counts: CensusData::load_table_from_disk::<
                OccupationCount,
                PreProcessingOccupationCountRecord,
            >(CensusTableNames::OccupationCount.get_filename())?,
            workplace_density: EmploymentDensities {},
            residents_workplace: workplace_areas,
        })
    }

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

    /// This downloads a Census table from the NOMIS API
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
}
