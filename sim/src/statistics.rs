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

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::{Add, AddAssign};

use log::error;
use num_format::Locale::am;
use num_format::ToFormattedString;

use crate::config::NUMBER_FORMATTING;
use crate::disease::DiseaseStatus;
use crate::models::building::BuildingID;
use crate::models::ID;
use crate::models::output_area::OutputAreaID;

/// A snapshot of the disease per time step
#[derive(Clone)]
pub struct Statistics {
    time_step: u32,
    susceptible: u32,
    exposed: u32,
    infected: u32,
    recovered: u32,
    vaccinated: u32,
    /// First Instance, Amount
    pub buildings_exposed: HashMap<BuildingID, (u32, u32)>,
    pub workplace_exposed: HashMap<BuildingID, (u32, u32)>,
    /// First Time Step it occurred, Amount
    pub output_areas_exposed: HashMap<OutputAreaID, (u32, u32)>,
    pub public_trandport_exposure_count: u32,
}

impl Statistics {
    pub fn from_hour(hour: u32) -> Statistics {
        Statistics {
            time_step: hour,
            susceptible: 0,
            exposed: 0,
            infected: 0,
            recovered: 0,
            vaccinated: 0,
            buildings_exposed: Default::default(),
            workplace_exposed: Default::default(),
            output_areas_exposed: Default::default(),
            public_trandport_exposure_count: 0,
        }
    }
    pub fn next(&mut self) {
        self.time_step += 1;
        self.susceptible = 0;
        self.exposed = 0;
        self.infected = 0;
        self.recovered = 0;
        self.vaccinated = 0;
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
    pub fn vaccinated(&self) -> u32 {
        self.vaccinated
    }

    #[inline]
    pub fn total(&self) -> u32 {
        self.susceptible() + self.exposed() + self.infected() + self.recovered() + self.vaccinated()
    }

    pub fn infected_percentage(&self) -> f64 {
        self.infected as f64 / (self.total() as f64)
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
    pub fn citizen_exposed(&mut self, location: ID) -> Result<(), crate::error::SimError> {
        let x = self.susceptible.checked_sub(1);
        if let Some(x) = x {
            self.susceptible = x;
            self.exposed += 1;
            //debug!("Exposing: {}", exposure);
            match location {
                ID::Building(building) => self.expose_building(building),
                ID::OutputArea(output_area) => self.expose_output_area(output_area),
                ID::PublicTransport(_) => {
                    self.public_trandport_exposure_count += 1;
                }
            }

            Ok(())
        } else {
            error!("Cannot log citizen being exposed, as no susceptible citizens left");
            Err(crate::error::SimError::new_simulation_error(String::from(
                "Cannot expose citizen as no citizens are susceptible!",
            )))
        }
    }

    fn expose_building(&mut self, building_id: BuildingID) {
        let building = self
            .buildings_exposed
            .entry(building_id)
            .or_insert((self.time_step, 0));
        building.1 += 1;
    }
    fn expose_output_area(&mut self, output_area: OutputAreaID) {
        let output_area = self
            .output_areas_exposed
            .entry(output_area)
            .or_insert((self.time_step, 0));
        output_area.1 += 1;
    }
    /// Returns true if at least one Citizen has the Disease
    pub fn disease_exists(&self) -> bool {
        self.exposed != 0 || self.infected != 0 || self.susceptible != 0
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

impl AddAssign for Statistics {
    fn add_assign(&mut self, rhs: Self) {
        self.susceptible += rhs.susceptible;
        self.exposed += rhs.exposed;
        self.infected += rhs.infected;
        self.recovered += rhs.recovered;
        self.vaccinated += rhs.vaccinated;
        for (building, amount) in rhs.buildings_exposed {
            let entry = self.buildings_exposed.entry(building).or_default();
            entry.0 += amount.0;
            entry.1 += amount.1;
        }
        for (building, amount) in rhs.workplace_exposed {
            let entry = self.workplace_exposed.entry(building).or_default();
            entry.0 += amount.0;
            entry.1 += amount.1;
        }
        for (building, amount) in rhs.output_areas_exposed {
            let entry = self.output_areas_exposed.entry(building).or_default();
            entry.0 += amount.0;
            entry.1 += amount.1;
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
            public_trandport_exposure_count: 0,
        }
    }
}

impl Display for Statistics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hour: {: >4}, Total: {: >10}, Susceptible: {: >10}, {:.2}%, Exposed: {: >10}, {:.2}%, Infected: {: >10}, {:.2}%, Recovered: {: >10}, {:.2}% Vaccinated: {: >10}, {:.2}%",
            self.time_step, self.total().to_formatted_string(&NUMBER_FORMATTING), self.susceptible().to_formatted_string(&NUMBER_FORMATTING), (self.susceptible() as f64 / self.total() as f64) * 100.0, self.exposed().to_formatted_string(&NUMBER_FORMATTING), (self.exposed() as f64 / self.total() as f64) * 100.0, self.infected().to_formatted_string(&NUMBER_FORMATTING), (self.infected() as f64 / self.total() as f64) * 100.0, self.recovered().to_formatted_string(&NUMBER_FORMATTING), (self.recovered() as f64 / self.total() as f64) * 100.0, self.vaccinated().to_formatted_string(&NUMBER_FORMATTING), (self.vaccinated() as f64 / self.total() as f64) * 100.0,
        )
    }
}
