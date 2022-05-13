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

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Serialize, Clone)]
pub enum DiseaseStatus {
    Susceptible,
    /// The amount of steps(hours) the citizen has been exposed for
    Exposed(u16),
    /// The amount of steps(hours) the citizen has been infected for
    Infected(u16),
    Recovered,
    Vaccinated,
}

impl DiseaseStatus {
    pub fn execute_time_step(
        status: &DiseaseStatus,
        disease_model: &DiseaseModel,
    ) -> DiseaseStatus {
        match status {
            DiseaseStatus::Susceptible => DiseaseStatus::Susceptible,
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
            DiseaseStatus::Recovered => DiseaseStatus::Recovered,
            // TODO Allow "break through" infections
            DiseaseStatus::Vaccinated => DiseaseStatus::Vaccinated,
        }
    }
}

impl Display for DiseaseStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DiseaseStatus::Susceptible => {
                write!(f, "Susceptible to Infection")
            }
            DiseaseStatus::Exposed(since) => {
                write!(f, "Exposed since: {}", since)
            }
            DiseaseStatus::Infected(since) => {
                write!(f, "Infected since: {}", since)
            }
            DiseaseStatus::Recovered => {
                write!(f, "Recovered/Died")
            }
            DiseaseStatus::Vaccinated => {
                write!(f, "Vaccinated")
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct DiseaseModel {
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
        DiseaseModel {
            exposure_chance: 0.00055,
            death_rate: 0.2,
            exposed_time: 4 * 24,
            infected_time: 14 * 24,
            max_time_step: 5000,
        }
    }
}
