use std::fmt::{Display, Formatter};

use uuid::Uuid;

use crate::BuildingCode;

impl Display for BuildingCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Output Area: {}, Area Type: {:?}, Building ID: {}", self.output_area_code, self.area_type, self.building_id)
    }
}

/// This represents a home for Citizens
///
/// Has an AreaCode for referencing it, and a list of Citizen ID's that live here
pub struct Household {
    /// This is unique to the specific output area - ~250 households
    household_code: BuildingCode,
    /// A list of all the ID's of citizens who live at his household
    residents: Vec<Uuid>,
}

impl Household {
    /// Builds a new household at the given location
    pub fn new(household_code: BuildingCode) -> Household {
        Household { household_code, residents: Vec::new() }
    }
    /// Adds the new citizen to this household
    pub fn add_citizen(&mut self, citizen_id: Uuid) {
        self.residents.push(citizen_id);
    }
    /// Returns the AreaCode where this household is located
    pub fn household_code(&self) -> &BuildingCode {
        &self.household_code
    }
    /// Returns a list of ids of residents that live here
    pub fn residents(&self) -> &Vec<Uuid> {
        &self.residents
    }
}

impl Display for Household {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Household: {}, with {} residents", self.household_code, self.residents.len())
    }
}