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
use std::path::Path;
use std::string::String;

use log::{debug, info, trace, warn};
use rand::{Rng, RngCore};

use crate::nomis_download::{build_table_request_string, DataFetcher};
use crate::parsing_error::DataLoadingError;
use crate::tables::age_structure::{AgePopulationRecord, PreProcessingAgePopulationRecord};
use crate::tables::employment_densities::EmploymentDensities;
use crate::tables::occupation_count::{OccupationCountRecord, PreProcessingOccupationCountRecord};
use crate::tables::population_and_density_per_output_area::{
    PopulationRecord, PreProcessingPopulationDensityRecord,
};
use crate::tables::resides_vs_workplace::{
    PreProcessingBulkWorkplaceResidentialRecord, PreProcessingWorkplaceResidentialRecord,
    WorkplaceResidentialRecord,
};
use crate::tables::{CensusTableNames, PreProcessingTable, TableEntry};

mod nomis_download;
pub mod parse_table;
pub mod parsing_error;
pub mod tables;

/// This is a container for all the Records relating to one Output Area for All Census Tables
pub struct CensusDataEntry<'a> {
    pub output_area_code: String,
    pub population_count: &'a PopulationRecord,
    pub age_population: &'a mut AgePopulationRecord,
    pub occupation_count: &'a mut OccupationCountRecord,

    pub resides_workplace_count: &'a WorkplaceResidentialRecord,
    pub workplace_density: EmploymentDensities,
}

impl<'a> CensusDataEntry<'a> {
    pub fn total_population_size(&self) -> u16 {
        self.population_count.population_size
    }

