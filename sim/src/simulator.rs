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
use std::ops::AddAssign;
use std::sync::{Arc, Mutex, RwLock, RwLockWriteGuard};
use std::time::Instant;

use anyhow::{Context, Error};
use dashmap::DashMap;
use log::{debug, error, info, warn};
use num_format::Locale::{am, ar, en};
use rand::prelude::{IteratorRandom, SliceRandom};
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};

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
use crate::statistics::Statistics;

pub enum DayOfWeek {
    /// Represents a normal working day (Mon - Fri), with the integer representing the actual day
    ///
    /// 1 -> Monday
    /// 2 -> Tuesday
    /// 3 -> Wednesday
    /// 4 -> Thursday
    /// 5 -> Friday
    Weekday(u8),
    /// The weekend, where most of the population are not working, with the integer representing the actual day
    ///
    /// 1 -> Saturday
    /// 2 -> Sunday
    Weekend(u8),
}

impl DayOfWeek {
    pub fn next_day(self) -> Self {
        match self {
            DayOfWeek::Weekday(day) => {
                if day < 5 {
                    DayOfWeek::Weekday(day + 1)
                } else {
                    DayOfWeek::Weekend(1)
                }
            }
            DayOfWeek::Weekend(day) => {
                if day < 2 {
                    DayOfWeek::Weekend(day + 1)
                } else {
                    DayOfWeek::Weekday(1)
                }
            }
        }
    }
}

impl Default for DayOfWeek {
    fn default() -> Self {
        DayOfWeek::Weekday(0)
    }
}

/// A simple struct for benchmarking how long a block of code takes
pub struct Timer {
    function_timer: Instant,
    code_block_timer: Instant,
}

impl Timer {
    /// Call this to record how long has elapsed since the last call
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

#[derive(Debug, Default, Clone)]
struct GeneratedExposures {
    /// The list of Citizens on Public Transport, grouped by their origin and destination,
    ///
    /// The bool represents whether a Citizen is infected
    public_transport_pre_generated: HashMap<
        (OutputAreaID, OutputAreaID),
        Vec<(CitizenID, bool)>,
    >,
    /// The list of buildings, with the amount of exposures that occurred
    building_exposure_list: HashMap<OutputAreaID, HashMap<BuildingID, Vec<CitizenID>>>,
}

impl AddAssign for GeneratedExposures {
    fn add_assign(&mut self, rhs: Self) {
        for (building, citizens) in rhs.public_transport_pre_generated {
            let entry = self.public_transport_pre_generated.entry(building).or_default();
            entry.extend(citizens);
        }
        for (area, exposures) in rhs.building_exposure_list {
            let area_entry = self.building_exposure_list.entry(area).or_default();
            for (building, citizens) in exposures {
                let area_entry = area_entry.entry(building).or_default();
                area_entry.extend(citizens);
            }
        }
    }
}

//#[derive(Clone)]
pub struct Simulator {
    /// The total size of the population
    current_population: u32,
    /// A list of all the sub areas containing agents
    pub output_areas: RwLock<HashMap<OutputAreaID, Mutex<OutputArea>>>,
    pub citizen_output_area_lookup: RwLock<HashMap<CitizenID, Mutex<OutputAreaID>>>,
    pub citizens_eligible_for_vaccine: Option<HashSet<CitizenID>>,
    pub statistics: Statistics,
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
        let exposures = self.generate_exposures()?;
        let generate_exposure_time = start.elapsed().as_secs_f64();

        self.apply_exposures(exposures)?;
        let apply_exposure_time = start.elapsed().as_secs_f64();
        start = Instant::now();

        self.apply_interventions()?;

