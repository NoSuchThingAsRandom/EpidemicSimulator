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

use std::convert::TryFrom;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};

use enum_map::Enum;
use lazy_static::lazy_static;
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::RngCore;
use serde::Serialize;
use strum_macros::EnumIter;
use uuid::Uuid;

use load_census_data::tables::occupation_count::RawOccupationType;

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

#[derive(Debug, Copy, Clone, Serialize)]
pub struct CitizenID {
    /// This is a global unique Citizen index
    global_index: u32,
    /// This is the local index, for lookup inside an Output Area
    local_index: u32,
    /// This is a randomised unique ID, to ensure unique hashes
    uuid_id: Uuid,
}

impl CitizenID {
    pub fn from_indexes(global_index: u32, local_index: u32) -> CitizenID {
        CitizenID {
            global_index,
            local_index,
            uuid_id: Uuid::new_v4(),
        }
    }

    pub fn global_index(&self) -> usize {
        self.global_index as usize
    }
    pub fn set_global_index(&mut self, global_index: u32) {
        self.global_index = global_index;
    }
    pub fn local_index(&self) -> usize {
        self.local_index as usize
    }
    pub fn set_local_index(&mut self, local_index: u32) {
        self.local_index = local_index;
    }
    pub fn uuid_id(&self) -> Uuid {
        self.uuid_id
    }
}

impl Default for CitizenID {
    fn default() -> Self {
        CitizenID { global_index: 0, local_index: 0, uuid_id: Uuid::new_v4() }
    }
}

impl Display for CitizenID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Citizen ID: {}", self.uuid_id)
    }
}

impl Hash for CitizenID {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.global_index.hash(state);
        self.uuid_id.hash(state);
    }
}

impl PartialEq for CitizenID {
    fn eq(&self, other: &Self) -> bool {
        self.global_index.eq(&other.global_index) && self.uuid_id.eq(&other.uuid_id)
    }
}

impl Eq for CitizenID {}

/// This is used to represent a single Citizen in the simulation
#[derive(Debug, Serialize, Clone)]
pub struct Citizen {
    /// A unique identifier for this Citizen
    id: CitizenID,
    /// The age of the Citizen in years
    pub age: u16,
    /// The building they reside at (home)
    pub household_code: BuildingID,
    /// The place they work at
    pub workplace_code: BuildingID,
    occupation: Occupation,
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
        citizen_id: CitizenID,
        household_code: BuildingID,
        workplace_code: BuildingID,
        age: u16,
        occupation: Occupation,
        is_mask_compliant: bool,
        rng: &mut dyn RngCore,
    ) -> Citizen {
        Citizen {
            id: citizen_id,
            age,
            household_code: household_code.clone(),
            workplace_code,
            occupation,
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
    ) -> Option<OutputAreaID> {
        let old_position = self.current_building_position.output_area_code();
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
        if self.current_building_position.output_area_code().eq(&old_position) {
            None
        } else {
            Some(self.current_building_position.output_area_code())
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
    /// Returns True if this Citizen is a student
    pub fn is_student(&self) -> bool {
        self.occupation == Occupation::Student
    }

    pub fn occupation(&self) -> Occupation {
        self.occupation
    }
    /// Attempts to return the detailed Occupation type, if it is available
    pub fn detailed_occupation(&self) -> Option<OccupationType> {
        match self.occupation() {
            Occupation::Normal { occupation } => { Some(occupation) }
            Occupation::Essential { occupation } => { Some(occupation) }
            Occupation::Unemployed => { None }
            Occupation::Student => { None }
        }
    }

    pub fn is_susceptible(&self) -> bool {
        self.disease_status == DiseaseStatus::Susceptible
    }
    pub fn is_infected(&self) -> bool {
        matches!(self.disease_status, DiseaseStatus::Infected(_))
    }
    pub fn set_local_index(&mut self, new_index: usize) {
        self.id.set_local_index(new_index as u32)
    }
}

impl Display for Citizen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Citizen {} Has Disease Status {}, Is Currently Located At {}, Resides at {}, Works at {}", self.id, self.disease_status, self.current_building_position, self.household_code, self.workplace_code)
    }
}

/// The type of employment a Citizen can have
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Hash)]
pub enum Occupation {
    /// The Citizen is part of the normal workforce, with a detailed classification
    Normal { occupation: OccupationType },
    /// The Citizen is classed as part of the "Essential" workforce
    Essential { occupation: OccupationType },
    /// The Citizen is unemployed (without a job) and stays at home
    Unemployed,
    /// The Citizen goes to school
    Student,
}

/// The detailed job type of a Citizen
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, EnumIter, Enum, Hash)]
pub enum OccupationType {
    Manager,
    Professional,
    Technical,
    Administrative,
    SkilledTrades,
    Caring,
    Sales,
    MachineOperatives,
    Teaching,
}

impl OccupationType {
    pub fn get_index(&self) -> usize {
        match self {
            OccupationType::Manager => { 0 }
            OccupationType::Professional => { 1 }
            OccupationType::Technical => { 2 }
            OccupationType::Administrative => { 3 }
            OccupationType::SkilledTrades => { 4 }
            OccupationType::Caring => { 5 }
            OccupationType::Sales => { 6 }
            OccupationType::MachineOperatives => { 7 }
            OccupationType::Teaching => { 8 }
        }
    }
}

impl TryFrom<RawOccupationType> for OccupationType {
    type Error = ();

    fn try_from(raw_occupation: RawOccupationType) -> Result<Self, Self::Error> {
        Ok(match raw_occupation {
            RawOccupationType::All => { return Err(()); }
            RawOccupationType::Managers => { OccupationType::Manager }
            RawOccupationType::Professional => { OccupationType::Professional }
            RawOccupationType::Technical => { OccupationType::Technical }
            RawOccupationType::Administrative => { OccupationType::Administrative }
            RawOccupationType::SkilledTrades => { OccupationType::SkilledTrades }
            RawOccupationType::Caring => { OccupationType::Caring }
            RawOccupationType::Sales => { OccupationType::Sales }
            RawOccupationType::MachineOperatives => { OccupationType::MachineOperatives }
            RawOccupationType::Teaching => { OccupationType::Teaching }
        })
    }
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