    // TODO Use a WeightedIndex Distribution
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
    pub age_counts: HashMap<String, AgePopulationRecord>,
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
        CensusData::read_generic_table_from_disk::<T, U>(&filename, false)
    }
    pub fn read_table_and_generate_filename<U: 'static + PreProcessingTable, T: TableEntry<U>>(
        census_directory: &str,
        region_code: &str,
        table_name: CensusTableNames,
        is_bulk: bool,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        let filename = if is_bulk {
            String::new()
                + census_directory
                + "tables/"
                + region_code
                + "/"
                + table_name.get_bulk_filename()
        } else {
            String::new()
                + census_directory
                + "tables/"
                + region_code
                + "/"
                + table_name.get_filename()
        };
        CensusData::read_generic_table_from_disk::<T, U>(&filename, is_bulk)
    }

    /// This loads a census data table from disk
    pub fn read_generic_table_from_disk<T: TableEntry<U>, U: 'static + PreProcessingTable>(
        table_name: &str,
        is_bulk: bool,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        info!("Reading census table: '{}' from disk", table_name);
        let mut reader =
            csv::Reader::from_path(table_name).map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: format!("Failed to create csv reader for file: {}", table_name),
            })?;
        if is_bulk {
            Ok(HashMap::new())
        } else {
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
    }
    /// Attempts to load all the Census Tables stored on disk into memory
    ///
    /// If they are not on disk will attempt to download
    pub async fn load_all_tables_async(
        census_directory: String,
        area_code: String,
        should_download: bool,
    ) -> Result<CensusData, DataLoadingError> {
        let mut population_counts = None;
        let mut age_counts = None;
        let mut occupation_counts = None;
        let mut residents_workplace = None;
        trace!("Loading all tables");
        // TODO await doesn't work fetch all tables at once - But can't rayon scope with async downloads

        // If we are using Bulk data, the file format is different, and we can't automate download - but we can thread
        // Otherwise, we can download - but limited to single thread, which is fine as smaller data
        if area_code == "England" {
            info!("Using bulk tables");
            rayon::scope(|s| {
                s.spawn(|_| {
                    // Build population table
                    population_counts = Some(
                        CensusData::read_table_and_generate_filename::<
                            PreProcessingPopulationDensityRecord,
                            PopulationRecord,
                        >(
                            &census_directory,
                            &area_code,
                            CensusTableNames::PopulationDensity,
                            true,
                        )
                        .unwrap(),
                    );
                });
                s.spawn(|_| {
                    // Build population table
                    age_counts = Some(
                        CensusData::read_table_and_generate_filename::<
                            PreProcessingAgePopulationRecord,
                            AgePopulationRecord,
                        >(
                            &census_directory,
                            &area_code,
                            CensusTableNames::AgeStructure,
                            true,
                        )
                        .unwrap(),
                    );
                });
                s.spawn(|_| {
                    // Build occupation table
                    occupation_counts = Some(
                        CensusData::read_table_and_generate_filename::<
                            PreProcessingOccupationCountRecord,
                            OccupationCountRecord,
                        >(
                            &census_directory,
                            &area_code,
                            CensusTableNames::OccupationCount,
                            true,
                        )
                        .unwrap(),
                    );
                });
                s.spawn(|_| {
                    // Build residents workplace table
                    residents_workplace = Some(
                        CensusData::read_table_and_generate_filename::<
                            PreProcessingBulkWorkplaceResidentialRecord,
                            WorkplaceResidentialRecord,
                        >(
                            &census_directory,
                            &area_code,
                            CensusTableNames::ResidentialAreaVsWorkplaceArea,
                            true,
                        )
                        .unwrap(),
                    );
                });
            });
        } else {
            let data_fetcher = if should_download {
                Some(DataFetcher::default())
            } else {
                None
            };
            // Build population table
            population_counts = Some(
                CensusData::fetch_generic_table::<
                    PreProcessingPopulationDensityRecord,
                    PopulationRecord,
                >(
                    &census_directory,
                    &area_code,
                    CensusTableNames::PopulationDensity,
                    &data_fetcher,
                )
                .await?,
            );

            // Build population table
            age_counts = Some(
                CensusData::fetch_generic_table::<
                    PreProcessingAgePopulationRecord,
                    AgePopulationRecord,
                >(
                    &census_directory,
                    &area_code,
                    CensusTableNames::AgeStructure,
                    &data_fetcher,
                )
                .await?,
            );

            // Build occupation table
            occupation_counts = Some(
                CensusData::fetch_generic_table::<
                    PreProcessingOccupationCountRecord,
                    OccupationCountRecord,
                >(
                    &census_directory,
                    &area_code,
                    CensusTableNames::OccupationCount,
                    &data_fetcher,
                )
                .await?,
            );

            // Build residents workplace table
            residents_workplace = Some(
                CensusData::fetch_generic_table::<
                    PreProcessingWorkplaceResidentialRecord,
                    WorkplaceResidentialRecord,
                >(
                    &census_directory,
                    &area_code,
                    CensusTableNames::ResidentialAreaVsWorkplaceArea,
                    &data_fetcher,
                )
                .await?,
            );
        }
        let (population_counts, age_counts, occupation_counts, residents_workplace) = (
            population_counts.expect("Population Counts Table has not been loaded"),
            age_counts.expect("Age Counts Table has not been loaded"),
            occupation_counts.expect("Occupation Counts Table has not been loaded"),
            residents_workplace.expect("Residents Workplace Table has not been loaded"),
        );
        debug!(
            "Built {} residential workplace areas with {} records",
            residents_workplace.len(),
            residents_workplace
                .iter()
                .map(|(_, v)| v.workplace_count.len())
                .sum::<usize>()
        );

        let mut census_data = CensusData {
            valid_areas: HashSet::with_capacity(population_counts.len()),
            population_counts,
            age_counts,
            occupation_counts,
            workplace_density: EmploymentDensities {},
            residents_workplace,
        };
        census_data.filter_incomplete_output_areas();
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
        if CensusTableNames::OutputAreaMap != table_name {
            info!("Downloading table '{:?}' from api", table_name);
            let request = build_table_request_string(table_name, region_code.to_string());
            data_fetcher
                .download_and_save_table(&filename, request, None, Some(resume_from_value))
                .await?;
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
            if self.occupation_counts.contains_key(key)
                && self.residents_workplace.contains_key(key)
                && self.age_counts.contains_key(key)
            {
                valid_areas.insert(key.to_string());
            }
        }
        debug!("Population area count: {:?}", self.population_counts.len());
        debug!("Age area count: {:?}", self.age_counts.len());
        debug!("Occupation area count: {:?}", self.occupation_counts.len());
        debug!(
            "Residents area count:  {:?}",
            self.residents_workplace.len()
        );
        debug!("Filtered area count:   {:?}", valid_areas.len());

        self.population_counts
            .retain(|area, _| valid_areas.contains(area));
        self.age_counts.retain(|area, _| valid_areas.contains(area));
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
        debug!(
            "Removed {} workplace areas. {} work areas out of {} home remaining",
            removed,
            new_size,
            self.residents_workplace.len()
        );
        self.valid_areas = valid_areas;
        debug!("There are {} complete output areas", self.valid_areas.len());
    }
}

impl CensusData {
    /// Attempts to retrieve all records relating to the given output area code
    ///
    /// Will return None, if at least one table is missing the entry
    pub fn for_output_area_code(&mut self, code: String) -> Option<CensusDataEntry> {
        let workplace_area_distribution = self.residents_workplace.get(&code)?;
        Some(CensusDataEntry {
            population_count: self.population_counts.get(&code)?,
            age_population: self.age_counts.get_mut(&code)?,
            occupation_count: self.occupation_counts.get_mut(&code)?,
            output_area_code: code,
            workplace_density: EmploymentDensities {},
            resides_workplace_count: workplace_area_distribution,
        })
    }
}

#[cfg(test)]
mod tests {
    use rand::thread_rng;

    use crate::CensusData;

    #[test]
    fn test_workplace_area_distrubution() {
        let data = load_census_data();
        let area_data = data
            .for_output_area_code("E00067299".to_string())
            .expect("Census area: 'E00067299' doesn't exist");
        let mut rng = thread_rng();
        for _ in 0..100 {
            println!("{}", area_data.get_random_workplace_area(&mut rng).unwrap())
        }
    }
}
