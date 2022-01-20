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
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

use geo::Point;
use log::error;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use osm_data::RawBuilding;

use crate::config::MIN_WORKPLACE_OCCUPANT_COUNT;
use crate::error::SimError;
use crate::models::citizen::{CitizenID, OccupationType};
use crate::models::get_density_for_occupation;
use crate::models::output_area::OutputAreaID;

/// The minimum floor size a building can have
pub const MINIMUM_FLOOR_SPACE_SIZE: u32 = 2000;

/// A wrapper for all building types, for easier use in Hashmaps
///
/// Each element contains
#[derive(Clone, Debug, Serialize, Hash, Eq, PartialEq)]
pub enum BuildingType {
    Household,
    Workplace,
    School,
}

/// This is used to represent a building location
///
/// It utilises:
/// * An `OutputArea` - for broad location in the country,
/// * An `AreaClassification` for differentiating between (Rural, Urban, Etc),
/// * A  `Uuid` for a unique building identifier
#[derive(Clone, Debug, Serialize, Hash, PartialEq, Eq)]
pub struct BuildingID {
    output_area_id: OutputAreaID,
    building_id: uuid::Uuid,
    building_type: BuildingType,
}

impl BuildingID {
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
    pub fn new(output_area_id: OutputAreaID, building_type: BuildingType) -> BuildingID {
        BuildingID {
            output_area_id,
            building_id: Uuid::new_v4(),
            building_type,
        }
    }

    /// Creates a new Building Code, but in the same Output Area and Area Type as the given BuildingCode
    pub(crate) fn new_from(other: BuildingID) -> Self {
        BuildingID {
            output_area_id: other.output_area_id.clone(),
            building_id: Default::default(),
            building_type: other.building_type,
        }
    }
    /// Returns the `OutputArea` code
    pub fn output_area_code(&self) -> OutputAreaID {
        self.output_area_id.clone()
    }
    /// Returns the unique ID of this `BuildingCode`
    pub fn building_id(&self) -> Uuid {
        self.building_id
    }
}

impl Display for BuildingID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Output Area: {}, Type: {:?}, Building ID: {}",
            self.output_area_id, self.building_type, self.building_id
        )
    }
}

/// This represents a home for Citizens
///
/// Has an AreaCode for referencing it, and a list of Citizen ID's that live here
pub trait Building: Display + Debug {
    /// Creates a new building at the given location, with the specified type
    //fn new(building_code: BuildingCode) -> Self;

    /// Adds the new citizen to this building
    fn add_citizen(&mut self, citizen_id: CitizenID) -> Result<(), SimError>;
    /// Returns the AreaCode where this building is located
    fn id(&self) -> &BuildingID;
    /// Returns a list of ids of occupants that are here
    fn occupants(&self) -> Vec<CitizenID>;
    fn as_any(&self) -> &dyn Any;
    /// Returns the location of the building
    fn get_location(&self) -> geo_types::Point<i32>;
    /// Returns a list of Citizens that would be exposed, if the given Citizen is infected
    fn apply_exposure(&self, infected_citizen: CitizenID) -> Vec<CitizenID>;
}

impl Serialize for dyn Building {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
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

#[derive(Clone, Debug, Serialize)]
pub struct Household {
    /// This is unique to the specific output area - ~250 households
    building_code: BuildingID,
    /// A list of all the ID's of citizens who are at this building
    occupants: Vec<CitizenID>,
    location: geo_types::Point<i32>,
}

impl Household {
    pub(crate) fn new(building_code: BuildingID, location: geo_types::Point<i32>) -> Self {
        Household {
            building_code,
            occupants: Vec::new(),
            location,
        }
    }
}

impl Building for Household {
    fn add_citizen(&mut self, citizen_id: CitizenID) -> Result<(), SimError> {
        self.occupants.push(citizen_id);
        Ok(())
    }

    fn id(&self) -> &BuildingID {
        &self.building_code
    }

