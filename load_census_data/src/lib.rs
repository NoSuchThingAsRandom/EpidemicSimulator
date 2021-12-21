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

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::string::String;

use geo_types::Point;
use log::{debug, info, warn};
use rand::{Rng, RngCore};

use crate::nomis_download::{build_table_request_string, DataFetcher};
use crate::osm_parsing::{OSMRawBuildings, RawBuildingTypes};
use crate::parsing_error::DataLoadingError;
use crate::tables::{CensusTableNames, PreProcessingTable, TableEntry};
use crate::tables::employment_densities::EmploymentDensities;
use crate::tables::occupation_count::{OccupationCountRecord, PreProcessingOccupationCountRecord};
use crate::tables::population_and_density_per_output_area::{
    PopulationRecord, PreProcessingPopulationDensityRecord,
};
use crate::tables::resides_vs_workplace::{
    PreProcessingWorkplaceResidentialRecord, WorkplaceResidentalRecord,
};

mod nomis_download;
pub mod osm_parsing;
pub mod parse_table;
pub mod parsing_error;
pub mod tables;

const OSM_FILENAME: &str = "data/england-latest.osm.pbf";

/// This is a container for all the Records relating to one Output Area for All Census Tables
pub struct CensusDataEntry<'a> {
    pub output_area_code: String,
    pub population_count: &'a PopulationRecord,
    pub occupation_count: &'a OccupationCountRecord,

    pub resides_workplace_count: &'a WorkplaceResidentalRecord,
    pub workplace_density: EmploymentDensities,
}

impl<'a> CensusDataEntry<'a> {
    pub fn total_population_size(&self) -> u16 {
        self.population_count.population_size
    }
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
    pub residents_workplace: HashMap<String, WorkplaceResidentalRecord>,
    pub osm_buildings: OSMRawBuildings,
}

/// Initialization
impl CensusData {
    /// Loads the workplace table from disk
    fn read_workplace_table(
        filename: String,
    ) -> Result<HashMap<String, HashMap<String, u32>>, DataLoadingError> {
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
    async fn fetch_generic_table<U: 'static + PreProcessingTable, T: TableEntry<U>>(
        census_directory: &str,
        region_code: &str,
        table_name: CensusTableNames,
        data_fetcher: &Option<DataFetcher>,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        let filename =
            String::new() + census_directory + region_code + "/" + table_name.get_filename();
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
        let filename =
            String::new() + census_directory + region_code + "/" + table_name.get_filename();
        CensusData::read_generic_table_from_disk::<T, U>(&filename)
    }

    /// This loads a census data table from disk
    pub fn read_generic_table_from_disk<T: TableEntry<U>, U: 'static + PreProcessingTable>(
        table_name: &str,
    ) -> Result<HashMap<String, T>, DataLoadingError> {
        debug!("Reading census table: '{}' from disk", table_name);
        let mut reader = csv::Reader::from_path(table_name)?;

        let data: Result<Vec<U>, csv::Error> = reader.deserialize().collect();
        debug!("Loaded table into pre processing");
        let data = T::generate(data?)?;
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
        let mut population_counts = CensusData::fetch_generic_table::<
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
        let mut occupation_counts = CensusData::fetch_generic_table::<
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
        let mut residents_workplace = CensusData::fetch_generic_table::<
            PreProcessingWorkplaceResidentialRecord,
            WorkplaceResidentalRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::ResidentialAreaVsWorkplaceArea,
            &data_fetcher,
        )
            .await?;

        // Filter out areas
        // TODO Is this the most optimal way?
        let mut valid_areas = HashSet::with_capacity(population_counts.len());
        for key in population_counts.keys() {
            if occupation_counts.contains_key(key) && residents_workplace.contains_key(key) {
                valid_areas.insert(key.to_string());
            }
        }
        population_counts.retain(|area, _| valid_areas.contains(area));
        occupation_counts.retain(|area, _| valid_areas.contains(area));
        residents_workplace.retain(|area, _| valid_areas.contains(area));
        for record in residents_workplace.values_mut() {
            let mut total = 0;
            record.workplace_count.retain(|code, count| {
                if valid_areas.contains(code) {
                    total += *count;
                    true
                } else {
                    false
                }
            });
            record.total_workplace_count = total;
        }
        Ok(CensusData {
            valid_areas,
            population_counts,
            occupation_counts,
            workplace_density: EmploymentDensities {},
            residents_workplace,
            osm_buildings: OSMRawBuildings::build_osm_data(OSM_FILENAME.to_string())?,
        })
    }
    /// Attempts to load all the Census Tables stored on disk into memory
    pub fn load_all_tables(
        census_directory: String,
        region_code: String,
        _should_download: bool,
    ) -> Result<CensusData, DataLoadingError> {
        // Build population table
        let mut population_counts = CensusData::read_table_and_generate_filename::<
            PreProcessingPopulationDensityRecord,
            PopulationRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::PopulationDensity,
        )?;

        // Build occupation table
        let mut occupation_counts = CensusData::read_table_and_generate_filename::<
            PreProcessingOccupationCountRecord,
            OccupationCountRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::OccupationCount,
        )?;

        // Build residents workplace table
        let mut residents_workplace = CensusData::read_table_and_generate_filename::<
            PreProcessingWorkplaceResidentialRecord,
            WorkplaceResidentalRecord,
        >(
            &census_directory,
            &region_code,
            CensusTableNames::ResidentialAreaVsWorkplaceArea,
        )?;

        // Filter out areas
        // TODO Is this the most optimal way?
        let mut valid_areas = HashSet::with_capacity(population_counts.len());
        for key in population_counts.keys() {
            if occupation_counts.contains_key(key) && residents_workplace.contains_key(key) {
                valid_areas.insert(key.to_string());
            }
        }
        population_counts.retain(|area, _| valid_areas.contains(area));
        occupation_counts.retain(|area, _| valid_areas.contains(area));
        residents_workplace.retain(|area, _| valid_areas.contains(area));
        for record in residents_workplace.values_mut() {
            let mut total = 0;
            record.workplace_count.retain(|code, count| {
                if valid_areas.contains(code) {
                    total += *count;
                    true
                } else {
                    false
                }
            });
            record.total_workplace_count = total;
        }
        Ok(CensusData {
            valid_areas,
            population_counts,
            occupation_counts,
            workplace_density: EmploymentDensities {},
            residents_workplace,
            osm_buildings: OSMRawBuildings::build_osm_data(OSM_FILENAME.to_string())?
        })
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
        let filename =
            String::new() + census_directory + region_code + "/" + table_name.get_filename();
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
