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

use std::collections::HashMap;
use std::fmt::Debug;

use serde::de::DeserializeOwned;

use crate::parsing_error::CensusError;

pub mod employment_densities;
pub mod occupation_count;
pub mod population_and_density_per_output_area;

/// This is used to load in a CSV file, and each row corresponds to one struct
pub trait PreProcessingTable: Debug + DeserializeOwned + Sized {}

/// This represents a transformed `PreProcessingTable` struct per output area
/// This is a container for the entire processed CSV
///
/// Should contain a hashmap of OutputArea Codes to TableEntries
pub trait TableEntry: Debug + Sized {
    /// Returns the entire processed CSV per output area
    fn generate(
        data: Vec<impl PreProcessingTable + 'static>,
    ) -> Result<HashMap<String, Self>, CensusError>;
}

pub enum CensusTableNames {
    OccupationCount,
    PopulationDensity,
    OutputAreaMap,
    WorkLocations,
}

impl CensusTableNames {
    ///
    pub fn get_filename<'a>(&self) -> &'a str {
        match &self {
            CensusTableNames::PopulationDensity => "data/tables/york_population_144.csv",
            CensusTableNames::OccupationCount => "data/tables/york_occupation_count_ks608uk.csv",
            CensusTableNames::OutputAreaMap => {
                "data/census_map_areas/England_oa_2011/england_oa_2011.shp"
            }
            CensusTableNames::WorkLocations => "data/tables/york_locations_of_work_wf01bew.csv",
        }
    }
}
