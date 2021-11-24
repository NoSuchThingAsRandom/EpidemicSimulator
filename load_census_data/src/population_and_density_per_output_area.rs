//! Intermediary and Post Processing Structs for the NOMIS Census Table 144
use std::convert::TryFrom;

use enum_map::EnumMap;
use serde::Deserialize;

use crate::parsing_error::{CensusError, ParseErrorType};
use crate::table_144_enum_values::{AreaClassification, PersonType};

pub const SELECTED_COLUMNS: &str = "GEOGRAPHY_NAME,GEOGRAPHY_TYPE,RURAL_URBAN_NAME,RURAL_URBAN_TYPECODE,CELL_NAME,MEASURES_NAME,OBS_VALUE,OBS_STATUS,RECORD_OFFSET,RECORD_COUNT";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PreProcessingRecord {
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


#[derive(Debug)]
pub struct PopulationRecord {
    pub geography_code: String,
    pub geography_type: String,
    pub area_size: f32,
    pub density: f32,
    pub population_counts: EnumMap<AreaClassification, EnumMap<PersonType, u16>>,
    pub population_size: u16,
}


impl TryFrom<Vec<PreProcessingRecord>> for PopulationRecord {
    type Error = CensusError;

    fn try_from(records: Vec<PreProcessingRecord>) -> Result<Self, Self::Error> {
        if records.is_empty() {
            return Err(CensusError::ValueParsingError { source: ParseErrorType::IsEmpty { message: String::from("PreProcessingRecord list is empty, can't build a PopulationRecord!") } });
        }
        let geography_code = String::from(&records[0].geography_name);
        let geography_type = String::from(&records[0].geography_type);
        let mut area_size: f32 = 0.0;
        let mut density: f32 = 0.0;
        let mut total_population = 0;
        let mut data: EnumMap<AreaClassification, EnumMap<PersonType, u16>> = EnumMap::default();
        for record in records {
            if record.geography_name != geography_code {
                return Err(CensusError::ValueParsingError { source: ParseErrorType::Mismatching { message: String::from("Mis matching geography codes for pre processing records"), value_1: geography_code, value_2: record.geography_name } });
            }
            if record.geography_type != geography_type {
                return Err(CensusError::ValueParsingError { source: ParseErrorType::Mismatching { message: String::from("Mis matching geography type for pre processing records"), value_1: geography_type, value_2: record.geography_type } });
            }
            if record.measures_name == "Value" {
                if &record.cell_name == "Area (Hectares)" {
                    area_size = record.obs_value.parse().unwrap_or(0.0);
                } else if &record.cell_name == "Density (number of persons per hectare)" {
                    density = record.obs_value.parse().unwrap_or(0.0);
                } else {
                    let area_classification: AreaClassification = serde_plain::from_str(&record.rural_urban_name)?;
                    let person_classification: PersonType = serde_plain::from_str(&record.cell_name)?;
                    let population_size = record.obs_value.parse()?;
                    total_population += population_size;
                    data[area_classification][person_classification] = population_size;
                }
            }
        }
        //debug!("New record: Code: {}, data: {:?}", geography_code, &data[AreaClassification::Total]);

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