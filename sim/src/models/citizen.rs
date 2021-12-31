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

use lazy_static::lazy_static;
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::RngCore;
use serde::Serialize;
use uuid::Uuid;

use load_census_data::tables::occupation_count::OccupationType;

use crate::config::PUBLIC_TRANSPORT_PERCENTAGE;
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::interventions::MaskStatus;
use crate::models::building::BuildingID;
use crate::models::output_area::OutputAreaID;

lazy_static! {
    /// This is a random uniform distribution, for fast random generation
    static ref RANDOM_DISTRUBUTION: Uniform<f64> =Uniform::new_inclusive(0.0, 1.0);
}
/// Calculates the binomial distribution, with at least one success
fn binomial(probability: f64, n: u8) -> f64 {
    1.0 - (1.0 - probability).powf(n as f64)
}

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, Serialize)]
pub struct CitizenID {
    id: Uuid,
}

impl CitizenID {
    pub fn id(&self) -> Uuid {
        self.id
    }
}

impl Default for CitizenID {
    fn default() -> Self {
        CitizenID { id: Uuid::new_v4() }
    }
}

impl Display for CitizenID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Citizen ID: {}", self.id)
    }
}

/// This is used to represent a single Citizen in the simulation
#[derive(Debug, Serialize, Clone)]
pub struct Citizen {
    /// A unique identifier for this Citizen
    id: CitizenID,
    /// The building they reside at (home)
    pub household_code: BuildingID,
    /// The place they work at
    pub workplace_code: BuildingID,
    occupation: OccupationType,
    /// The hour which they go to work
    start_working_hour: u32,
    /// The hour which they leave to work
    end_working_hour: u32,
    /// The building the Citizen is currently at
    ///
    /// Note that it will be the starting point, if Citizen is using Public Transport
    pub current_building_position: BuildingID,
    /// Disease Status
    pub disease_status: DiseaseStatus,
    /// Whether this Citizen wears a mask
    pub is_mask_compliant: bool,
    pub uses_public_transport: bool,
    /// The source and destination for a Citizen on Transport this time step
    pub on_public_transport: std::option::Option<(OutputAreaID, OutputAreaID)>,
}

impl Citizen {
    /// Generates a new Citizen with a random ID
    pub fn new(
        household_code: BuildingID,
        workplace_code: BuildingID,
        occupation_type: OccupationType,
        is_mask_compliant: bool,
        rng: &mut dyn RngCore,
    ) -> Citizen {
        Citizen {
            id: CitizenID::default(),
            household_code: household_code.clone(),
            workplace_code,
            occupation: occupation_type,
            start_working_hour: 9,
            end_working_hour: 17,
            current_building_position: household_code,
            disease_status: DiseaseStatus::Susceptible,
            is_mask_compliant,
            uses_public_transport: RANDOM_DISTRUBUTION.sample(rng) < PUBLIC_TRANSPORT_PERCENTAGE,
            on_public_transport: None,
        }
    }
    /// Returns the ID of this Citizen
    pub fn id(&self) -> CitizenID {
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
                // Travelling home to work
                hour if hour == self.start_working_hour - 1 && self.uses_public_transport => {
                    self.on_public_transport = Some((
                        self.household_code.output_area_code(),
                        self.workplace_code.output_area_code(),
                    ))
                }
                // Starts work
                hour if hour == self.start_working_hour => {
                    self.current_building_position = self.workplace_code.clone();
                    self.on_public_transport = None;
                }
                // Travelling work to home
                hour if hour == self.end_working_hour - 1 && self.uses_public_transport => {
                    self.on_public_transport = Some((
                        self.workplace_code.output_area_code(),
                        self.household_code.output_area_code(),
                    ))
                }
                // Finish work, goes home
                hour if hour == self.end_working_hour => {
                    self.current_building_position = self.household_code.clone();
                    self.on_public_transport = None;
                }
                _ => {
                    self.on_public_transport = None;
                }
            }
        }
    }
    /// Registers a new exposure to this citizen
    ///
    /// # Paramaters
    /// exposure_total: The amount of the exposures that occured in this time step
    pub fn expose(
        &mut self,
        exposure_total: usize,
        disease_model: &DiseaseModel,
        mask_status: &MaskStatus,
        rng: &mut dyn RngCore,
    ) -> bool {
        let mask_status = if self.is_mask_compliant {
            &MaskStatus::None(0)
        } else {
            mask_status
        };
        let exposure_chance = binomial(
            disease_model.get_exposure_chance(
                self.disease_status == DiseaseStatus::Vaccinated,
                mask_status,
                self.is_mask_compliant && self.on_public_transport.is_some(),
            ),
            exposure_total as u8,
        );
        if self.disease_status == DiseaseStatus::Susceptible
            && RANDOM_DISTRUBUTION.sample(rng) < exposure_chance
        {
            self.disease_status = DiseaseStatus::Exposed(0);
            return true;
        }
        false
    }
    pub fn set_workplace_code(&mut self, workplace_code: BuildingID) {
        self.workplace_code = workplace_code;
    }
    pub fn occupation(&self) -> OccupationType {
        self.occupation
    }

    pub fn is_susceptible(&self) -> bool {
        self.disease_status == DiseaseStatus::Susceptible
    }
    pub fn is_infected(&self) -> bool {
        matches!(self.disease_status, DiseaseStatus::Infected(_))
    }
}

impl Display for Citizen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Citizen {} Has Disease Status {}, Is Currently Located At {}, Resides at {}, Works at {}", self.id, self.disease_status, self.current_building_position, self.household_code, self.workplace_code)
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
