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
use std::convert::TryFrom;
use std::fmt::Debug;

use rand::distributions::WeightedIndex;
use rand::prelude::Distribution;
use serde::Deserialize;

use crate::parsing_error::{DataLoadingError, ParseErrorType};
use crate::RngCore;
use crate::tables::{PreProcessingTable, TableEntry};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingAgePopulationRecord {
    pub geography_name: String,
    geography_type: String,
    rural_urban_name: String,
    c_age: usize,
    obs_value: String,
    obs_status: String,
    record_offset: u32,
    record_count: u32,
}

impl PreProcessingTable for PreProcessingAgePopulationRecord {
    fn get_geography_code(&self) -> String {
        self.geography_name.to_string()
    }
}

#[derive(Clone, Debug)]
pub struct AgePopulationRecord {
    /// This is the population of the age group,starting from 0, to the last entry being 100 and over
    pub population_counts: [u16; 101],
    age_weighting: WeightedIndex<u16>,
    pub population_size: u16,
}

impl AgePopulationRecord {
    pub fn get_random_age(
        &mut self,
        rng: &mut dyn RngCore,
    ) -> u16 {
        self.age_weighting.sample(rng) as u16
    }
}

impl TableEntry<PreProcessingAgePopulationRecord> for AgePopulationRecord {}

impl<'a> TryFrom<&'a Vec<Box<PreProcessingAgePopulationRecord>>> for AgePopulationRecord {
    type Error = DataLoadingError;
    /// Takes in a list of unsorted CSV record entries, and builds a hashmap of output areas with the given table data
    ///
    /// First iterates through the records, and checks they are all of type `PreProcessingRecord`, then adds them to a hashmap with keys of output areas
    ///
    /// Then converts all the PreProcessingRecords for one output area into a consolidated PopulationRecord
    ///
    fn try_from(
        records: &'a Vec<Box<PreProcessingAgePopulationRecord>>,
    ) -> Result<Self, Self::Error> {
        if records.is_empty() {
            return Err(DataLoadingError::ValueParsingError {
                source: ParseErrorType::IsEmpty {
                    message: String::from(
                        "PreProcessingRecord list is empty, can't build a PopulationRecord!",
                    ),
                },
            });
        }
        let geography_code = String::from(&records[0].geography_name);
        let geography_type = String::from(&records[0].geography_type);
        let mut total_population = 0;
        let mut data = [0; 101];
        for record in records {
            if record.geography_name != geography_code {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::Mismatching {
                        message: String::from(
                            "Mis matching geography codes for pre processing records",
                        ),
                        value_1: geography_code,
                        value_2: record.geography_name.clone(),
                    },
                });
            }
            if record.geography_type != geography_type {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::Mismatching {
                        message: String::from(
                            "Mis matching geography type for pre processing records",
                        ),
                        value_1: geography_type,
                        value_2: record.geography_type.clone(),
                    },
                });
            }
            assert_eq!(record.rural_urban_name, "Total", "Invalid Rural Area type ({}) for age structure table", record.rural_urban_name);
            // As an age of under 1, is 1
            let age = record.c_age - 1;
            assert!(age <= 100, "Age {} has exceed bounds of 100", age);
            let population_size = record.obs_value.parse()?;
            total_population += population_size;
            data[age] = population_size;
        }
        Ok(AgePopulationRecord {
            population_counts: data,
            age_weighting: WeightedIndex::new(&data).expect("Failed to build age weighted sampling"),
            population_size: total_population,
        })
    }
}
