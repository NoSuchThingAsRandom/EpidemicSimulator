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

use rand::distributions::WeightedIndex;
use rand::prelude::Distribution;
use serde::Deserialize;
use std::convert::TryFrom;
use std::fmt::Debug;

use crate::parsing_error::{DataLoadingError, ParseErrorType};
use crate::RngCore;
use crate::tables::{PreProcessingTable, TableEntry};

#[derive(Debug, Deserialize)]
pub struct PreProcessingTeacherRecord {
    time_period: String,
    time_identifier: String,
    geographic_level: String,
    country_code: String,
    country_name: String,
    region_code: String,
    region_name: String,
    old_la_code: String,
    new_la_code: String,
    la_name: String,
    school_type: String,
    number_schools: String,
    fte_workforce: String,
    fte_all_teachers: String,
    fte_classroom_teachers: String,
    fte_leadership_teachers: String,
    fte_head_teachers: String,
    fte_deputy_head_teachers: String,
    fte_assistant_head_teachers: String,
    fte_all_teachers_without_qts: String,
    fte_ft_all_teachers: String,
    fte_ft_classroom_teachers: String,
    fte_ft_leadership_teachers: String,
    fte_ft_head_teachers: String,
    fte_ft_deputy_head_teachers: String,
    fte_ft_assistant_head_teachers: String,
    fte_ft_all_teachers_without_qts: String,
    fte_pt_all_teachers: String,
    fte_pt_classroom_teachers: String,
    fte_pt_leadership_teachers: String,
    fte_pt_head_teachers: String,
    fte_pt_deputy_head_teachers: String,
    fte_pt_assistant_head_teachers: String,
    fte_pt_all_teachers_without_qts: String,
    fte_teaching_assistants: String,
    fte_other_school_support_staff: String,
    fte_administrative_staff: String,
    fte_technicians: String,
    fte_auxiliary_staff: String,
    fte_ft_teaching_assistants: String,
    fte_ft_other_school_support_staff: String,
    fte_ft_administrative_staff: String,
    fte_ft_technicians: String,
    fte_ft_auxiliary_staff: String,
    fte_pt_teaching_assistants: String,
    fte_pt_other_school_support_staff: String,
    fte_pt_administrative_staff: String,
    fte_pt_technicians: String,
    fte_pt_auxiliary_staff: String,
    hc_workforce: String,
    hc_all_teachers: String,
    hc_classroom_teachers: String,
    hc_leadership_teachers: String,
    hc_head_teachers: String,
    hc_deputy_head_teachers: String,
    hc_assistant_head_teachers: String,
    hc_all_teachers_without_qts: String,
    hc_occasional_teachers: String,
    hc_ft_all_teachers: String,
    hc_ft_classroom_teachers: String,
    hc_ft_leadership_teachers: String,
    hc_ft_head_teachers: String,
    hc_ft_deputy_head_teachers: String,
    hc_ft_assistant_head_teachers: String,
    hc_ft_all_teachers_without_qts: String,
    hc_pt_all_teachers: String,
    hc_pt_classroom_teachers: String,
    hc_pt_leadership_teachers: String,
    hc_pt_head_teachers: String,
    hc_pt_deputy_head_teachers: String,
    hc_pt_assistant_head_teachers: String,
    hc_pt_all_teachers_without_qts: String,
    hc_teaching_assistants: String,
    hc_other_school_support_staff: String,
    hc_administrative_staff: String,
    hc_technicians: String,
    hc_auxiliary_staff: String,
    hc_third_party_support_staff: String,
    hc_ft_teaching_assistants: String,
    hc_ft_other_school_support_staff: String,
    hc_ft_administrative_staff: String,
    hc_ft_technicians: String,
    hc_ft_auxiliary_staff: String,
    hc_pt_teaching_assistants: String,
    hc_pt_other_school_support_staff: String,
    hc_pt_administrative_staff: String,
    hc_pt_technicians: String,
    hc_pt_auxiliary_staff: String,
    percent_pt_teacher: String,
    ratio_of_teaching_assistants_to_all_teachers: String,
}

impl PreProcessingTable for PreProcessingTeacherRecord {
    fn get_geography_code(&self) -> String {
        self.new_la_code.to_string()
    }
}

#[derive(Clone, Debug)]
pub struct TeacherRecord {
    pub local_authority_code: String,
    /// This is the population of the age group,starting from 0, to the last entry being 100 and over
    pub population_counts: [u16; 101],
    age_weighting: WeightedIndex<u16>,
    pub population_size: u16,
}

impl TableEntry<PreProcessingTeacherRecord> for TeacherRecord {}

impl<'a> TryFrom<&'a Vec<Box<PreProcessingTeacherRecord>>> for TeacherRecord {
    type Error = DataLoadingError;
    /// Takes in a list of unsorted CSV record entries, and builds a hashmap of output areas with the given table data
    ///
    /// First iterates through the records, and checks they are all of type `PreProcessingRecord`, then adds them to a hashmap with keys of output areas
    ///
    /// Then converts all the PreProcessingRecords for one output area into a consolidated PopulationRecord
    ///
    fn try_from(
        records: &'a Vec<Box<PreProcessingTeacherRecord>>,
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
        let region_code = String::from(&records[0].region_code);
        if region_code == "Yorkshire and The Humber" {
            return Err(DataLoadingError::Misc { source: "Area code is not supported!".to_string() });
        }
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
