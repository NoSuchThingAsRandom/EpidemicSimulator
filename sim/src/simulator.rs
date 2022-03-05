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

use crate::config::{DEBUG_ITERATION_PRINT, get_memory_usage};
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::disease::DiseaseStatus::Infected;
use crate::error::SimError;
use crate::interventions::{InterventionsEnabled, InterventionStatus};
use crate::models::building::BuildingID;
use crate::models::citizen::{Citizen, CitizenID};
use crate::models::ID;
use crate::models::output_area::{OutputArea, OutputAreaID};
use crate::models::public_transport_route::{PublicTransport, PublicTransportID};
use crate::simulator_builder::SimulatorBuilder;
use crate::statistics::{StatisticEntry, StatisticsRecorder};

#[derive(Debug, Default, Clone)]
struct GeneratedExposures {
    /// The list of Citizens on Public Transport, grouped by their origin and destination
    public_transport_pre_generated: HashMap<
        (OutputAreaID, OutputAreaID),
        Vec<(CitizenID, bool)>,
    >,
    /// The list of buildings, with the amount of exposures that occurred
    building_exposure_list: HashMap<BuildingID, Vec<CitizenID>>,
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
    pub statistics_recorder: StatisticsRecorder,
    interventions: InterventionStatus,
    disease_model: DiseaseModel,
    pub public_transport: HashMap<PublicTransportID, PublicTransport>,
    rng: ThreadRng,
}

/// Runtime Simulation Methods
impl Simulator {
    /// Start the entire simulation process, until the disease is eradicated, or we reach teh max time step
    pub fn simulate(&mut self) -> anyhow::Result<()> {
        let mut start_time = Instant::now();
        info!("Starting simulation...");
        for time_step in 0..self.disease_model.max_time_step {
            if !self.step()? {
                debug!("{:?}", self.statistics_recorder.global_stats.last().expect("No data recorded!"));
                break;
            }
            if time_step % DEBUG_ITERATION_PRINT as u16 == 0 {
                println!("Completed {: >3} time steps, in: {: >6} seconds  Statistics: {:?},   Memory usage: {}", DEBUG_ITERATION_PRINT, format!("{:.2}", start_time.elapsed().as_secs_f64()), self.statistics_recorder.global_stats.last().expect("No data recorded!"), get_memory_usage()?);
                start_time = Instant::now();
            }
        }
        self.statistics_recorder.dump_to_file("recordings/v1.1.json");
        Ok(())
    }
    /// Applies a single time step to the simulation
    ///
    /// Returns False if it has finished
    pub fn step(&mut self) -> anyhow::Result<bool> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        self.statistics_recorder.next();
        // Reset public transport containers
        self.public_transport = Default::default();
        let exposures = self.generate_exposures()?;
        self.statistics_recorder.record_function_time("Generate Exposures".to_string());

        self.apply_exposures(exposures)?;
        self.statistics_recorder.record_function_time("Apply Exposures".to_string());

        self.apply_interventions()?;
        self.statistics_recorder.record_function_time("Apply Interventions".to_string());


