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
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};

use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use uuid::Uuid;

use load_census_data::tables::employment_densities::EmploymentDensities;
use load_census_data::tables::occupation_count::OccupationType;
use load_census_data::tables::population_and_density_per_output_area::AreaClassification;

use crate::error::Error;

/// This is used to represent a building location
///
/// It utilises:
/// * An `OutputArea` - for broad location in the country,
/// * An `AreaClassification` for differentiating between (Rural, Urban, Etc),
/// * A  `Uuid` for a unique building identifier
#[derive(Clone, Debug, Serialize)]
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

    /// Creates a new Building Code, but in the same Output Area and Area Type as the given BuildingCode
    pub(crate) fn new_from(other: BuildingCode) -> Self {
        BuildingCode {
            output_area_code: other.output_area_code.to_string(),
            area_type: other.area_type,
            building_id: Default::default(),
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
        self.output_area_code == other.output_area_code && self.building_id.eq(&other.building_id)
    }
}

impl Eq for BuildingCode {}

/// This represents a home for Citizens
///
/// Has an AreaCode for referencing it, and a list of Citizen ID's that live here
pub trait Building: Display + Debug {
    /// Creates a new building at the given location, with the specified type
    //fn new(building_code: BuildingCode) -> Self;

    /// Adds the new citizen to this building
    fn add_citizen(&mut self, citizen_id: Uuid) -> Result<(), Error>;
    /// Returns the AreaCode where this building is located
    fn building_code(&self) -> &BuildingCode;
    /// Returns a list of ids of occupants that are here
    fn occupants(&self) -> &Vec<Uuid>;
    fn as_any(&self) -> &dyn Any;
}

impl Serialize for dyn Building {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let raw = self.as_any();
        if let Some(workplace) = raw.downcast_ref::<Workplace>() {
            return workplace.serialize(serializer);
        }
        if let Some(household) = raw.downcast_ref::<Household>() {
            return household.serialize(serializer);
        }
        Err(serde::ser::Error::custom("Unknown building type!"))
    }
}

#[derive(Debug, Serialize)]
pub struct Household {
    /// This is unique to the specific output area - ~250 households
    building_code: BuildingCode,
    /// A list of all the ID's of citizens who are at this building
    occupants: Vec<Uuid>,
}

impl Household {
    pub(crate) fn new(building_code: BuildingCode) -> Self {
        Household {
            building_code,
            occupants: Vec::new(),
        }
    }
}

impl Building for Household {
    fn add_citizen(&mut self, citizen_id: Uuid) -> Result<(), Error> {
        self.occupants.push(citizen_id);
        Ok(())
    }

    fn building_code(&self) -> &BuildingCode {
        &self.building_code
    }

    fn occupants(&self) -> &Vec<Uuid> {
        &self.occupants
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

impl Display for Household {
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

#[derive(Debug, Serialize)]
pub struct Workplace {
    /// This is unique to the specific output area - ~250 households
    building_code: BuildingCode,
    /// A list of all the ID's of citizens who are at this building
    occupants: Vec<Uuid>,
    floor_space: u16,
    workplace_occupation_type: OccupationType,
}

impl Workplace {
    pub fn new(
        building_code: BuildingCode,
        floor_space: u16,
        occupation_type: OccupationType,
    ) -> Self {
        Workplace {
            building_code,
            occupants: Vec::new(),
            floor_space,
            workplace_occupation_type: occupation_type,
        }
    }
    pub fn is_at_capacity(&self) -> bool {
        (self.floor_space as usize)
            <= (self.occupants.len()
            * EmploymentDensities::get_size_for_occupation(self.workplace_occupation_type)
            as usize)
    }
}

impl Building for Workplace {
    fn add_citizen(&mut self, citizen_id: Uuid) -> Result<(), Error> {
        if self.is_at_capacity() {
            return Err(Error::Default {
                message: "Workplace has full occupancy, so cannot add new occupant".to_string(),
            });
        }
        self.occupants.push(citizen_id);
        Ok(())
    }

    fn building_code(&self) -> &BuildingCode {
        &self.building_code
    }

    fn occupants(&self) -> &Vec<Uuid> {
        &self.occupants
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

impl Display for Workplace {
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
