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

use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;

use enum_map::EnumMap;
use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};
use strum_macros::EnumCount as EnumCountMacro;

use crate::parsing_error::{CensusError, ParseErrorType};
use crate::tables::{PreProcessingTable, TableEntry};

#[derive(Deserialize, Serialize, Debug, Enum, PartialEq, Eq, Hash, EnumCountMacro, Clone, Copy)]
pub enum OccupationType {
    /*    #[serde(alias = "Occupation: all categories: Occupation; measures: Value")]
    All,*/
    #[serde(alias = "Occupation: 1. managers, directors and senior officials; measures: Value")]
    Managers,
    #[serde(alias = "Occupation: 2. professional occupations; measures: Value")]
    Professional,
    #[serde(
        alias = "Occupation: 3. Associate professional and technical occupations; measures: Value"
    )]
    Technical,
    #[serde(alias = "Occupation: 4. administrative and secretarial occupations; measures: Value")]
    Administrative,
    #[serde(alias = "Occupation: 5. Skilled trades occupations; measures: Value")]
    SkilledTrades,
    #[serde(
        alias = "Occupation: 6. caring, leisure and other service occupations; measures: Value"
    )]
    Caring,
    #[serde(alias = "Occupation: 7. sales and customer service occupations; measures: Value")]
    Sales,
    #[serde(alias = "Occupation: 8. Process plant and machine operatives; measures: Value")]
    MachineOperatives,
    #[serde(alias = "Occupation: 9. Elementary occupations; measures: Value")]
    Teaching,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingOccupationCountRecordOLD {
    pub date: String,
    pub geography: String,
    #[serde(alias = "geography code")]
    pub geography_code: String,
    #[serde(alias = "Occupation: all categories: Occupation; measures: Value")]
    all: u32,
    #[serde(alias = "Occupation: 1. managers, directors and senior officials; measures: Value")]
    managers: u32,
    #[serde(alias = "Occupation: 2. professional occupations; measures: Value")]
    professional: u32,
    #[serde(
        alias = "Occupation: 3. Associate professional and technical occupations; measures: Value"
    )]
    technical: u32,
    #[serde(alias = "Occupation: 4. administrative and secretarial occupations; measures: Value")]
    administrative: u32,
    #[serde(alias = "Occupation: 5. Skilled trades occupations; measures: Value")]
    skilled_trades: u32,
    #[serde(
        alias = "Occupation: 6. caring, leisure and other service occupations; measures: Value"
    )]
    caring: u32,
    #[serde(alias = "Occupation: 7. sales and customer service occupations; measures: Value")]
    sales: u32,
    #[serde(alias = "Occupation: 8. Process plant and machine operatives; measures: Value")]
    machine_operatives: u32,
    #[serde(alias = "Occupation: 9. Elementary occupations; measures: Value")]
    teaching: u32,
    //pub occupation_count: HashMap<OccupationType, u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingOccupationCountRecord {
    pub geography_name: String,
    geography_type: String,
    cell_name: String,
    measures_name: String,
    obs_value: String,
    obs_status: String,
    record_offset: u32,
    record_count: u32,
}

impl PreProcessingTable for PreProcessingOccupationCountRecord {
    fn get_geography_code(&self) -> String {
        self.geography_name.to_string()
    }
}

#[derive(Debug)]
pub struct OccupationCountRecord {
    pub occupation_count: EnumMap<OccupationType, u32>,
    /// This is the sum of the values per occupation, so can be used for random generation
    total_range: u32,
}

impl OccupationCountRecord {
    pub fn get_random_occupation(
        &self,
        rng: &mut dyn RngCore,
    ) -> Result<OccupationType, CensusError> {
        let chosen = rng.gen_range(0..self.total_range);
        let mut index = 0;
        for (occupation_type, value) in self.occupation_count.iter() {
            if index <= chosen && chosen <= index + *value {
                return Ok(occupation_type);
            }
            index += *value;
        }
        Err(CensusError::Misc {
            source: format!(
                "Allocating a occupation failed, as chosen value ({}) is out of range (0..{})",
                chosen, self.total_range
            ),
        })
    }
}

impl TableEntry<PreProcessingOccupationCountRecord> for OccupationCountRecord {
    /*    fn generate(
            data: Vec<impl PreProcessingTable + 'static>,
        ) -> Result<HashMap<String, Self>, CensusError> {
            let mut output = HashMap::new();
            // Group the pre processing records, by output area
    /*        for entry in data {
                let entry = Box::new(entry) as Box<dyn Any>;
                if let Ok(entry) = entry.downcast::<PreProcessingOccupationCountRecord>() {
                    output.insert(entry.get_geography_code().clone(), OccupationCount::from(entry));
                } else {
                    return Err(CensusError::ValueParsingError {
                        source: ParseErrorType::InvalidDataType {
                            value: None,
                            expected_type: "Invalid pre processing type, for population density table!"
                                .to_string(),
                        },
                    });
                }
            }*/
            Ok(output)
        }*/
}

impl<'a> TryFrom<&'a Vec<Box<PreProcessingOccupationCountRecord>>> for OccupationCountRecord {
    type Error = CensusError;

    fn try_from(records: &'a Vec<Box<PreProcessingOccupationCountRecord>>) -> Result<Self, Self::Error> {
        if records.is_empty() {
            return Err(CensusError::ValueParsingError {
                source: ParseErrorType::IsEmpty {
                    message: String::from(
                        "PreProcessingRecord list is empty, can't build a OccupationCountRecord!",
                    ),
                },
            });
        }
        let geography_code = String::from(&records[0].geography_name);
        let geography_type = String::from(&records[0].geography_type);
        let mut total_range = 0;
        let mut occupation_count: EnumMap<OccupationType, u32> = EnumMap::default();
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
                let occupation: OccupationType = serde_plain::from_str(&record.cell_name)?;
                let count = record.obs_value.parse()?;
                total_range += count;
                occupation_count[occupation] = count;
            }
        }
        Ok(OccupationCountRecord {
            occupation_count,
            total_range,
        })
    }
}
