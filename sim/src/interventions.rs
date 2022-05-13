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
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use log::info;
use serde::{Deserialize, Serialize};

/// The set of possible interventions that can be applied
#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum ActiveInterventions {
    Lockdown,
    Vaccination,
    MaskWearing(MaskStatus),
}

/// The current Mask Status applied to the population
///
/// The `u32` represents the amount of steps the current status has been active for
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
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

/// This contains the thresholds of percentage cases to trigger a given intervention
///
/// If none, then the Intervention is never applied
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InterventionThresholds {
    /// The percent of cases to trigger a total lockdown
    lockdown: Option<f64>,
    /// The percent of cases to trigger vaccinations
    vaccination_threshold: Option<f64>,
    /// The percent of cases to trigger masks on public transport
    masks_public_transport: Option<f64>,
    /// The percent of cases to trigger masks everywhere
    masks_everywhere: Option<f64>,
    /// How effective the vaccines are at preventing infection
    vaccine_effectiveness: f64,
    /// The amount of people vaccinated per timestamp (per 100,000 people)
    pub vaccination_rate: usize,

    // TODO Check if data on mask compliance ratio
    /// The amount of people that wear masks
    pub mask_compliance_percentage: f64,
    /// How much the masks reduce chance of infection
    pub mask_effectiveness: f64,
}

impl InterventionThresholds {
    pub fn get_mask_threshold(&self, mask_status: MaskStatus) -> Option<f64> {
        match mask_status {
            MaskStatus::None(_) => Some(0.0),
            MaskStatus::PublicTransport(_) => self.masks_public_transport,
            MaskStatus::Everywhere(_) => self.masks_everywhere,
        }
    }
}

impl Default for InterventionThresholds {
    fn default() -> Self {
        Self {
            lockdown: Some(0.0034),
            vaccination_threshold: Some(0.005),
            masks_public_transport: Some(0.001),
            masks_everywhere: Some(0.0022),
            vaccination_rate: 42,
            vaccine_effectiveness: 1.0,
            mask_compliance_percentage: 0.8,
            mask_effectiveness: 0.7,
        }
    }
}

#[derive(Clone, Debug)]
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

impl From<InterventionThresholds> for InterventionStatus {
    fn from(thresholds: InterventionThresholds) -> Self {
        let mut status = InterventionStatus::default();
        status.thresholds = thresholds;
        status
    }
}

impl InterventionStatus {
    pub fn update_status(&mut self, percentage_infected: f64) -> BTreeSet<ActiveInterventions> {
        //debug!("Updating intervention status");
        let mut new_interventions = BTreeSet::new();
        // Lockdown
        if let Some(threshold) = self.thresholds.lockdown {
            // Lockdown is enabled
            if threshold < percentage_infected {
                self.lockdown = Some(if let Some(hour) = self.lockdown {
                    hour + 1
                } else {
                    new_interventions.insert(ActiveInterventions::Lockdown);
                    0
                });
            }
            // Lockdown is removed
            else if self.lockdown.is_some() {
                self.lockdown = None;
            }
        }

        // Vaccination
        if let Some(threshold) = self.thresholds.vaccination_threshold {
            if threshold < percentage_infected {
                self.vaccination = Some(if let Some(hour) = self.vaccination {
                    hour + 1
                } else {
                    new_interventions.insert(ActiveInterventions::Vaccination);
                    0
                });
            }
        }
        self.update_mask_status(percentage_infected, &mut new_interventions);

        // Mask Wearing
        new_interventions
    }
    /// This updates the current mask status depending on the percentage of infected Citizens
    ///
    /// If the current infected percent drops below the threshold, the mask status drops
    ///
    /// Or if If the current infected percent increases above the next threshold, the mask status increases
    ///
    /// TODO Change to use if let chains: https://github.com/rust-lang/rust/pull/9492, when released
    fn update_mask_status(
        &mut self,
        percentage_infected: f64,
        new_interventions: &mut BTreeSet<ActiveInterventions>,
    ) {
        self.mask_status = match &self.mask_status {
            MaskStatus::None(hour) => {
                let mut new_status = MaskStatus::None(hour + 1);
                if let Some(threshold) = self
                    .thresholds
                    .get_mask_threshold(MaskStatus::PublicTransport(0))
                {
                    if threshold < percentage_infected {
                        info!("Mask wearing on public transport is enacted");
                        new_interventions.insert(ActiveInterventions::MaskWearing(
                            MaskStatus::PublicTransport(0),
                        ));
                        new_status = MaskStatus::PublicTransport(0);
                    }
                }
                new_status
            }
            MaskStatus::PublicTransport(hour) => {
                let mut new_status = MaskStatus::PublicTransport(hour + 1);
                if let Some(threshold) = self
                    .thresholds
                    .get_mask_threshold(MaskStatus::PublicTransport(0))
                {
                    if percentage_infected < threshold {
                        info!("Mask wearing on public transport is removed");
                        new_interventions
                            .insert(ActiveInterventions::MaskWearing(MaskStatus::None(0)));
                        new_status = MaskStatus::None(0);
                    }
                }
                if let Some(threshold) = self
                    .thresholds
                    .get_mask_threshold(MaskStatus::Everywhere(0))
                {
                    if threshold < percentage_infected {
                        info!("Mask wearing everywhere is enacted");
                        new_interventions
                            .insert(ActiveInterventions::MaskWearing(MaskStatus::Everywhere(0)));
                        new_status = MaskStatus::Everywhere(0);
                    }
                }
                new_status
            }
            MaskStatus::Everywhere(hour) => {
                let mut new_status = MaskStatus::Everywhere(hour + 1);
                if let Some(threshold) = self
                    .thresholds
                    .get_mask_threshold(MaskStatus::Everywhere(0))
                {
                    if percentage_infected < threshold {
                        info!("Mask wearing everywhere is removed");
                        new_interventions.insert(ActiveInterventions::MaskWearing(
                            MaskStatus::PublicTransport(0),
                        ));
                        new_status = MaskStatus::PublicTransport(0);
                    }
                }
                new_status
            }
        };
    }
    pub fn lockdown_enabled(&self) -> bool {
        self.lockdown.is_some()
    }
    pub fn vaccination_program_started(&self) -> bool {
        self.vaccination.is_some()
    }
    pub fn vaccination_effectiveness(&self) -> f64 {
        self.thresholds.vaccine_effectiveness
    }
    pub fn mask_compliance_percentage(&self) -> f64 {
        self.thresholds.mask_compliance_percentage
    }
    pub fn mask_effectiveness(&self) -> f64 {
        self.thresholds.mask_effectiveness
    }
    pub fn vaccination_rate(&self) -> usize {
        self.thresholds.vaccination_rate
    }
}
