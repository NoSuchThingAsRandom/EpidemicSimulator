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

use serde::Deserialize;
use strum_macros::EnumCount as EnumCountMacro;

use crate::parsing_error::{CensusError, ParseErrorType};
use crate::tables::{PreProcessingTable, TableEntry};

#[derive(Deserialize, Debug, Enum, PartialEq, Eq, Hash, EnumCountMacro)]
pub enum OccupationType {
    #[serde(alias = "Occupation: all categories: Occupation; measures: Value")]
    All,
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
pub struct PreProcessingOccupationCountRecord {
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

impl PreProcessingTable for PreProcessingOccupationCountRecord {}

#[derive(Debug)]
pub struct OccupationCount {
    pub occupation_count: HashMap<OccupationType, u32>,
}

impl TableEntry for OccupationCount {
    fn generate(
        data: Vec<impl PreProcessingTable + 'static>,
    ) -> Result<HashMap<String, Self>, CensusError> {
        let mut output = HashMap::new();
        // Group the pre processing records, by output area
        for entry in data {
            let entry = Box::new(entry) as Box<dyn Any>;
            if let Ok(entry) = entry.downcast::<PreProcessingOccupationCountRecord>() {
                output.insert(entry.geography_code.clone(), OccupationCount::from(entry));
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
        Ok(output)
    }
}

impl From<Box<PreProcessingOccupationCountRecord>> for OccupationCount {
    fn from(pre_processing: Box<PreProcessingOccupationCountRecord>) -> Self {
        let mut output = HashMap::new();
        // TODO I hate this
        output.insert(OccupationType::All, pre_processing.all);
        output.insert(OccupationType::Managers, pre_processing.managers);
        output.insert(OccupationType::Professional, pre_processing.professional);
        output.insert(OccupationType::Technical, pre_processing.technical);
        output.insert(
            OccupationType::Administrative,
            pre_processing.administrative,
        );
        output.insert(OccupationType::SkilledTrades, pre_processing.skilled_trades);
        output.insert(OccupationType::Caring, pre_processing.caring);
        output.insert(OccupationType::Sales, pre_processing.sales);
        output.insert(
            OccupationType::MachineOperatives,
            pre_processing.machine_operatives,
        );
        output.insert(OccupationType::Teaching, pre_processing.teaching);

        Self {
            occupation_count: output,
        }
    }
}
