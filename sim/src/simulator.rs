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

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{LineWriter, Write};
use std::time::Instant;

use anyhow::Context;
use log::{debug, error, info};
use rand::prelude::{IteratorRandom, SliceRandom};
use rand::rngs::ThreadRng;
use rand::thread_rng;

use crate::config::{
    DEBUG_ITERATION_PRINT, get_memory_usage};
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::disease::DiseaseStatus::Infected;
use crate::interventions::{InterventionsEnabled, InterventionStatus};
use crate::models::building::{Building, BuildingID};
use crate::models::citizen::{Citizen, CitizenID};
use crate::models::ID;
use crate::models::output_area::{OutputArea, OutputAreaID};
use crate::models::public_transport_route::{PublicTransport, PublicTransportID};
use crate::simulator_builder::SimulatorBuilder;
use crate::statistics::Statistics;

pub struct Timer {
    function_timer: Instant,
    code_block_timer: Instant,
}

impl Timer {
    #[inline]
    pub fn code_block_finished(&mut self, message: &str) -> anyhow::Result<()> {
        info!(
            "{} in {:.2} seconds. Total function time: {:.2} seconds, Memory usage: {}",
            message,
            self.code_block_timer.elapsed().as_secs_f64(),
            self.function_timer.elapsed().as_secs_f64(),
            get_memory_usage()?
        );
        self.code_block_timer = Instant::now();
        Ok(())
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            function_timer: Instant::now(),
            code_block_timer: Instant::now(),
        }
    }
}


//#[derive(Clone)]
pub struct Simulator {
    /// The total size of the population
    current_population: u32,
    /// A list of all the sub areas containing agents
    pub output_areas: HashMap<OutputAreaID, OutputArea>,
    /// The list of citizens who have a "home" in this area
    pub citizens: HashMap<CitizenID, Citizen>,
    pub citizens_eligible_for_vaccine: Option<HashSet<CitizenID>>,
    pub statistics: Statistics,
    interventions: InterventionStatus,
    disease_model: DiseaseModel,
    pub public_transport: HashMap<PublicTransportID, PublicTransport>,
    rng: ThreadRng,
}

