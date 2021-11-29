use rand::{Rng, RngCore};
use uuid::Uuid;

use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::models::building::BuildingCode;

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
    /// The hour which they go to work
    start_working_hour: u8,
    /// The hour which they leave to work
    end_working_hour: u8,
    /// The building the Citizen is currently at
    pub current_position: BuildingCode,
    /// Disease Status
    pub disease_status: DiseaseStatus,
}

impl Citizen {
    /// Generates a new Citizen with a random ID
    pub fn new(
        household_code: BuildingCode,
        workplace_code: BuildingCode,
        work_type: WorkType,
    ) -> Citizen {
        Citizen {
            id: Uuid::new_v4(),
            household_code: household_code.clone(),
            workplace_code,
            work_type,
            start_working_hour: 9,
            end_working_hour: 17,
            current_position: household_code,
            disease_status: DiseaseStatus::Susceptible,
        }
    }
    /// Returns the ID of this Citizen
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn execute_time_step(&mut self, current_hour: u8, disease: &DiseaseModel) {
        self.disease_status = DiseaseStatus::execute_time_step(&self.disease_status, disease);

        match current_hour {
            hour if hour == self.start_working_hour => {
                self.current_position = self.workplace_code.clone();
            }
            hour if hour == self.end_working_hour => {
                self.current_position = self.household_code.clone();
            }
            _ => {}
        }
    }
    /// Registers a new exposure to this citizen
    pub fn expose(&mut self, disease_model: &DiseaseModel, rng: &mut dyn RngCore) -> bool {
        if self.disease_status == DiseaseStatus::Susceptible
            && rng.gen::<f64>() < disease_model.exposure_chance
        {
            self.disease_status = DiseaseStatus::Exposed(0);
            return true;
        }
        false
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
