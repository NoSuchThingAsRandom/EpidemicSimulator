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

//! Intermediary and Post Processing Structs for the NOMIS Census Table 144
use std::any::Any;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Debug;

use enum_map::EnumMap;
use serde::Deserialize;

use crate::parsing_error::{CensusError, ParseErrorType};
use crate::tables::{PreProcessingTable, TableEntry};

/// This is a representation of Nomis Area Classifications for table 144
#[derive(Deserialize, Debug, Enum, Clone, Copy)]
pub enum AreaClassification {
    #[serde(alias = "Total")]
    Total,
    #[serde(alias = "Urban (total)")]
    UrbanTotal,
    #[serde(alias = "Urban major conurbation")]
    UrbanMajorConurbation,
    #[serde(alias = "Urban minor conurbation")]
    UrbanMinorConurbation,
    #[serde(alias = "Urban city and town")]
    UrbanCity,
    #[serde(alias = "Urban city and town in a sparse setting")]
    UrbanSparseTownCity,
    #[serde(alias = "Rural (total)")]
    RuralTotal,
    #[serde(alias = "Rural town and fringe")]
    RuralTown,
    #[serde(alias = "Rural town and fringe in a sparse setting")]
    RuralSparseTown,
    #[serde(alias = "Rural village")]
    RuralVillage,
    #[serde(alias = "Rural village in a sparse setting")]
    RuralSparseVillage,
    #[serde(alias = "Rural hamlet and isolated dwellings")]
    RuralHamlet,
    #[serde(alias = "Rural hamlet and isolated dwellings in a sparse setting")]
    RuralSparseHamlet,
}

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

/// The columns that should be fetched from the API
pub const SELECTED_COLUMNS: &str = "GEOGRAPHY_NAME,GEOGRAPHY_TYPE,RURAL_URBAN_NAME,RURAL_URBAN_TYPECODE,CELL_NAME,MEASURES_NAME,OBS_VALUE,OBS_STATUS,RECORD_OFFSET,RECORD_COUNT";

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

impl PreProcessingTable for PreProcessingPopulationDensityRecord {}

#[derive(Debug)]
pub struct PopulationRecord {
    pub geography_code: String,
    pub geography_type: String,
    pub area_size: f32,
    pub density: f32,
    pub population_counts: EnumMap<AreaClassification, EnumMap<PersonType, u16>>,
    pub population_size: u16,
}

impl TableEntry for PopulationRecord {
    /// Takes in a list of unsorted CSV record entries, and builds a hashmap of output areas with the given table data
    ///
    /// First iterates through the records, and checks they are all of type `PreProcessingRecord`, then adds them to a hashmap with keys of output areas
    ///
    /// Then converts all the PreProcessingRecords for one output area into a consolidated PopulationRecord
    ///
    /// TODO: THIS IS SO FUCKING CURSED - NEEDS A REWRITE
    fn generate(
        data: Vec<impl PreProcessingTable + 'static>,
    ) -> Result<HashMap<String, Self>, CensusError> {
        let mut buffer = HashMap::new();
        // Group the pre processing records, by output area
        for entry in data {
            let entry = Box::new(entry) as Box<dyn Any>;
            if let Ok(entry) = entry.downcast::<PreProcessingPopulationDensityRecord>() {
                if !buffer.contains_key(&entry.geography_name) {
                    buffer.insert(entry.geography_name.to_string(), Vec::new());
                }
                buffer.get_mut(&entry.geography_name.to_string()).ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Pre processing record, does not have a matching output area code, even though it should've been added???".to_string(), key: (entry.geography_name).to_string() } })?.push(entry);
            } else {
                return Err(CensusError::ValueParsingError {
                    source: ParseErrorType::InvalidDataType {
                        value: None,
                        expected_type: "Invalid pre processing type, for population density table!"
                            .to_string(),
                    },
                });
            }
        }
        // Convert into Population Records
        let mut output = HashMap::new();
        for (code, records) in buffer {
            output.insert(code.to_string(), PopulationRecord::try_from(records)?);
        }
        Ok(output)
    }
}

impl TryFrom<Vec<Box<PreProcessingPopulationDensityRecord>>> for PopulationRecord {
    type Error = CensusError;

    fn try_from(
        records: Vec<Box<PreProcessingPopulationDensityRecord>>,
    ) -> Result<Self, Self::Error> {
        if records.is_empty() {
            return Err(CensusError::ValueParsingError {
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
        let mut data: EnumMap<AreaClassification, EnumMap<PersonType, u16>> = EnumMap::default();
        for record in records {
            if record.geography_name != geography_code {
                return Err(CensusError::ValueParsingError {
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
                return Err(CensusError::ValueParsingError {
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
                    let area_classification: AreaClassification =
                        serde_plain::from_str(&record.rural_urban_name)?;
                    let person_classification: PersonType =
                        serde_plain::from_str(&record.cell_name)?;
                    let population_size = record.obs_value.parse()?;
                    total_population += population_size;
                    data[area_classification][person_classification] = population_size;
                }
            }
        }
        Ok(PopulationRecord {
            geography_code,
            geography_type,
            area_size,
            density,
            population_counts: data,
            population_size: total_population,
        })
    }
}