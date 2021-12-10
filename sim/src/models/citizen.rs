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

use std::fmt::{Debug, Display, Formatter};

use log::warn;
use rand::{Rng, RngCore};
use serde::Serialize;
use uuid::Uuid;

use load_census_data::tables::occupation_count::OccupationType;

use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::interventions::MaskStatus;
use crate::models::building::BuildingCode;

/// This is used to represent a single Citizen in the simulation
#[derive(Debug, Serialize)]
pub struct Citizen {
    /// A unique identifier for this Citizen
    id: Uuid,
    /// The building they reside at (home)
    pub household_code: BuildingCode,
    /// The place they work at
    pub workplace_code: BuildingCode,
    occupation: OccupationType,
    /// The hour which they go to work
    start_working_hour: u32,
    /// The hour which they leave to work
    end_working_hour: u32,
    /// The building the Citizen is currently at
    pub current_position: BuildingCode,
    /// Disease Status
    pub disease_status: DiseaseStatus,
    /// Whether this Citizen wears a mask
    pub is_mask_compliant: bool,
}

impl Citizen {
    /// Generates a new Citizen with a random ID
    pub fn new(
        household_code: BuildingCode,
        workplace_code: BuildingCode,
        occupation_type: OccupationType,
        is_mask_compliant: bool,
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
            is_mask_compliant,
        }
    }
    /// Returns the ID of this Citizen
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn execute_time_step(
        &mut self,
        current_hour: u32,
        disease: &DiseaseModel,
        lockdown_enabled: bool,
    ) {
        self.disease_status = DiseaseStatus::execute_time_step(&self.disease_status, disease);
        if !lockdown_enabled {
            match current_hour % 24 {
                hour if hour == self.start_working_hour => {
                    self.current_position = self.workplace_code.clone();
                }
                hour if hour == self.end_working_hour => {
                    self.current_position = self.household_code.clone();
                }
                _ => {}
            }
        }
    }
    /// Registers a new exposure to this citizen
    pub fn expose(
        &mut self,
        disease_model: &DiseaseModel,
        mask_status: &MaskStatus,
        rng: &mut dyn RngCore,
    ) -> bool {
        let mask_status = if self.is_mask_compliant {
            &MaskStatus::None(0)
        } else {
            mask_status
        };
        let exposure_chance = disease_model.get_exposure_chance(
            self.disease_status == DiseaseStatus::Vaccinated,
            mask_status,
        );

        if self.disease_status == DiseaseStatus::Susceptible && rng.gen::<f64>() < exposure_chance {
            self.disease_status = DiseaseStatus::Exposed(0);
            return true;
        }
        false
    }
    pub fn set_workplace_code(&mut self, workplace_code: BuildingCode) {
        self.workplace_code = workplace_code.clone();
    }
    pub fn occupation(&self) -> OccupationType {
        self.occupation
    }
}

impl Display for Citizen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Citizen {} Has Disease Status {}, Is Currently Located At {}, Resides at {}, Works at {}", self.id, self.disease_status, self.current_position, self.household_code, self.workplace_code)
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
/*
#[cfg(test)]
mod tests {
    use load_census_data::tables::occupation_count::OccupationType;
    use load_census_data::tables::population_and_density_per_output_area::AreaClassification;
    use crate::disease::DiseaseModel;
    use crate::models::building::BuildingCode;
    use crate::models::citizen::Citizen;

    pub fn check_citizen_schedule() {
        let work = BuildingCode::new("A".to_string(), AreaClassification::UrbanCity);
        let home = BuildingCode::new("B".to_string(), AreaClassification::UrbanCity);
        let mut citizen = Citizen::new(home, work, OccupationType::Sales);
        let disease = DiseaseModel::covid();
        for hour in 0..100 {
            citizen.execute_time_step(hour, &disease);
            if citizen.w
        }
    }
}*/
