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
use std::any::Any;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::{DataLoadingError, PreProcessingTable, TableEntry};
use crate::parsing_error::ParseErrorType;

pub trait PreProcessingWorkplaceResidentialTrait: PreProcessingTable + Debug + Sized {
    //fn get_geography_code(&self) -> String;
    fn get_workplace_code(&self) -> String;
    fn get_residential_code(&self) -> String;
    fn get_count(&self) -> u32;
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingWorkplaceResidentialRecord {
    pub currently_residing_in_code: String,
    place_of_work_type: String,
    place_of_work_name: String,
    obs_value: String,
    record_offset: u32,
    record_count: u32,
}

impl PreProcessingTable for PreProcessingWorkplaceResidentialRecord {
    fn get_geography_code(&self) -> String {
        self.currently_residing_in_code.to_string()
    }
}

impl PreProcessingWorkplaceResidentialTrait for PreProcessingWorkplaceResidentialRecord {
    fn get_workplace_code(&self) -> String {
        self.place_of_work_name.to_string()
    }

    fn get_residential_code(&self) -> String {
        self.currently_residing_in_code.to_string()
    }

    fn get_count(&self) -> u32 {
        self.obs_value.parse().expect("Failed to parse the count for WorkplaceResidential Record")
    }
}

#[derive(Debug, Deserialize)]
pub struct PreProcessingBulkWorkplaceResidentialRecord {
    #[serde(alias = "Area of usual residence")]
    residing_code: String,
    #[serde(alias = "Area of workplace")]
    work_code: String,
    count: u32,
}

impl PreProcessingTable for PreProcessingBulkWorkplaceResidentialRecord {
    fn get_geography_code(&self) -> String {
        self.residing_code.to_string()
    }
}

impl PreProcessingWorkplaceResidentialTrait for PreProcessingBulkWorkplaceResidentialRecord {
    fn get_workplace_code(&self) -> String {
        self.work_code.to_string()
    }

    fn get_residential_code(&self) -> String {
        self.residing_code.to_string()
    }

    fn get_count(&self) -> u32 {
        self.count
    }
}


#[derive(Clone, Debug)]
pub struct WorkplaceResidentialRecord {
    pub workplace_count: HashMap<String, u32>,
    pub total_workplace_count: u32,
}


impl TableEntry<PreProcessingWorkplaceResidentialRecord> for WorkplaceResidentialRecord {}

impl TableEntry<PreProcessingBulkWorkplaceResidentialRecord> for WorkplaceResidentialRecord {}

impl<'a, T: PreProcessingWorkplaceResidentialTrait> TryFrom<&'a Vec<T>> for WorkplaceResidentialRecord {
    type Error = DataLoadingError;

    fn try_from(
        records: &'a Vec<T>,
    ) -> Result<Self, Self::Error> {
        if records.is_empty() {
            return Err(DataLoadingError::ValueParsingError {
                source: ParseErrorType::IsEmpty {
                    message: String::from(
                        "PreProcessingRecord list is empty, can't build a Residential Workplace Record!",
                    ),
                },
            });
        }
        let mut total = 0;
        let residential_code = String::from(&records[0].get_residential_code());
        let mut workplace_count = HashMap::new();
        for record in records {
            if record.get_residential_code() != residential_code {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::Mismatching {
                        message: String::from(
                            "Mis matching geography codes for pre processing records",
                        ),
                        value_1: residential_code,
                        value_2: record.get_residential_code(),
                    },
                });
            }
            let count = record.get_count();
            if count > 0 {
                total += count;
                workplace_count.insert(record.get_workplace_code(), count);
            }
        }
        Ok(WorkplaceResidentialRecord {
            workplace_count,
            total_workplace_count: total,
        })
    }
}
