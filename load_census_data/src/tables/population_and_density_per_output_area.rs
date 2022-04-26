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

//! Intermediary and Post Processing Structs for the NOMIS Census Table 144

use std::convert::TryFrom;
use std::fmt::Debug;

use enum_map::EnumMap;
use serde::Deserialize;

use crate::parsing_error::{DataLoadingError, ParseErrorType};
use crate::tables::{PreProcessingTable, TableEntry};

#[derive(Deserialize, Debug, Enum)]
pub enum PersonType {
    #[serde(alias = "All usual residents")]
    All,
    #[serde(alias = "Males")]
    Male,
    #[serde(alias = "Females")]
    Female,
    #[serde(alias = "Lives in a household")]
    LivesInHousehold,
    #[serde(alias = "Lives in a communal establishment")]
    LivesInCommunalEstablishment,
    #[serde(
        alias = "Schoolchild or full-time student aged 4 and over at their non term-time address"
    )]
    Schoolchild,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingPopulationDensityRecord {
    pub geography_name: String,
    geography_type: String,
    rural_urban_name: String,
    cell_name: String,
    measures_name: String,
    obs_value: String,
    obs_status: String,
    record_offset: u32,
    record_count: u32,
}

impl PreProcessingTable for PreProcessingPopulationDensityRecord {
    fn get_geography_code(&self) -> String {
        self.geography_name.to_string()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PopulationRecord {
    pub area_size: f32,
    pub density: f32,
    pub population_counts: EnumMap<PersonType, u16>,
    pub population_size: u16,
}

impl TableEntry<PreProcessingPopulationDensityRecord> for PopulationRecord {}

impl<'a> TryFrom<&'a Vec<PreProcessingPopulationDensityRecord>> for PopulationRecord {
    type Error = DataLoadingError;
    /// Takes in a list of unsorted CSV record entries, and builds a hashmap of output areas with the given table data
    ///
    /// First iterates through the records, and checks they are all of type `PreProcessingRecord`, then adds them to a hashmap with keys of output areas
    ///
    /// Then converts all the PreProcessingRecords for one output area into a consolidated PopulationRecord
    ///
    fn try_from(
        records: &'a Vec<PreProcessingPopulationDensityRecord>,
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
        let mut area_size: f32 = 0.0;
        let mut density: f32 = 0.0;
        let mut total_population = 0;
        let mut data: EnumMap<PersonType, u16> = EnumMap::default();
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
            if record.measures_name == "Value" {
                if &record.cell_name == "Area (Hectares)" {
                    area_size = record.obs_value.parse().unwrap_or(0.0);
                } else if &record.cell_name == "Density (number of persons per hectare)" {
                    density = record.obs_value.parse().unwrap_or(0.0);
                } else {
                    assert_eq!(
                        record.rural_urban_name, "Total",
                        "Invalid Rural Area type ({}) for population count table",
                        record.rural_urban_name
                    );
                    let person_classification: PersonType =
                        serde_plain::from_str(&record.cell_name)?;
                    let population_size = record.obs_value.parse()?;
                    total_population += population_size;
                    data[person_classification] = population_size;
                }
            }
        }
        Ok(PopulationRecord {
            area_size,
            density,
            population_counts: data,
            population_size: total_population,
        })
    }
}