/// Runtime Simulation Methods
impl Simulator {
    pub fn simulate(&mut self) -> anyhow::Result<()> {
        let mut start_time = Instant::now();
        info!("Starting simulation...");
        for time_step in 0..self.disease_model.max_time_step {
            if time_step % DEBUG_ITERATION_PRINT as u16 == 0 {
                println!("Completed {: >3} time steps, in: {: >6} seconds  Statistics: {},   Memory usage: {}", DEBUG_ITERATION_PRINT, format!("{:.2}", start_time.elapsed().as_secs_f64()), self.statistics, get_memory_usage()?);
                start_time = Instant::now();
            }
            if !self.step()? {
                debug!("{}", self.statistics);
                break;
            }
        }
        Ok(())
    }
    /// Applies a single time step to the simulation
    ///
    /// Returns False if it has finished
    pub fn step(&mut self) -> anyhow::Result<bool> {
        let mut start = Instant::now();
        // Reset public transport containers
        self.public_transport = Default::default();
        self.generate_and_apply_exposures()?;

        let exposure_time = start.elapsed().as_secs_f64();
        start = Instant::now();

        self.apply_interventions()?;

        let intervention_time = start.elapsed().as_secs_f64();
        let total = exposure_time + intervention_time;
        debug!("Generate Exposures: {:.3} seconds ({:.3}%), Apply Interventions: {:.3} seconds ({:.3}%)",exposure_time,exposure_time/total,intervention_time,intervention_time/total);
        if !self.statistics.disease_exists() {
            info!("Disease finished as no one has the disease");
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn generate_and_apply_exposures(&mut self) -> anyhow::Result<()> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        let mut building_exposure_list: HashMap<BuildingID, usize> = HashMap::new();
        self.statistics.next();

        // The list of Citizens on Public Transport, grouped by their origin and destination
        let mut public_transport_pre_generate: HashMap<
            (OutputAreaID, OutputAreaID),
            Vec<(CitizenID, bool)>,
        > = HashMap::new();

        // Generate exposures for fixed building positions
        for citizen in self.citizens.values_mut() {
            citizen.execute_time_step(
                self.statistics.time_step(),
                &self.disease_model,
                self.interventions.lockdown_enabled(),
            );
            self.statistics.add_citizen(&citizen.disease_status);

            // Either generate public transport session, or add exposure for fixed building position
            if let Some(travel) = &citizen.on_public_transport {
                let transport_session = public_transport_pre_generate
                    .entry(travel.clone())
                    .or_default();

                transport_session.push((citizen.id(), citizen.is_infected()));
            } else if let Infected(_) = citizen.disease_status {
                let entry = building_exposure_list
                    .entry(citizen.current_building_position.clone())
                    .or_insert(1);
                *entry += 1;
            }
        }

        // Apply Building Exposures
        for (building_id, exposure_count) in building_exposure_list {
            let area = self.output_areas.get(&building_id.output_area_code());
            match area {
                Some(area) => {
                    // TODO Sometime there's a weird bug here?
                    let building = &area.buildings.get(&building_id).context(format!(
                        "Failed to retrieve exposure building {}",
                        building_id
                    ))?;
                    let building = building.as_ref();
                    let occupants = building.occupants().clone();
                    self.expose_citizens(
                        occupants,
                        exposure_count,
                        ID::Building(building_id.clone()),
                    )?;
                }

                None => {
                    error!(
                        "Cannot find output area {}, that had an exposure occurred in!",
                        &building_id.output_area_code()
                    );
                }
            }
        }
        // Generate public transport routes
        for (route, mut citizens) in public_transport_pre_generate {
            citizens.shuffle(&mut self.rng);
            let mut current_bus = PublicTransport::new(route.0.clone(), route.1.clone());
            while let Some((citizen, is_infected)) = citizens.pop() {
                // If bus is full, generate a new one
                if current_bus.add_citizen(citizen).is_err() {
                    // Only need to save buses with exposures
                    if current_bus.exposure_count > 0 {
                        self.expose_citizens(
                            current_bus.occupants().clone(),
                            current_bus.exposure_count,
                            ID::PublicTransport(current_bus.id().clone()),
                        )?;
                    }
                    current_bus = PublicTransport::new(route.0.clone(), route.1.clone());
                    current_bus
                        .add_citizen(citizen)
                        .context("Failed to add Citizen to new bus")?;
                }
                if is_infected {
                    current_bus.exposure_count += 1;
                }
            }
            if current_bus.exposure_count > 0 {
                self.expose_citizens(
                    current_bus.occupants().clone(),
                    current_bus.exposure_count,
                    ID::PublicTransport(current_bus.id().clone()),
                )?;
            }
        }
        // Apply Public Transport Exposures
        //debug!("There are {} exposures", exposure_list.len());
        Ok(())
    }

    fn expose_citizens(
        &mut self,
        citizens: Vec<CitizenID>,
        exposure_count: usize,
        location: ID,
    ) -> anyhow::Result<()> {
        for citizen_id in citizens {
            let citizen = self.citizens.get_mut(&citizen_id);
            match citizen {
                Some(citizen) => {
                    if citizen.is_susceptible()
                        && citizen.expose(
                        exposure_count,
                        &self.disease_model,
                        &self.interventions.mask_status,
                        &mut self.rng,
                    )
                    {
                        self.statistics
                            .citizen_exposed(location.clone())
                            .context(format!("Exposing citizen {}", citizen_id))?;

                        if let Some(vaccine_list) = &mut self.citizens_eligible_for_vaccine {
                            vaccine_list.remove(&citizen_id);
                        }
                    }
                }
                None => {
                    error!("Citizen {}, does not exist!", citizen_id);
                }
            }
        }
        Ok(())
    }
    fn apply_interventions(&mut self) -> anyhow::Result<()> {
        let infected_percent = self.statistics.infected_percentage();
        //debug!("Infected percent: {}",infected_percent);
        let new_interventions = self.interventions.update_status(infected_percent);
        for intervention in new_interventions {
            match intervention {
                InterventionsEnabled::Lockdown => {
                    info!(
                        "Lockdown is enabled at hour {}",
                        self.statistics.time_step()
                    );
                    // Send every Citizen home
                    for mut citizen in &mut self.citizens {
                        let home = citizen.1.household_code.clone();
                        citizen.1.current_building_position = home;
                    }
                }
                InterventionsEnabled::Vaccination => {
                    info!(
                        "Starting vaccination program at hour: {}",
                        self.statistics.time_step()
                    );
                    let mut eligible = HashSet::new();
                    self.citizens.iter().for_each(|(id, citizen)| {
                        if citizen.disease_status == DiseaseStatus::Susceptible {
                            eligible.insert(*id);
                        }
                    });
                    self.citizens_eligible_for_vaccine = Some(eligible);
                }
                InterventionsEnabled::MaskWearing(status) => {
                    info!(
                        "Mask wearing status has changed: {} at hour {}",
                        status,
                        self.statistics.time_step()
                    )
                }
            }
        }
        if let Some(citizens) = &mut self.citizens_eligible_for_vaccine {
            let chosen: Vec<CitizenID> = citizens
                .iter()
                .choose_multiple(&mut self.rng, self.disease_model.vaccination_rate as usize)
                .iter()
                .map(|id| **id)
                .collect();
            for citizen_id in chosen {
                citizens.remove(&citizen_id);
                let citizen = self
                    .citizens
                    .get_mut(&citizen_id)
                    .context("Citizen '{}' due to be vaccinated, doesn't exist!")?;
                citizen.disease_status = DiseaseStatus::Vaccinated;
            }
        }

        Ok(())
    }
}

impl Simulator {
    pub fn error_dump(self) -> anyhow::Result<()> {
        println!("Creating Core Dump!");
        let file = File::create("crash.dump")?;
        let mut file = LineWriter::new(file);
        writeln!(file, "{}", self.statistics)?;
        for area in self.output_areas {
            writeln!(file, "Output Area: {}", area.0)?;
            for building in area.1.buildings.values() {
                writeln!(file, "      {}", building)?;
            }
        }
        writeln!(file, "\n\n\n----------\n\n\n")?;
        for citizen in self.citizens {
            writeln!(file, "    {}", citizen.1)?;
        }
        Ok(())
    }
    pub fn error_dump_json(self) -> anyhow::Result<()> {
        println!("Creating Core Dump!");
        let mut file = File::create("crash.json")?;
        use serde_json::json;

        let mut output_area_json = HashMap::new();
        for area in self.output_areas {
            output_area_json.insert(area.0, area.1.buildings);
        }
        let mut citizens = HashMap::new();
        for citizen in self.citizens {
            citizens.insert(citizen.0.to_string(), citizen.1);
        }
        file.write_all(
            json!({"citizens":citizens,"output_areas":output_area_json})
                .to_string()
                .as_ref(),
        )?;

        Ok(())
    }
}

impl From<SimulatorBuilder> for Simulator {
    fn from(builder: SimulatorBuilder) -> Self {
        Simulator {
            current_population: builder.citizens.len() as u32,
            output_areas: builder.output_areas,
            citizens: builder.citizens,
            citizens_eligible_for_vaccine: None,
            statistics: Statistics::default(),
            interventions: Default::default(),
            disease_model: builder.disease_model,
            public_transport: Default::default(),
            rng: thread_rng(),
        }
    }
}