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
use std::collections::HashMap;
use std::convert::TryFrom;

use serde::Deserialize;

use crate::{DataLoadingError, PreProcessingTable, TableEntry};
use crate::parsing_error::ParseErrorType;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingWorkplaceResidentialRecord {
    pub currently_residing_in_code: String,
    //place_of_work_type: String,
    place_of_work_code: String,
    obs_value: String,
    //record_offset: u32,
    //record_count: u32,
}

impl PreProcessingTable for PreProcessingWorkplaceResidentialRecord {
    fn get_geography_code(&self) -> String {
        self.currently_residing_in_code.to_string()
    }
}

#[derive(Clone, Debug)]
pub struct WorkplaceResidentalRecord {
    pub workplace_count: HashMap<String, u32>,
    pub total_workplace_count: u32,
}

impl TableEntry<PreProcessingWorkplaceResidentialRecord> for WorkplaceResidentalRecord {}

impl<'a> TryFrom<&'a Vec<Box<PreProcessingWorkplaceResidentialRecord>>>
for WorkplaceResidentalRecord
{
    type Error = DataLoadingError;

    fn try_from(
        records: &'a Vec<Box<PreProcessingWorkplaceResidentialRecord>>,
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
        let residential_code = String::from(&records[0].currently_residing_in_code);
        let mut workplace_count = HashMap::new();
        for record in records {
            if record.currently_residing_in_code != residential_code {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::Mismatching {
                        message: String::from(
                            "Mis matching geography codes for pre processing records",
                        ),
                        value_1: residential_code,
                        value_2: record.currently_residing_in_code.clone(),
                    },
                });
            }
            //if record.place_of_work_type == "2011 output areas" {
                let count = record.obs_value.parse()?;
                if count > 0 {
                    total += count;
                    workplace_count.insert(record.place_of_work_code.to_string(), count);
                }
            //}
        }
        Ok(WorkplaceResidentalRecord {
            workplace_count,
            total_workplace_count: total,
        })
    }
}
