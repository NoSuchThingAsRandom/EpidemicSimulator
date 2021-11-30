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

use log::info;

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
    pub workplace_density: EmploymentDensities,
}

impl<'a> CensusDataEntry<'a> {
    pub fn total_population_size(&self) -> u16 {
        self.population_count.population_size
    }
}

/// This is a container for all the Census Data Tables
pub struct CensusData {
    pub population_counts: HashMap<String, PopulationRecord>,
    pub occupation_counts: HashMap<String, OccupationCount>,
    pub workplace_density: EmploymentDensities,
}

impl CensusData {
    /// Attempts to load all the Census Tables stored on disk into memory
    pub fn load() -> Result<CensusData, CensusError> {
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
        })
    }

    /// Attempts to retrieve all records relating to the given output area code
    ///
    /// Will return None, if at least one table is missing the entry
    pub fn get_output_area<'a>(&self, code: &'a String) -> Option<CensusDataEntry> {
        Some(CensusDataEntry {
            output_area_code: code.clone(),
            population_count: self.population_counts.get(code)?,
            occupation_count: self.occupation_counts.get(code)?,
            workplace_density: EmploymentDensities {},
        })
    }
    /// Returns an iterator over Output Areas
    pub fn values(&self) -> impl Iterator<Item=CensusDataEntry> {
        let keys = self.population_counts.keys();
        keys.filter_map(|key| self.get_output_area(key))
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
