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

use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

use uuid::Uuid;

use load_census_data::tables::population_and_density_per_output_area::AreaClassification;

/// This is used to represent a building location
///
/// It utilises:
/// * An `OutputArea` - for broad location in the country,
/// * An `AreaClassification` for differentiating between (Rural, Urban, Etc),
/// * A  `Uuid` for a unique building identifier
#[derive(Clone)]
pub struct BuildingCode {
    output_area_code: String,
    area_type: AreaClassification,
    building_id: uuid::Uuid,
}

impl BuildingCode {
    /// Generates a new `BuildingCode` in the given position, with a new random building ID (`Uuid`)
    ///
    /// # Example
    /// ```
    /// use census_geography::BuildingCode;
    /// use load_census_data::table_144_enum_values::AreaClassification;
    ///
    /// let output_area = String::from("1234");
    /// let area_type = AreaClassification::UrbanCity;
    ///
    /// let building_code = BuildingCode::new(output_area, area_type);
    ///
    /// assert_eq!(building_code.output_area_code(), output_area);
    /// assert_eq!(building_code.area_type(), area_type);
    ///
    /// ```
    pub fn new(output_code: String, area_type: AreaClassification) -> BuildingCode {
        BuildingCode {
            output_area_code: output_code,
            area_type,
            building_id: Uuid::new_v4(),
        }
    }
    /// Returns the `OutputArea` code
    pub fn output_area_code(&self) -> String {
        String::from(&self.output_area_code)
    }
    /// Returns the type of area this building is located in
    pub fn area_type(&self) -> AreaClassification {
        self.area_type
    }
    /// Returns the unique ID of this `BuildingCode`
    pub fn building_id(&self) -> Uuid {
        self.building_id
    }
}

impl Display for BuildingCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Output Area: {}, Area Type: {:?}, Building ID: {}",
            self.output_area_code, self.area_type, self.building_id
        )
    }
}

impl Hash for BuildingCode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.building_id.hash(state);
        self.output_area_code.hash(state)
    }
}

impl PartialEq<Self> for BuildingCode {
    fn eq(&self, other: &Self) -> bool {
        self.output_area_code == other.output_area_code && self.building_id == other.building_id
    }
}

impl Eq for BuildingCode {}

/// The types of buildings that can exist
pub enum BuildingType {
    /// A place where Citizens reside at night
    Household,
    /// A place where people work
    Workplace,
}

/// This represents a home for Citizens
///
/// Has an AreaCode for referencing it, and a list of Citizen ID's that live here
pub struct Building {
    /// What the function of this building is
    building_type: BuildingType,
    /// This is unique to the specific output area - ~250 households
    building_code: BuildingCode,
    /// A list of all the ID's of citizens who are at this building
    occupants: Vec<Uuid>,
}

impl Building {
    /// Creates a new building at the given location, with the specified type
    pub fn new(building_type: BuildingType, building_code: BuildingCode) -> Building {
        Building {
            building_type,
            building_code,
            occupants: Vec::new(),
        }
    }
    /// Adds the new citizen to this building
    pub fn add_citizen(&mut self, citizen_id: Uuid) {
        self.occupants.push(citizen_id);
    }
    /// Returns the AreaCode where this building is located
    pub fn household_code(&self) -> &BuildingCode {
        &self.building_code
    }
    /// Returns a list of ids of occupants that are here
    pub fn occupants(&self) -> &Vec<Uuid> {
        &self.occupants
    }
}

impl Display for Building {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} Building at {}, with {} residents",
            self.building_code,
            self.building_code,
            self.occupants.len()
        )
    }
}
