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

use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::iter::FromIterator;

use enum_map::EnumMap;
use rand::{distributions::Distribution, Rng, RngCore};
use rand::distributions::WeightedIndex;
use serde::{Deserialize, Serialize};
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

use crate::parsing_error::{DataLoadingError, ParseErrorType};
use crate::tables::{PreProcessingTable, TableEntry};

#[derive(
Deserialize, Serialize, Debug, Enum, PartialEq, Eq, Hash, EnumCountMacro, Clone, Copy, EnumIter,
)]
pub enum RawOccupationType {
    #[serde(alias = "All categories: Occupation")]
    All,
    #[serde(alias = "1. Managers, directors and senior officials")]
    Managers,
    #[serde(alias = "2. Professional occupations")]
    Professional,
    #[serde(alias = "3. Associate professional and technical occupations")]
    Technical,
    #[serde(alias = "4. Administrative and secretarial occupations")]
    Administrative,
    #[serde(alias = "5. Skilled trades occupations")]
    SkilledTrades,
    #[serde(alias = "6. Caring, leisure and other service occupations")]
    Caring,
    #[serde(alias = "7. Sales and customer service occupations")]
    Sales,
    #[serde(alias = "8. Process plant and machine operatives")]
    MachineOperatives,
    #[serde(alias = "9. Elementary occupations")]
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

#[derive(Clone, Debug)]
pub struct OccupationCountRecord {
    occupations: Vec<RawOccupationType>,
    occupation_population: Vec<u32>,
    occupation_weighting: WeightedIndex<u32>,
    /// This is the sum of the values per occupation, so can be used for random generation
    total_range: u32,
}

impl OccupationCountRecord {
    pub fn get_random_occupation(
        &mut self,
        rng: &mut dyn RngCore,
    ) -> RawOccupationType {
        return self.occupations[self.occupation_weighting.sample(rng)];
    }
}

impl TableEntry<PreProcessingOccupationCountRecord> for OccupationCountRecord {}

impl<'a> TryFrom<&'a Vec<Box<PreProcessingOccupationCountRecord>>> for OccupationCountRecord {
    type Error = DataLoadingError;

    fn try_from(
        records: &'a Vec<Box<PreProcessingOccupationCountRecord>>,
    ) -> Result<Self, Self::Error> {
        if records.is_empty() {
            return Err(DataLoadingError::ValueParsingError {
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
        let mut occupations = Vec::with_capacity(records.len());
        let mut occupation_population = Vec::with_capacity(records.len());
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
                let occupation: RawOccupationType = serde_plain::from_str(&record.cell_name)?;
                if occupation == RawOccupationType::All {
                    continue;
                }
                let count = record.obs_value.parse()?;
                total_range += count;
                occupations.push(occupation);
                occupation_population.push(count);
            }
        }
        Ok(OccupationCountRecord {
            occupations,
            occupation_population: vec![],
            total_range,
            occupation_weighting: WeightedIndex::new(&occupation_population).expect("Failed to build weighted sampling"),
        })
    }
}
