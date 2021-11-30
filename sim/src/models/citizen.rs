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

use rand::{Rng, RngCore};
use uuid::Uuid;

use load_census_data::tables::occupation_count::OccupationType;

use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::models::building::BuildingCode;

/// This is used to represent a single Citizen in the simulation
#[derive(Debug)]
pub struct Citizen {
    /// A unique identifier for this Citizen
    id: Uuid,
    /// The building they reside at (home)
    household_code: BuildingCode,
    /// The place they work at
    workplace_code: BuildingCode,
    occupation: OccupationType,
    /// The hour which they go to work
    start_working_hour: u32,
    /// The hour which they leave to work
    end_working_hour: u32,
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
        occupation_type: OccupationType,
    ) -> Citizen {
        Citizen {
            id: Uuid::new_v4(),
            household_code: household_code.clone(),
            workplace_code,
            occupation: occupation_type,
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

    pub fn execute_time_step(&mut self, current_hour: u32, disease: &DiseaseModel) {
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
    pub fn set_workplace_code(&mut self, workplace_code: BuildingCode) {
        self.workplace_code = workplace_code;
    }
    pub fn occupation(&self) -> OccupationType {
        self.occupation
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
