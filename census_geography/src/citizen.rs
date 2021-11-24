use uuid::Uuid;

use crate::BuildingCode;

/// This is used to represent a single Citizen in the simulation
pub struct Citizen {
    /// A unique identifier for this Citizen
    id: Uuid,
    /// The building they reside at (home)
    household_code: BuildingCode,
    /// The place they work at
    workplace_code: BuildingCode,
    /// Their type of employment
    work_type: WorkType,
}

impl Citizen {
    /// Generates a new Citizen with a random ID
    pub fn new(household_code: BuildingCode, workplace_code: BuildingCode, work_type: WorkType) -> Citizen {
        Citizen {
            id: Uuid::new_v4(),
            household_code,
            workplace_code,
            work_type,
        }
    }
    /// Returns the ID of this Citizen
    pub fn id(&self) -> Uuid {
        self.id
    }
}

/// The type of employment a Citizen can have
#[derive(Debug)]
pub enum WorkType {
    Normal,
    Essential,
    Unemployed,
    Student,
    NA,
}