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
use std::convert::TryFrom;
use std::fmt::Debug;

use log::trace;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::parsing_error::DataLoadingError;

pub mod employment_densities;
pub mod occupation_count;
pub mod population_and_density_per_output_area;
pub mod resides_vs_workplace;
/// This is used to load in a CSV file, and each row corresponds to one struct
pub trait PreProcessingTable: Debug + DeserializeOwned + Sized {
    fn get_geography_code(&self) -> String;

    fn group_by_area<T: 'static + PreProcessingTable>(
        data: Vec<impl PreProcessingTable + 'static>,
    ) -> Result<HashMap<String, Vec<Box<T>>>, DataLoadingError> {
        let mut buffer = HashMap::new();
        // Group the pre processing records, by output area
        for entry in data {
            let container = buffer
                .entry(entry.get_geography_code())
                .or_insert_with(Vec::new);
            let entry = Box::new(entry) as Box<dyn Any>;
            if let Ok(entry) = entry.downcast::<T>() {
                container.push(entry);
            }
        }
        Ok(buffer)
    }
}

/// This represents a transformed `PreProcessingTable` struct per output area
/// This is a container for the entire processed CSV
///
/// Should contain a hashmap of OutputArea Codes to TableEntries
pub trait TableEntry<T: 'static + PreProcessingTable>:
Debug + Sized + for<'a> TryFrom<&'a Vec<Box<T>>, Error=DataLoadingError>
{
    /// Returns the entire processed CSV per output area
    fn generate(
        data: Vec<impl PreProcessingTable + 'static>,
    ) -> Result<HashMap<String, Self>, DataLoadingError> {
        let mut grouped: HashMap<String, Vec<Box<T>>> = T::group_by_area(data)?;
        // Convert into Population Records
        let mut output = HashMap::new();
        for (code, records) in grouped.drain() {
            output.insert(code.to_string(), Self::try_from(&records)?);
        }
        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
pub enum CensusTableNames {
    OccupationCount,
    PopulationDensity,
    OutputAreaMap,
    ResidentialAreaVsWorkplaceArea,
}

impl CensusTableNames {
    /// Returns the filenames for tables stored on disk
    pub fn get_filename<'a>(&self) -> &'a str {
        match &self {
            CensusTableNames::PopulationDensity => "ks101ew_population_144.csv",
            CensusTableNames::OccupationCount => "ks608uk_occupation_count_NM_1518_1.csv",
            CensusTableNames::OutputAreaMap => {
                "data/census_map_areas/England_oa_2011/england_oa_2011.shp"
            }
            CensusTableNames::ResidentialAreaVsWorkplaceArea => {
                "wf01bew_residential_vs_workplace_NM_1228_1.csv"
            }
        }
    }
    /// Returns the api code for table
    pub fn get_api_code<'a>(&self) -> &'a str {
        match &self {
            CensusTableNames::PopulationDensity => "NM_144_1",
            CensusTableNames::OccupationCount => "NM_1518_1",
            CensusTableNames::OutputAreaMap => {
                "data/census_map_areas/England_oa_2011/england_oa_2011.shp"
            }
            CensusTableNames::ResidentialAreaVsWorkplaceArea => "NM_1228_1",
        }
    }
    /// The columns to retrieve from the API
    pub fn get_required_columns<'a>(&self) -> Option<&'a str> {
        match &self {
            CensusTableNames::OccupationCount => { None }
            CensusTableNames::PopulationDensity => { Some("GEOGRAPHY_NAME,GEOGRAPHY_TYPE,RURAL_URBAN_NAME,RURAL_URBAN_TYPECODE,CELL_NAME,MEASURES_NAME,OBS_VALUE,OBS_STATUS,RECORD_OFFSET,RECORD_COUNT") }
            CensusTableNames::OutputAreaMap => { Some("GEOGRAPHY_NAME,GEOGRAPHY_TYPE,CELL_NAME,MEASURES_NAME,OBS_VALUE,OBS_STATUS,RECORD_OFFSET,RECORD_COUNT") }
            CensusTableNames::ResidentialAreaVsWorkplaceArea => { Some("CURRENTLY_RESIDING_IN_CODE,PLACE_OF_WORK_TYPE,PLACE_OF_WORK_NAME,OBS_VALUE,RECORD_OFFSET,RECORD_COUNT") }
        }
    }
}

impl TryFrom<String> for CensusTableNames {
    type Error = DataLoadingError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(serde_plain::from_str(&value)?)
    }
}

impl TryFrom<&str> for CensusTableNames {
    type Error = DataLoadingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(serde_plain::from_str(value)?)
    }
}
