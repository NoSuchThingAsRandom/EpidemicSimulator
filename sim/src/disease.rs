use std::fmt::{Display, Formatter};
use std::hash::Hash;

use log::error;
use uuid::Uuid;

use load_census_data::tables::population_and_density_per_output_area::AreaClassification;

use crate::models::building::BuildingCode;

#[derive(PartialEq)]
pub enum DiseaseStatus {
    Susceptible,
    /// The amount of steps(hours) the citizen has been exposed for
    Exposed(u16),
    /// The amount of steps(hours) the citizen has been infected for
    Infected(u16),
    Recovered,
}

impl DiseaseStatus {
    pub fn execute_time_step(status: &DiseaseStatus, disease_model: &DiseaseModel) -> DiseaseStatus {
        match status {
            DiseaseStatus::Susceptible => {
                DiseaseStatus::Susceptible
            }
            DiseaseStatus::Exposed(time) => {
                if disease_model.exposed_time <= *time {
                    DiseaseStatus::Infected(0)
                } else {
                    DiseaseStatus::Exposed(time + 1)
                }
            }
            DiseaseStatus::Infected(time) => {
                if disease_model.infected_time <= *time {
                    DiseaseStatus::Recovered
                } else {
                    DiseaseStatus::Infected(time + 1)
                }
            }
            DiseaseStatus::Recovered => { DiseaseStatus::Recovered }
        }
    }
}

/// A snapshot of the disease per time step
pub struct Statistics {
    time_step: u8,
    susceptible: u32,
    exposed: u32,
    infected: u32,
    recovered: u32,
}

impl Statistics {
    pub fn new(hour: u8) -> Statistics {
        Statistics {
            time_step: hour,
            susceptible: 0,
            exposed: 0,
            infected: 0,
            recovered: 0,
        }
    }
    pub fn time_step(&self) -> u8 {
        self.time_step
    }
    pub fn susceptible(&self) -> u32 {
        self.susceptible
    }
    pub fn exposed(&self) -> u32 {
        self.exposed
    }
    pub fn infected(&self) -> u32 {
        self.infected
    }
    pub fn recovered(&self) -> u32 {
        self.recovered
    }
    pub fn increment(&mut self) {
        self.time_step += 1;
    }
    /// Adds a new Citizen to the log, and increments the stage the citizen is at by one
    pub fn add_citizen(&mut self, disease_status: &DiseaseStatus) {
        match disease_status {
            DiseaseStatus::Susceptible => {
                self.susceptible += 1;
            }
            DiseaseStatus::Exposed(_) => {
                self.exposed += 1;
            }
            DiseaseStatus::Infected(_) => {
                self.infected += 1;
            }
            DiseaseStatus::Recovered => {
                self.recovered += 1;
            }
        }
    }
    /// When a citizen has been exposed, the susceptible count drops by one, and exposure count increases by 1
    /// Will error, if called when no Citizens are susceptible
    pub fn citizen_exposed(&mut self) -> Result<(), crate::error::MyError> {
        let x = self.susceptible.checked_sub(1);
        if let Some(x) = x {
            self.susceptible = x;
            self.exposed += 1;
            Ok(())
        } else {
            error!("Cannot log citizen being exposed, as no susceptible citizens left");
            return Err(crate::error::MyError::new(String::from("Cannot expose citizen as no citizens are susceptible!")));
        }
    }
    /// Returns true if at least one Citizen has the Disease
    pub fn disease_exists(&self) -> bool {
        self.exposed != 0 && self.infected != 0
    }
}


impl Default for Statistics {
    fn default() -> Self {
        Statistics {
            time_step: 0,
            susceptible: 0,
            exposed: 0,
            infected: 0,
            recovered: 0,
        }
    }
}

impl Display for Statistics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hour: {}, Susceptible: {}, Exposed: {}, Infected: {}, Recovered: {}", self.time_step, self.susceptible, self.exposed, self.infected, self.recovered)
    }
}

pub struct DiseaseModel {
    pub reproduction_rate: f64,
    pub exposure_chance: f64,
    pub death_rate: f64,
    pub exposed_time: u16,
    pub infected_time: u16,
    pub max_time_step: u16,

}

impl DiseaseModel {
    /// Creates a new disease model representative of COVID-19
    ///
    /// R Rate - 2.5
    /// Death Rate - 0.05
    /// Exposure Time - 4 days
    /// Infected Time - 14 days
    pub fn covid() -> DiseaseModel {
        DiseaseModel { reproduction_rate: 2.5, exposure_chance: 0.6, death_rate: 0.05, exposed_time: 4 * 24, infected_time: 14 * 24, max_time_step: 1000 }
    }
}

/// Represents an event where Citizens are exposed to an infected individual at the given building
#[derive(Hash, PartialEq, Eq)]
pub struct Exposure {
    /// The ID of the citizen who is infected
    pub infector_id: Uuid,
    /// The building the infection occurred in
    building_code: BuildingCode,
}

impl Exposure {
    /// Create a new exposure event from the given citizen at the given building
    pub fn new(citizen_id: Uuid, building: BuildingCode) -> Exposure {
        Exposure { infector_id: citizen_id, building_code: building }
    }
    pub fn output_area_code(&self) -> String {
        self.building_code.output_area_code()
    }
    pub fn area_classification(&self) -> AreaClassification {
        self.building_code.area_type()
    }
    pub fn building_code(&self) -> Uuid {
        self.building_code.building_id()
    }
}

impl Display for Exposure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Exposure by {}, at {} ", self.infector_id, self.building_code)
    }
}