        if !self.statistics_recorder.disease_exists() {
            info!("Disease finished as no one has the disease");
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Detects the Citizens that have been exposed in the current time step
    fn generate_exposures(&mut self) -> anyhow::Result<GeneratedExposures> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        let mut exposures = GeneratedExposures::default();

        // Generate exposures for fixed building positions
        for citizen in self.citizens.values_mut() {
            citizen.execute_time_step(
                self.statistics_recorder.time_step(),
                &self.disease_model,
                self.interventions.lockdown_enabled(),
            );
            self.statistics_recorder.add_citizen(&citizen.disease_status);

            // Either generate public transport session, or add exposure for fixed building position
            if let Some(travel) = &citizen.on_public_transport {
                let transport_session = exposures.public_transport_pre_generated
                    .entry(travel.clone())
                    .or_default();

                transport_session.push((citizen.id(), citizen.is_infected()));
            } else if let Infected(_) = citizen.disease_status {
                let entry = exposures.building_exposure_list
                    .entry(citizen.current_building_position.clone())
                    .or_insert(vec![citizen.id()]);
                entry.push(citizen.id());
            }
        }
        return Ok(exposures);
    }
    fn apply_exposures(&mut self, exposures: GeneratedExposures) -> anyhow::Result<()> {

        // Apply Building Exposures
        for (building_id, exposure_count) in exposures.building_exposure_list {
            let area = self.output_areas.get(&building_id.output_area_code());
            match area {
                Some(area) => {
                    // TODO Sometime there's a weird bug here?
                    let building = &area.buildings.get(&building_id).context(format!(
                        "Failed to retrieve exposure building {}",
                        building_id
                    ))?;
                    let building = building.as_ref();
                    let mut to_expose = Vec::new();
                    for citizen in exposure_count {
                        let occupants = building.apply_exposure(citizen);
                        for id in occupants {
                            to_expose.push(id);
                        }
                    }
                    if let Err(e) =
                    self.expose_citizens(
                        to_expose, 1,
                        ID::Building(building_id.clone()),
                    ).context(format!("Exposing building: {}", building_id)) {
                        error!("{:?}",e)
                    }
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
        for (route, mut citizens) in exposures.public_transport_pre_generated {
            citizens.shuffle(&mut self.rng);
            let mut current_bus = PublicTransport::new(route.0.clone(), route.1.clone());
            while let Some((citizen, is_infected)) = citizens.pop() {
                // If bus is full, generate a new one
                if current_bus.add_citizen(citizen).is_err() {
                    // Only need to save buses with exposures
                    if current_bus.exposure_count > 0 {
                        if let Err(e) =
                        self.expose_citizens(
                            current_bus.occupants().clone(),
                            current_bus.exposure_count,
                            ID::PublicTransport(current_bus.id().clone()),
                        ).context(format!("Failed to expose bus: {}", current_bus.id())) {
                            error!("{:?}",e);
                        }
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
                if let Err(e) =
                self.expose_citizens(
                    current_bus.occupants().clone(),
                    current_bus.exposure_count,
                    ID::PublicTransport(current_bus.id().clone()),
                ).context(format!("Failed to expose bus: {}", current_bus.id())) {
                    error!("{:?}",e);
                }
            }
        }
        // Apply Public Transport Exposures
        //debug!("There are {} exposures", exposure_list.len());
        Ok(())
    }

    /// Applies the Exposure event to the given Citizens
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
                        self.statistics_recorder
                            .add_exposure(location.clone())
                            .context(format!("Exposing citizen {}", citizen_id))?;

                        if let Some(vaccine_list) = &mut self.citizens_eligible_for_vaccine {
                            vaccine_list.remove(&citizen_id);
                        }
                    }
                }
                None => {
                    return Err(SimError::MissingCitizen { citizen_id: citizen_id.to_string() }).context("Cannot expose Citizen, as they do not exist!");
                }
            }
        }
        Ok(())
    }
    fn apply_interventions(&mut self) -> anyhow::Result<()> {
        let infected_percent = self.statistics_recorder.infected_percentage();
        //debug!("Infected percent: {}",infected_percent);
        let new_interventions = self.interventions.update_status(infected_percent);
        for intervention in new_interventions {
            match intervention {
                InterventionsEnabled::Lockdown => {
                    info!(
                        "Lockdown is enabled at hour {}",
                        self.statistics_recorder.time_step()
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
                        self.statistics_recorder.time_step()
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
                        self.statistics_recorder.time_step()
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
        writeln!(file, "{:?}", self.statistics_recorder)?;
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
        let mut file = File::create("../../debug_dumps/crash.json")?;
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
            statistics_recorder: StatisticsRecorder::default(),
            interventions: Default::default(),
            disease_model: builder.disease_model,
            public_transport: Default::default(),
            rng: thread_rng(),
        }
    }
}
