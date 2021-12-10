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

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use log::error;

use crate::disease::{DiseaseStatus, Exposure};
use crate::models::building::BuildingCode;

/// A snapshot of the disease per time step
pub struct Statistics {
    time_step: u32,
    susceptible: u32,
    exposed: u32,
    infected: u32,
    recovered: u32,
    vaccinated: u32,
    /// First Instance, Amount
    pub buildings_exposed: HashMap<BuildingCode, (u32, u32)>,
    pub workplace_exposed: HashMap<BuildingCode, (u32, u32)>,
    /// First Instance, Amount
    pub output_areas_exposed: HashMap<String, (u32, u32)>,
}

impl Statistics {
    pub fn new() -> Statistics {
        Statistics {
            time_step: 0,
            susceptible: 0,
            exposed: 0,
            infected: 0,
            recovered: 0,
            vaccinated: 0,
            buildings_exposed: Default::default(),
            workplace_exposed: Default::default(),
            output_areas_exposed: Default::default(),
        }
    }
    pub fn next(&mut self) {
        self.time_step += 1;
        self.susceptible = 0;
        self.exposed = 0;
        self.infected = 0;
        self.recovered = 0;
    }
    pub fn time_step(&self) -> u32 {
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
    pub fn infected_percentage(&self) -> f64 {
        self.infected as f64 / (self.susceptible + self.exposed + self.recovered) as f64
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
            DiseaseStatus::Vaccinated => self.vaccinated += 1,
        }
    }
    /// When a citizen has been exposed, the susceptible count drops by one, and exposure count increases by 1
    /// Will error, if called when no Citizens are susceptible
    pub fn citizen_exposed(&mut self, exposure: Exposure) -> Result<(), crate::error::Error> {
        let x = self.susceptible.checked_sub(1);
        if let Some(x) = x {
            self.susceptible = x;
            self.exposed += 1;
            //debug!("Exposing: {}", exposure);
            if let Some(data) = self.buildings_exposed.get_mut(&exposure.building_code) {
                data.1 += 1;
            } else {
                self.buildings_exposed
                    .insert(exposure.building_code.clone(), (self.time_step, 1));
            }

            if let Some(data) = self
                .output_areas_exposed
                .get_mut(&exposure.building_code.output_area_code().clone())
            {
                data.1 += 1;
            } else {
                self.output_areas_exposed.insert(
                    exposure.building_code.output_area_code(),
                    (self.time_step, 1),
                );
            }

            Ok(())
        } else {
            error!("Cannot log citizen being exposed, as no susceptible citizens left");
            return Err(crate::error::Error::new_simulation_error(String::from(
                "Cannot expose citizen as no citizens are susceptible!",
            )));
        }
    }
    /// Returns true if at least one Citizen has the Disease
    pub fn disease_exists(&self) -> bool {
        self.exposed != 0 || self.infected != 0
    }

    pub fn summarise(&self) {
        println!("\n\n\n--------\n");
        println!("Output Areas Exposed: ");
        for area in &self.output_areas_exposed {
            println!(
                "         {} first infected at {} with total {}",
                area.0, area.1.0, area.1.1
            );
        }
        println!("\n\n\n--------\n");
        println!("Buildings exposed Exposed: ");
        for building in &self.buildings_exposed {
            println!(
                "         {} first infected at {} with total {}",
                building.0, building.1.0, building.1.1
            );
        }
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
            vaccinated: 0,
            buildings_exposed: Default::default(),
            workplace_exposed: Default::default(),
            output_areas_exposed: Default::default(),
        }
    }
}

impl Display for Statistics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hour: {}, Susceptible: {}, Exposed: {}, Infected: {}, Recovered: {} Vaccinated: {}",
            self.time_step, self.susceptible, self.exposed, self.infected, self.recovered, self.vaccinated
        )
    }
}
