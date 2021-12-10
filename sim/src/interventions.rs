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
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use log::{debug, info};

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum MaskStatus {
    None(u32),
    PublicTransport(u32),
    Everywhere(u32),
}

impl Display for MaskStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MaskStatus::None(_) => {
                write!(f, "None")
            }
            MaskStatus::PublicTransport(_) => {
                write!(f, "Only Public Transport")
            }
            MaskStatus::Everywhere(_) => {
                write!(f, "Everywhere")
            }
        }
    }
}

impl MaskStatus {
    /// The percentage of infected cases, to trigger an increase
    pub fn get_threshold(&self) -> f64 {
        // TODO Make this loaded from a config file
        match self {
            MaskStatus::None(_) => 0.0,
            MaskStatus::PublicTransport(_) => 0.2,
            MaskStatus::Everywhere(_) => 0.4,
        }
    }
}

/// This contains the thresholds of percentage cases to trigger a given intervention
///
/// If none, then the Intervention is never applied
pub struct InterventionThresholds {
    /// The percent of cases to trigger a total lockdown
    lockdown: Option<f64>,
    /// The percent of cases to trigger vaccinations
    vaccination_threshold: Option<f64>,
}

impl Default for InterventionThresholds {
    fn default() -> Self {
        Self {
            lockdown: Some(0.6),
            vaccination_threshold: Some(0.3),
        }
    }
}

pub struct InterventionStatus {
    /// The hour at which lockdown was implemented
    lockdown: Option<u32>,
    /// The hour at which vaccination was implemented
    vaccination: Option<u32>,
    /// The hour at which lockdown was implemented
    pub mask_status: MaskStatus,
    thresholds: InterventionThresholds,
}

impl Default for InterventionStatus {
    fn default() -> Self {
        Self {
            lockdown: None,
            vaccination: None,
            mask_status: MaskStatus::None(0),
            thresholds: Default::default(),
        }
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum InterventionsEnabled {
    Lockdown,
    Vaccination,
    MaskWearing(MaskStatus),
}

impl InterventionStatus {
    pub fn update_status(&mut self, percentage_infected: f64) -> BTreeSet<InterventionsEnabled> {
        //debug!("Updating intervention status");
        let mut new_interventions = BTreeSet::new();
        // Lockdown
        if let Some(threshold) = self.thresholds.lockdown {
            // Lockdown is enabled
            if threshold < percentage_infected {
                if let Some(mut hour) = self.lockdown {
                    hour += 1;
                } else {
                    self.lockdown = Some(0);
                    new_interventions.insert(InterventionsEnabled::Lockdown);
                }
            }
            // Lockdown is removed
            else if self.lockdown.is_some() {
                self.lockdown = None;
            }
        }

        // Vaccination
        if let Some(threshold) = self.thresholds.vaccination_threshold {
            if threshold < percentage_infected {
                if let Some(mut hour) = self.vaccination {
                    hour += 1;
                } else {
                    self.vaccination = Some(0);
                    new_interventions.insert(InterventionsEnabled::Vaccination);
                }
            }
        }
        //Mask Wearing
        self.mask_status = match &self.mask_status {
            MaskStatus::None(hour) => {
                if MaskStatus::PublicTransport(0).get_threshold() < percentage_infected {
                    info!("Mask wearing on public transport is enacted");
                    new_interventions.insert(InterventionsEnabled::MaskWearing(
                        MaskStatus::PublicTransport(0),
                    ));
                    MaskStatus::PublicTransport(0)
                } else {
                    MaskStatus::None(hour + 1)
                }
            },
            MaskStatus::PublicTransport(hour) => {
                if percentage_infected < MaskStatus::PublicTransport(0).get_threshold() {
                    info!("Mask wearing on public transport is removed");
                    new_interventions
                        .insert(InterventionsEnabled::MaskWearing(MaskStatus::None(0)));
                    MaskStatus::None(0)
                } else if MaskStatus::Everywhere(0).get_threshold() < percentage_infected {
                    info!("Mask wearing everywhere is enacted");
                    new_interventions
                        .insert(InterventionsEnabled::MaskWearing(MaskStatus::Everywhere(0)));
                    MaskStatus::Everywhere(0)
                } else {
                    MaskStatus::PublicTransport(hour + 1)
                }
            },
            MaskStatus::Everywhere(hour) => {
                if percentage_infected < MaskStatus::Everywhere(0).get_threshold() {
                    info!("Mask wearing everywhere is removed");
                    new_interventions.insert(InterventionsEnabled::MaskWearing(
                        MaskStatus::PublicTransport(0),
                    ));
                    MaskStatus::PublicTransport(0)
                } else {
                    MaskStatus::Everywhere(hour + 1)
                }
            }
        };

        // Mask Wearing
        new_interventions
    }
    pub fn lockdown_enabled(&self) -> bool {
        self.lockdown.is_some()
    }
    pub fn vaccination_program_started(&self) -> bool {
        self.vaccination.is_some()
    }
}
