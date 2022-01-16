/*
 * Epidemic Simulation Using Census Data (ESUCD)
 * Copyright (c)  2022. Sam Ralph
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

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::string::String;
use std::thread::sleep;
use std::time::Duration;

use log::{debug, info, warn};
use rand::{Rng, RngCore};

use crate::nomis_download::{build_table_request_string, DataFetcher};
use crate::osm_parsing::{OSMRawBuildings, TagClassifiedBuilding};
use crate::parsing_error::DataLoadingError;
use crate::tables::{CensusTableNames, PreProcessingTable, TableEntry};
use crate::tables::employment_densities::EmploymentDensities;
use crate::tables::occupation_count::{OccupationCountRecord, PreProcessingOccupationCountRecord};
use crate::tables::population_and_density_per_output_area::{
    PopulationRecord, PreProcessingPopulationDensityRecord,
};
use crate::tables::resides_vs_workplace::{
    PreProcessingWorkplaceResidentialRecord, WorkplaceResidentialRecord,
};

mod nomis_download;
pub mod osm_parsing;
pub mod parse_table;
pub mod parsing_error;
pub mod polygon_lookup;
pub mod tables;
pub mod voronoi_generator;

pub const OSM_FILENAME: &str = "OSM/england-latest.osm.pbf";
pub const OSM_CACHE_FILENAME: &str = "OSM/cached";

/// This is a container for all the Records relating to one Output Area for All Census Tables
pub struct CensusDataEntry<'a> {
    pub output_area_code: String,
    pub population_count: &'a PopulationRecord,
    pub occupation_count: &'a OccupationCountRecord,

    pub resides_workplace_count: &'a WorkplaceResidentialRecord,
    pub workplace_density: EmploymentDensities,
}

impl<'a> CensusDataEntry<'a> {
    pub fn total_population_size(&self) -> u16 {
        self.population_count.population_size
    }
    // TODO IS THIS FUCKED?
    pub fn get_random_workplace_area(
        &self,
        rng: &mut dyn RngCore,
    ) -> Result<String, DataLoadingError> {
        let chosen = rng.gen_range(0..self.resides_workplace_count.total_workplace_count);
        let mut index = 0;
        for (area_code, value) in self.resides_workplace_count.workplace_count.iter() {
            if index <= chosen && chosen <= index + *value {
                return Ok(area_code.to_string());
            }
            index += *value;
        }
        Err(DataLoadingError::Misc {
            source: format!(
                "Allocating a output area failed, as chosen value ({}) is out of range (0..{})",
                chosen, self.resides_workplace_count.total_workplace_count
            ),
        })
    }
}

/// This is a container for all the Census Data Tables
//#[derive(Clone)]
pub struct CensusData {
    /// The list of output area codes that are valid and complete (records exist for each table)
    pub valid_areas: HashSet<String>,
    pub population_counts: HashMap<String, PopulationRecord>,
    pub occupation_counts: HashMap<String, OccupationCountRecord>,
    pub workplace_density: EmploymentDensities,
    /// Residential Area -> Workplace Area -> Count
    pub residents_workplace: HashMap<String, WorkplaceResidentialRecord>,
}

/// Initialization
impl CensusData {
    /// Loads the workplace table from disk
    fn read_workplace_table(
        filename: String,
    ) -> Result<HashMap<String, HashMap<String, u32>>, DataLoadingError> {
        let mut workplace_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(filename)
            .map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: "Cannot create CSV Reader for workplace areas".to_string(),
            })?;
        let headers = workplace_reader
            .headers()
            .map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: "Failed to read workplace CSV headers".to_string(),
            })?
            .clone();
        let mut workplace_areas: HashMap<String, HashMap<String, u32>> =
            HashMap::with_capacity(headers.len());
        for record in workplace_reader.records() {
            let record = record.map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: "Failed to read record from workplace table".to_string(),
            })?;
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
    async fn fetch_generic_table<U: 'static + PreProcessingTable, T: TableEntry<U>>(
        census_directory: &str,
        region_code: &str,
        table_name: CensusTableNames,
        data_fetcher: &Option<DataFetcher>,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        let filename = String::new()
            + census_directory
            + "tables/"
            + region_code
            + "/"
            + table_name.get_filename();
        info!("Fetching table '{}'", filename);
        if !Path::new(&filename).exists() {
            warn!("{:?} table doesn't exist on disk!", table_name);
            if let Some(fetcher) = data_fetcher {
                info!("Downloading table '{:?}' from api", table_name);
                let request = build_table_request_string(table_name, region_code.to_string());
                fetcher
                    .download_and_save_table(&filename, request, None, None)
                    .await?;
            }
        }
        CensusData::read_generic_table_from_disk::<T, U>(&filename)
    }
    pub fn read_table_and_generate_filename<U: 'static + PreProcessingTable, T: TableEntry<U>>(
        census_directory: &str,
        region_code: &str,
        table_name: CensusTableNames,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        let filename = String::new()
            + census_directory
            + "tables/"
            + region_code
            + "/"
            + table_name.get_filename();
        CensusData::read_generic_table_from_disk::<T, U>(&filename)
    }

    /// This loads a census data table from disk
    pub fn read_generic_table_from_disk<T: TableEntry<U>, U: 'static + PreProcessingTable>(
        table_name: &str,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        debug!("Reading census table: '{}' from disk", table_name);
        let mut reader =
            csv::Reader::from_path(table_name).map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: format!("Failed to create csv reader for file: {}", table_name),
            })?;

        let data = reader
            .deserialize()
            .collect::<Result<Vec<U>, csv::Error>>()
            .map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: format!("Failed to parse csv file: {}", table_name),
            })?;
        debug!("Loaded table into pre processing");
        let data = T::generate(data)?;
        Ok(data)
    }
    /// Attempts to load all the Census Tables stored on disk into memory
    ///
    /// If they are not on disk will attempt to download
    pub async fn load_all_tables_async(
        census_directory: String,
        region_code: String,
        should_download: bool,
    ) -> Result<CensusData, DataLoadingError> {
        let data_fetcher = if should_download {
            Some(DataFetcher::default())
        } else {
            None
        };
        // Build population table
        let population_counts = CensusData::fetch_generic_table::<
            PreProcessingPopulationDensityRecord,
            PopulationRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::PopulationDensity,
            &data_fetcher,
        )
            .await?;

        // Build occupation table
        let occupation_counts = CensusData::fetch_generic_table::<
            PreProcessingOccupationCountRecord,
            OccupationCountRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::OccupationCount,
            &data_fetcher,
        )
            .await?;

        // Build residents workplace table
        let residents_workplace = CensusData::fetch_generic_table::<
            PreProcessingWorkplaceResidentialRecord,
            WorkplaceResidentialRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::ResidentialAreaVsWorkplaceArea,
            &data_fetcher,
        )
            .await?;
        println!("Built {} residential workplace areas with {} records", residents_workplace.len(), residents_workplace.iter().map(|(_, v)| v.workplace_count.len()).sum::<usize>());

        let mut census_data = CensusData {
            valid_areas: HashSet::with_capacity(population_counts.len()),
            population_counts,
            occupation_counts,
            workplace_density: EmploymentDensities {},
            residents_workplace,
        };
        census_data.filter_incomplete_output_areas();
        Ok(census_data)
    }
    /// Attempts to load all the Census Tables stored on disk into memory
    pub fn load_all_tables(
        census_directory: String,
        region_code: String,
        _should_download: bool,
    ) -> Result<CensusData, DataLoadingError> {
        // Build population table
        let population_counts = CensusData::read_table_and_generate_filename::<
            PreProcessingPopulationDensityRecord,
            PopulationRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::PopulationDensity,
        )?;

        // Build occupation table
        let occupation_counts = CensusData::read_table_and_generate_filename::<
            PreProcessingOccupationCountRecord,
            OccupationCountRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::OccupationCount,
        )?;

        // Build residents workplace table
        let residents_workplace = CensusData::read_table_and_generate_filename::<
            PreProcessingWorkplaceResidentialRecord,
            WorkplaceResidentialRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::ResidentialAreaVsWorkplaceArea,
        )?;
        sleep(Duration::from_secs(2));

        let mut census_data = CensusData {
            valid_areas: HashSet::with_capacity(population_counts.len()),
            population_counts,
            occupation_counts,
            workplace_density: EmploymentDensities {},
            residents_workplace,
        };
        census_data.filter_incomplete_output_areas();


        let mut chosen_areas = File::create("debug_dumps/census_resides_workplace.csv").unwrap();
        for (area, sub_areas) in &census_data.residents_workplace {
            chosen_areas.write(area.as_ref()).expect("Failed to dump");
            for a in sub_areas.workplace_count.keys() {
                chosen_areas.write((", ".to_string() + a).as_ref()).expect("Failed to dump");
            }
            chosen_areas.write(("\n").as_ref()).expect("Failed to dump");
        }
        Ok(census_data)
    }
    pub async fn resume_download(
        census_directory: &str,
        region_code: &str,
        table_name: CensusTableNames,
        resume_from_value: usize,
    ) -> Result<(), DataLoadingError> {
        info!(
            "Resuming download of table {:?} from record {}",
            table_name, resume_from_value
        );
        let data_fetcher = DataFetcher::default();
        let filename = String::new()
            + census_directory
            + "tables/"
            + region_code
            + "/"
            + table_name.get_filename();
        match &table_name {
            CensusTableNames::OccupationCount
            | CensusTableNames::PopulationDensity
            | CensusTableNames::ResidentialAreaVsWorkplaceArea => {
                info!("Downloading table '{:?}' from api", table_name);
                let request = build_table_request_string(table_name, region_code.to_string());
                data_fetcher
                    .download_and_save_table(&filename, request, None, Some(resume_from_value))
                    .await?;
            }
            CensusTableNames::OutputAreaMap => {}
        }
        info!("Finished resume");
        Ok(())
    }

    pub fn filter_incomplete_output_areas(&mut self) {
        info!("Removing incomplete Output Areas");
        // Filter out areas
        // TODO Is this the most optimal way?
        let mut valid_areas = HashSet::with_capacity(self.population_counts.len());
        for key in self.population_counts.keys() {
            if self.occupation_counts.contains_key(key) && self.residents_workplace.contains_key(key)
            {
                valid_areas.insert(key.to_string());
            }
        }
        debug!("Population area count: {:?}", self.population_counts.len());
        debug!("Occupation area count: {:?}", self.occupation_counts.len());
        debug!(
            "Residents area count:  {:?}",
            self.residents_workplace.len()
        );
        debug!("Filtered area count:   {:?}", valid_areas.len());

        self.population_counts
            .retain(|area, _| valid_areas.contains(area));
        self.occupation_counts
            .retain(|area, _| valid_areas.contains(area));
        self.residents_workplace
            .retain(|area, _| valid_areas.contains(area));
        let mut removed = 0;
        let mut new_size = 0;
        for record in self.residents_workplace.values_mut() {
            let mut total = 0;
            record.workplace_count.retain(|code, count| {
                if valid_areas.contains(code) {
                    new_size += 1;
                    total += *count;
                    true
                } else {
                    removed += 1;
                    false
                }
            });
            record.total_workplace_count = total;
        }
        debug!("Removed {} workplace areas. {} work areas out of {} home remaining",removed,new_size,self.residents_workplace.len());
        self.valid_areas = valid_areas;
        debug!("There are {} complete output areas", self.valid_areas.len());
    }
}

impl CensusData {
    /// Attempts to retrieve all records relating to the given output area code
    ///
    /// Will return None, if at least one table is missing the entry
    pub fn for_output_area_code(&self, code: String) -> Option<CensusDataEntry> {
        let workplace_area_distribution = self.residents_workplace.get(&code)?;
        Some(CensusDataEntry {
            population_count: self.population_counts.get(&code)?,
            occupation_count: self.occupation_counts.get(&code)?,
            output_area_code: code,
            workplace_density: EmploymentDensities {},
            resides_workplace_count: workplace_area_distribution,
        })
    }
    /// Returns an iterator over Output Areas
    pub fn values(&self) -> impl Iterator<Item=CensusDataEntry> {
        let keys = self.population_counts.keys();
        keys.filter_map(move |key| {
            let data = self.for_output_area_code(key.to_string());
            if data.is_none() {
                warn!("Output Area: {} is incomplete", key);
            }
            data
        })
    }
}

#[cfg(test)]
mod tests {
    use rand::thread_rng;

    use crate::CensusData;

    fn load_census_data() -> CensusData {
        CensusData::load_all_tables(
            "../data/".to_string(),
            "1946157112TYPE299".to_string(),
            false).expect("Failed to load data")
    }

    #[test]
    fn test_workplace_area_distrubution() {
        let data = load_census_data();
        let area_data = data.for_output_area_code("E00067299".to_string()).expect("Census area: 'E00067299' doesn't exist");
        let mut rng = thread_rng();
        for _ in 0..100 {
            println!("{}", area_data.get_random_workplace_area(&mut rng).unwrap())
        }
    }
}