    fn occupants(&self) -> Vec<CitizenID> {
        self.occupants.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn get_location(&self) -> Point<i32> {
        self.location
    }

    fn apply_exposure(&self, infected_citizen: CitizenID) -> Vec<CitizenID> {
        let mut exposed = self.occupants();
        exposed.retain(|id| *id != infected_citizen);
        exposed
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

#[derive(Clone, Debug, Serialize)]
pub struct Workplace {
    /// This is unique to the specific output area - ~250 households
    building_code: BuildingID,
    /// A list of all the ID's of citizens who are at this building
    occupants: Vec<CitizenID>,
    floor_space: u32,
    workplace_occupation_type: OccupationType,
    location: geo_types::Point<i32>,
}

impl Workplace {
    pub fn new(
        building_code: BuildingID,
        raw_building: RawBuilding,
        occupation_type: OccupationType,
    ) -> Self {
        Workplace {
            building_code,
            occupants: Vec::new(),
            floor_space: (raw_building.size() as u32).max(MINIMUM_FLOOR_SPACE_SIZE),
            workplace_occupation_type: occupation_type,
            location: raw_building.center(),
        }
    }
    fn max_occupant_count(&self) -> u32 {
        ((self.floor_space) / get_density_for_occupation(self.workplace_occupation_type)).max(MIN_WORKPLACE_OCCUPANT_COUNT)
    }
    pub fn is_at_capacity(&self) -> bool {
        self.max_occupant_count() <= (self.occupants.len() as u32)
    }
}

impl Building for Workplace {
    fn add_citizen(&mut self, citizen_id: CitizenID) -> Result<(), SimError> {
        if self.is_at_capacity() {
            return Err(SimError::Default {
                message: format!("Workplace of type {:?} has full occupancy ({} Citizens out of {}), so cannot add new occupant, with floor space {}", self.workplace_occupation_type, self.occupants.len(), self.max_occupant_count(), self.floor_space),
            });
        }
        self.occupants.push(citizen_id);
        Ok(())
    }

    fn id(&self) -> &BuildingID {
        &self.building_code
    }

    fn occupants(&self) -> Vec<CitizenID> {
        self.occupants.clone()
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn get_location(&self) -> Point<i32> {
        self.location
    }
    fn apply_exposure(&self, infected_citizen: CitizenID) -> Vec<CitizenID> {
        let mut exposed = self.occupants();
        exposed.retain(|id| *id != infected_citizen);
        exposed
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

pub struct Class {
    students: Vec<CitizenID>,
    teachers: Vec<CitizenID>,
}

impl Class {
    /// Returns all students and the teacher in the class
    pub fn get_participants(&self) -> Vec<CitizenID> {
        let mut participants: Vec<CitizenID> = self.students.iter().cloned().collect();
        for teacher in &self.teachers {
            participants.push(teacher.clone())
        }
        participants
    }
}

pub struct School {
    building_code: BuildingID,
    location: geo_types::Point<i32>,
    /// A class consists 20/30 students and a teacher?
    classes: Vec<Class>,
    /// The class index the teacher/student belongs to
    occupant_to_class: HashMap<CitizenID, usize>,
}

impl School {
    pub fn with_students_and_teachers(building_id: BuildingID, building: RawBuilding, mut students: Vec<Vec<CitizenID>>, teachers: Vec<CitizenID>) -> School {
        if teachers.len() < 1 {
            panic!("Cannot have a school without any teachers!")
        }


        let mut teachers_per_age_group = (students.len() as f64) / (teachers.len() as f64);
        // Merge age groups, until there are enough teachers
        while teachers_per_age_group < 1.0 {
            let mut new_students = Vec::with_capacity(students.len() / 2);
            let mut current_index = 0;
            for age_group in students {
                if current_index % 2 == 0 {
                    new_students.push(age_group);
                } else {
                    new_students.last_mut().unwrap().extend(age_group);
                }
                current_index += 1;
            }
            students = new_students;
            teachers_per_age_group = (students.len() as f64) / (teachers.len() as f64);
        }

        // Allocate students/teachers into classes
        let mut occupant_to_class = HashMap::with_capacity(students.len());
        let mut class_index = 0;

        let mut teachers = teachers.into_iter();
        let mut classes: Vec<Class> = Vec::new();

        let mut teachers_allocated = 0;
        let mut teachers_should_be_allocated = 0.0;

        for age_group in students {
            let mut new_classes = Vec::new();

            let age_group = age_group.into_iter();
            let class_size = age_group.len() / teachers_per_age_group.floor() as usize;

            for class in age_group.as_slice().chunks(class_size) {
                let teacher = teachers.next().expect("Ran out of teachers!");
                for student in class {
                    occupant_to_class.insert(*student, class_index);
                }
                occupant_to_class.insert(teacher, class_index);
                class_index += 1;
                new_classes.push(Class {
                    students: class.to_vec(),
                    teachers: vec![teacher],
                });
                teachers_allocated += 1;
                teachers_should_be_allocated += teachers_per_age_group;
            }
            // Add any missing teachers
            let mut age_group_class_index = 0;
            while teachers_allocated < teachers_should_be_allocated as usize {
                let teacher = teachers.next().expect("Ran out of teachers!");
                new_classes.get_mut(age_group_class_index).unwrap().teachers.push(teacher);
                occupant_to_class.insert(teacher, classes.len() + age_group_class_index);

                teachers_allocated += 1;
                age_group_class_index += 1;
                if age_group_class_index == new_classes.len() {
                    age_group_class_index = 0;
                }
            }
            classes.extend(new_classes);
        }
        School {
            building_code: building_id,
            location: building.center(),
            classes,
            occupant_to_class,
        }
    }
    pub fn classes(&self) -> &Vec<Class> {
        &self.classes
    }
}

impl Display for School {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "School: {},\tWith  {} classes\tLocated at: {:?} ", self.id(), self.classes.len(), self.location)
    }
}

impl Debug for School {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Building for School {
    fn add_citizen(&mut self, _: CitizenID) -> Result<(), SimError> {
        panic!("Schools can only have citizens added at creation!");
    }

    fn id(&self) -> &BuildingID {
        &self.building_code
    }

    fn occupants(&self) -> Vec<CitizenID> {
        self.classes.iter().flat_map(|class| class.get_participants()).collect()
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn get_location(&self) -> Point<i32> {
        self.location
    }
    fn apply_exposure(&self, infected_citizen: CitizenID) -> Vec<CitizenID> {
        let class_index = match self.occupant_to_class.get(&infected_citizen) {
            Some(class_index) => class_index,
            None => {
                error!("Citizen does not belong to a class!");
                return Vec::new();
            }
        };
        let mut exposed = self.classes[*class_index].get_participants();
        exposed.retain(|id| *id != infected_citizen);
        exposed
    }
}


#[cfg(test)]
mod tests {
    use geo::prelude::Area;
    use geo_types::Geometry::LineString;
    use geo_types::Polygon;
    use strum::IntoEnumIterator;

    use load_census_data::osm_parsing::{
        BuildingBoundaryID, convert_polygon_to_float, RawBuilding, TagClassifiedBuilding,
    };
    use load_census_data::tables::employment_densities::EmploymentDensities;
    use load_census_data::tables::occupation_count::OccupationType;
    use osm_data::{BuildingBoundaryID, convert_polygon_to_float, RawBuilding, TagClassifiedBuilding};

    use crate::models::building::{
        Building, BuildingID, BuildingType, MINIMUM_FLOOR_SPACE_SIZE, Workplace,
    };
    use crate::models::citizen::{CitizenID, OccupationType};
    use crate::models::output_area::OutputAreaID;

    #[test]
    fn minimum_one_occupant() {
        let building_size = geo_types::Polygon::new(
            geo_types::LineString::from(vec![(0, 0), (100, 0), (100, 2), (0, 2), (0, 0)]),
            vec![],
        );
        let id = BuildingID::new(
            OutputAreaID::from_code("a".to_string()),
            BuildingType::Workplace,
        );
        let raw = RawBuilding::new(
            TagClassifiedBuilding::WorkPlace,
            &building_size,
            BuildingBoundaryID::default(),
        )
            .unwrap();
        let float: Polygon<f64> = convert_polygon_to_float(&building_size);
        assert_eq!(float.unsigned_area(), MINIMUM_FLOOR_SPACE_SIZE as f64);
        for occupation_type in OccupationType::iter() {
            println!("Testing: {:?}", occupation_type);
            let mut workplace = Workplace::new(id.clone(), raw, occupation_type);
            assert!(
                EmploymentDensities::get_density_for_occupation(occupation_type)
                    < workplace.floor_space
            );
            assert!(0 < workplace.max_occupant_count());
            assert!(workplace.add_citizen(CitizenID::default()).is_ok());
        }
    }

    #[test]
    fn minimum_size() {
        let building_size = geo_types::Polygon::new(
            geo_types::LineString::from(vec![(0, 0), (100, 0), (100, 2), (0, 2), (0, 0)]),
            vec![],
        );
        let id = BuildingID::new(
            OutputAreaID::from_code("a".to_string()),
            BuildingType::Workplace,
        );
        let raw = RawBuilding::new(
            TagClassifiedBuilding::WorkPlace,
            &building_size,
            BuildingBoundaryID::default(),
        )
            .unwrap();
        let float: Polygon<f64> = convert_polygon_to_float(&building_size);
        assert!(float.unsigned_area() < MINIMUM_FLOOR_SPACE_SIZE as f64);
        let mut workplace = Workplace::new(id.clone(), raw, OccupationType::All);
        assert!(MINIMUM_FLOOR_SPACE_SIZE <= workplace.floor_space);
    }
}