        let intervention_time = start.elapsed().as_secs_f64();
        let total = generate_exposure_time + apply_exposure_time + intervention_time;
        //debug!("Generate Exposures: {:.3} seconds ({:.3}%),Apply Exposures: {:.3} seconds ({:.3}%),  Apply Interventions: {:.3} seconds ({:.3}%)",generate_exposure_time,(generate_exposure_time/total)*100.0,apply_exposure_time,(apply_exposure_time/total)*100.0,intervention_time,(intervention_time/total)*100.0);
        if !self.statistics.disease_exists() {
            info!("Disease finished as no one has the disease");
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Detects the Citizens that have been exposed in the current time step
    fn generate_exposures(&mut self) -> anyhow::Result<GeneratedExposures> {
        //debug!("Executing time step at hour: {}",self.current_statistics.time_step());
        self.statistics.next();
        let hour = self.statistics.time_step();
        let disease = &self.disease_model;
        let lockdown = self.interventions.lockdown_enabled();
        let mut output_areas = self.output_areas.write().unwrap();


        // Update the Position and Schedule of each Citizen
        // If a Citizen is changing area, then they are moved into `moved_citizens`
        // For any Citizens that are infected, build a list of infected buildings
        let (statistics, exposures, moved_citizens) = output_areas.par_iter_mut().map(|(area_code, mut area)| {
            let mut area = area.lock().unwrap();
            let (mut statistics, mut exposures) = (Statistics::from_hour(hour), GeneratedExposures::default());
            // Apply timestep, and generate exposures
            let mut area_citizens = HashMap::with_capacity(area.citizens.len());
            let mut moving_citizens: HashMap<OutputAreaID, HashMap<CitizenID, Citizen>> = HashMap::new();
            for (id, mut citizen) in area.citizens.drain() {
                let need_to_move = citizen.execute_time_step(
                    hour, disease, lockdown,
                ).is_some();
                statistics.add_citizen(&citizen.disease_status);

                // Either generate public transport session, or add exposure for fixed building position
                if let Some(travel) = &citizen.on_public_transport {
                    let transport_session = exposures.public_transport_pre_generated
                        .entry(travel.clone())
                        .or_default();

                    transport_session.push((citizen.id(), citizen.is_infected()));
                } else if let Infected(_) = citizen.disease_status {
                    let area_entry = exposures.building_exposure_list.entry(citizen.current_building_position.output_area_code()).or_default();
                    let entry = area_entry
                        .entry(citizen.current_building_position.clone())
                        .or_default();
                    entry.push(citizen.id());
                }
                if need_to_move {
                    let entry = moving_citizens.entry(citizen.current_building_position.output_area_code()).or_default();
                    entry.insert(id, citizen);
                } else {
                    area_citizens.insert(id, citizen);
                }
            }
            area.citizens = area_citizens;
            (statistics, exposures, moving_citizens)
        }).reduce(|| (Statistics::from_hour(hour), GeneratedExposures::default(), HashMap::new()), |(mut a_stats, mut a_exposures, mut a_to_move), (b_stats, b_exposures, b_to_move)| {
            a_stats += b_stats;
            a_exposures += b_exposures;
            for (area, citizens) in b_to_move {
                let entry = a_to_move.entry(area).or_default();
                for (id, citizen) in citizens {
                    entry.insert(id, citizen);
                }
            }
            (a_stats, a_exposures, a_to_move)
        });
        let mut citizen_lookup = self.citizen_output_area_lookup.write().unwrap();
        for (area, citizens) in moved_citizens {
            match output_areas.get_mut(&area) {
                Some(area) => {
                    let mut area = area.lock().unwrap();
                    for (id, citizen) in citizens {
                        match citizen_lookup.get_mut(&id) {
                            Some(lookup_entry) => { *lookup_entry = Mutex::new(area.output_area_id.clone()); }
                            None => {
                                warn!("Citizen {} does not have a lookup entry!",id);
                                citizen_lookup.insert(id, Mutex::new(area.output_area_id.clone()));
                            }
                        }
                        area.citizens.insert(id, citizen);
                    }
                }
                None => error!("Area {} doesn't exist!",area)
            };
        }
        self.statistics += statistics;

        return Ok(exposures);
    }

    /// Applies the exposure cycle on any Citizens that have come in contact with an infected Citizen
    fn apply_exposures(&mut self, exposures: GeneratedExposures) -> anyhow::Result<()> {
        let disease = &self.disease_model;
        let mask_status = &self.interventions.mask_status;
        let output_areas = &self.output_areas;
        let mut statistics = &mut self.statistics;
        // Apply building exposures
        let exposure_statistics: Vec<ID> = exposures.building_exposure_list.par_iter().map(|(area_id, building_exposures)| -> Vec<ID> {
            let mut exposures = Vec::new();
            let output_areas = output_areas.read().unwrap();
            let area = match output_areas.get(&area_id).context("Failed to retrieve Output Area") {
                Ok(area) => { area }
                Err(e) => {
                    error!("{:?}",e);
                    return exposures;
                }
            };
            let mut area = area.lock().unwrap();
            for (building_id, infected_citizens) in building_exposures {
                let building = &area.buildings.get(&building_id).context(format!(
                    "Failed to retrieve exposure building {}",
                    building_id
                ));
                let building = match building {
                    Ok(building) => building,
                    Err(e) => {
                        error!("{:?}",e);
                        continue;
                    }
                };

                let building = building.as_ref();
                let exposure_count = infected_citizens.len();
                for citizen_id in building.find_exposures(infected_citizens) {
                    let mut citizen = match area.citizens.get_mut(&citizen_id).context("Cannot expose Citizen, as they do not exist!")
                    {
                        Ok(citizen) => { citizen }
                        Err(e) => {
                            // TODO This is a big error, as Citizens aren't in the Building they're meant to be?
                            // Perhaps it's remote working and/or Public transport meaning Citizens get to buildings at different points?
                            //error!("{:?}",e);
                            continue;
                        }
                    };
                    if citizen.is_susceptible()
                        && citizen.expose(
                        exposure_count,
                        disease,
                        mask_status,
                        &mut thread_rng(),
                    )
                    {
                        exposures.push(ID::Building(building_id.clone()));
                        if let Some(vaccine_list) = &mut area.citizens_eligible_for_vaccine {
                            vaccine_list.remove(&citizen_id);
                        }
                    }
                }
            }
            return exposures;
        }).flatten().collect();
        for id in exposure_statistics {
            self.statistics.citizen_exposed(id);
        }

        // Generate public transport routes
        for (route, mut citizens) in exposures.public_transport_pre_generated {
            // Shuffle to ensure randomness on bus
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
            let mut area_ref = self.output_areas.write().unwrap();
            let area_lookup_ref = self.citizen_output_area_lookup.read().unwrap();
            let area_id = area_lookup_ref.get(&citizen_id).context(format!("Citizen {}, does not exist in Output Area Lookup", citizen_id))?;
            let area_id = area_id.lock().unwrap();
            let mut area = area_ref.get_mut(&area_id).context(format!("Area id {} does not exist!", area_id))?;
            let mut area = area.lock().unwrap();
            let citizen = area.citizens.get_mut(&citizen_id);


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
                    return Err(SimError::MissingCitizen { citizen_id: citizen_id.to_string() }).context("Cannot expose Citizen, as they do not exist!");
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
                    self.output_areas.write().expect("Failed to retrive global Citizen lock").par_iter_mut().for_each(|(id, area)| {
                        let mut area = area.lock().unwrap();
                        // Send every Citizen home
                        for (_id, mut citizen) in area.citizens.iter_mut() {
                            ;
                            let home = citizen.household_code.clone();
                            citizen.current_building_position = home;
                        }
                    });
                }
                InterventionsEnabled::Vaccination => {
                    info!(
                        "Starting vaccination program at hour: {}",
                        self.statistics.time_step()
                    );
                    let mut eligible = self.output_areas.write().expect("Failed to retrive global Citizen lock").par_iter_mut().fold(|| HashSet::new(), |mut accum, (id, area)| {
                        let area = area.lock().unwrap();
                        area.citizens.iter().for_each(|(id, citizen)| {
                            if citizen.disease_status == DiseaseStatus::Susceptible {
                                accum.insert(*id);
                            }
                        });

                        accum
                    }).reduce(|| HashSet::new(), |mut a, b| {
                        for entry in b {
                            a.insert(entry);
                        }
                        a
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
                let citizen_lookup_ref = self.citizen_output_area_lookup.read().unwrap();

                let area_id = citizen_lookup_ref.get(&citizen_id).context("Citizen doesn't belong to an Output Area!")?;
                let area_id = area_id.lock().unwrap();

                let areas_ref = self.output_areas.read().unwrap();

                let output_area_ref = areas_ref.get(&area_id).context("Area doesn't exist!")?;
                let mut output_area_ref = output_area_ref.lock().unwrap();

                let citizen = output_area_ref.citizens.get_mut(&citizen_id).context("Citizen '{}' due to be vaccinated, doesn't exist!")?;

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
        for area in self.output_areas.read().unwrap().iter() {
            writeln!(file, "Output Area: {}", area.0)?;
            for building in area.1.lock().unwrap().buildings.values() {
                writeln!(file, "      {}", building)?;
            }
        }/*
        for citizen in self.citizens.read().unwrap().iter() {
            writeln!(file, "    {}", citizen.1.lock().unwrap())?;
        }*/
        writeln!(file, "\n\n\n----------\n\n\n")?;
        Ok(())
    }
    pub fn error_dump_json(self) -> anyhow::Result<()> {
        println!("Creating Core Dump!");
        let mut file = File::create("../../debug_dumps/crash.json")?;
        /*        use serde_json::json;

                let mut output_area_json = HashMap::new();
                for area in self.output_areas.read().unwrap().iter() {
                    output_area_json.insert(area.0, area.1.buildings);
                }
                let mut citizens = HashMap::new();
                /*for (id, citizen) in self.citizens.read().unwrap().iter() {
                    let citizen = citizen.lock().unwrap();
                    citizens.insert(id.to_string(), citizen.clone());
                }*/
                file.write_all(
                    json!({"citizens":citizens,"output_areas":output_area_json})
                        .to_string()
                        .as_ref(),
                )?;*/

        Ok(())
    }
}

impl From<SimulatorBuilder> for Simulator {
    fn from(builder: SimulatorBuilder) -> Self {
        let mut citizen_output_area_lookup = HashMap::with_capacity(builder.citizens.len());
        builder.output_areas.iter().for_each(|(area_id, area)| { area.citizens.iter().for_each(|(id, citizen)| { citizen_output_area_lookup.insert(id.clone(), Mutex::new(area_id.clone())); }) });
        let output_areas = RwLock::new(builder.output_areas.into_par_iter().map(|(id, area)| (id, Mutex::new(area))).collect());

        Simulator {
            current_population: citizen_output_area_lookup.len() as u32,
            output_areas,
            citizen_output_area_lookup: RwLock::new(citizen_output_area_lookup),
            citizens_eligible_for_vaccine: None,
            statistics: Statistics::default(),
            interventions: Default::default(),
            disease_model: builder.disease_model,
            public_transport: Default::default(),
            rng: thread_rng(),
        }
    }
}
