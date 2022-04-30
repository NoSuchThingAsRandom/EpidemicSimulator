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
use std::ops::AddAssign;
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use anyhow::Context;
use log::{debug, error, info};
use rand::prelude::{IteratorRandom, SliceRandom};
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rayon::prelude::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator,
    IntoParallelRefMutIterator, ParallelIterator,
};

use crate::config::{DEBUG_ITERATION_PRINT, get_memory_usage};
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::disease::DiseaseStatus::Infected;
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
    /// The list of Citizens on Public Transport, grouped by their origin and destination,
    ///
    /// The bool represents whether a Citizen is infected
    public_transport_pre_generated: HashMap<(OutputAreaID, OutputAreaID), Vec<(CitizenID, bool)>>,
    /// The list of buildings, with the amount of exposures that occurred
    /// Output Area Index -> Building Exposure Index?
    building_exposure_list: Vec<HashMap<BuildingID, Vec<CitizenID>>>,
}

impl AddAssign for GeneratedExposures {
    fn add_assign(&mut self, rhs: Self) {
        for (building, citizens) in rhs.public_transport_pre_generated {
            let entry = self
                .public_transport_pre_generated
                .entry(building)
                .or_default();
            entry.extend(citizens);
        }
        if self.building_exposure_list.len() < rhs.building_exposure_list.len() {
            self.building_exposure_list.extend(vec![
                HashMap::new();
                rhs.building_exposure_list.len()
                    - self.building_exposure_list.len()
            ]);
        }
        for (area_index, exposures) in rhs.building_exposure_list.into_iter().enumerate() {
            let area_entry = self.building_exposure_list.get_mut(area_index).expect(
                "Can't combine Generated Exposures with differing amounts of Output Areas!",
            );
            for (building, citizens) in exposures {
                let area_entry = area_entry.entry(building).or_default();
                area_entry.extend(citizens);
            }
        }
    }
}

//#[derive(Clone)]
pub struct Simulator {
    pub area_code: String,
    /// This maps the String code of an Output Area to it's index
    pub output_area_lookup: HashMap<String, u32>,
    /// The total size of the population
    current_population: u32,
    /// A list of all the sub areas containing agents
    pub output_areas: RwLock<Vec<Mutex<OutputArea>>>,
    /// The Output Area ID, and Local Index for that Citizen
    pub citizen_output_area_lookup: RwLock<Vec<Mutex<(OutputAreaID, u32)>>>,
    pub citizens_eligible_for_vaccine: Option<HashSet<CitizenID>>,
    statistics_recorder: StatisticsRecorder,
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
        info!(
            "Starting simulation with {} areas",
            self.output_areas.read().unwrap().len()
        );
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
        self.statistics_recorder.dump_to_file(&("statistics_results/v1.6/viking/".to_string() + &self.area_code + "/"));
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
        let hour = self.statistics_recorder.time_step();
        let disease = &self.disease_model;
        let lockdown = self.interventions.lockdown_enabled();
        let mut output_areas = self.output_areas.write().unwrap();
        let mut citizen_lookup = self.citizen_output_area_lookup.write().unwrap();

        let output_area_count = output_areas.len();

        // Update the Position and Schedule of each Citizen
        // If a Citizen is changing area, then they are moved into `moved_citizens`
        // For any Citizens that are infected, build a list of infected buildings
        let (statistics, exposures, moved_citizens) = output_areas.par_iter_mut().map(|area| {
            let mut area = area.lock().unwrap();
            let (mut statistics, mut exposures) = (StatisticEntry::with_time_step(hour), GeneratedExposures::default());
            // Apply timestep, and generate exposures
            let mut area_citizens = Vec::with_capacity(area.citizens.len());
            let mut moving_citizens: Vec<Vec<Citizen>> = vec![Vec::new(); output_area_count];
            let area_id = area.id();
            for mut citizen in area.citizens.drain(0..) {
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
                    let area_index = citizen.current_building_position.output_area_code().index();
                    if exposures.building_exposure_list.len() <= area_index {
                        exposures.building_exposure_list.extend(vec![HashMap::new(); (area_index - exposures.building_exposure_list.len()) + 1]);
                    }
                    let exposures_area_count = exposures.building_exposure_list.len();
                    let area_entry = exposures.building_exposure_list.get_mut(area_index).expect(&format!("Output area {} hasn't been initialised for exposures! Only {} areas have been created", area_index, exposures_area_count));
                    let entry = area_entry
                        .entry(citizen.current_building_position.clone())
                        .or_default();
                    entry.push(citizen.id());
                }
                if need_to_move {
                    let entry = moving_citizens.get_mut(citizen.current_building_position.output_area_code().index()).expect("Couldn't retrieve Output Area");
                    entry.push(citizen);
                } else {
                    let local_index = area_citizens.len();
                    match citizen_lookup.get(citizen.id().global_index()) {
                        Some(lookup_entry) => {
                            let mut lookup_entry = lookup_entry.lock().expect("Failed to retrieve citizen lock");
                            *lookup_entry = (area_id.clone(), local_index as u32);
                        }
                        None => {
                            panic!("Citizen {} does not have a lookup entry!", citizen.id());
                        }
                    }
                    area_citizens.push(citizen);
                }
            }
            area.citizens = area_citizens;
            (statistics, exposures, moving_citizens)
        }).reduce(|| (StatisticEntry::with_time_step(hour), GeneratedExposures::default(), vec![Vec::new(); output_area_count]), |(mut a_stats, mut a_exposures, mut a_to_move), (b_stats, b_exposures, b_to_move)| {
            a_stats += b_stats;
            a_exposures += b_exposures;
            if a_to_move.len() != b_to_move.len() {
                panic!("Moving Citizens is an invalid size! A: {}, B: {} ", a_to_move.len(), b_to_move.len());
            }
            for (area_index, citizens) in b_to_move.into_iter().enumerate() {
                let area = a_to_move.get_mut(area_index).expect("Generate Exposures have mismatched Area Counts");
                area.extend(citizens);
            }
            (a_stats, a_exposures, a_to_move)
        });
        // Move the Citizens to their new Output Area, and update the lookup table
        for (area_index, citizens) in moved_citizens.into_iter().enumerate() {
            match output_areas.get_mut(area_index) {
                Some(area) => {
                    let mut area = area
                        .lock()
                        .expect("Failed to retrieve lock for Output Area");
                    for mut citizen in citizens {
                        let local_index = area.citizens.len();
                        let id = citizen.id().clone();
                        area.citizens.push(citizen);
                        match citizen_lookup.get_mut(id.global_index()) {
                            Some(lookup_entry) => {
                                *lookup_entry = Mutex::new((area.id().clone(), local_index as u32));
                            }
                            None => {
                                panic!("Citizen {} does not have a lookup entry!", id);
                            }
                        }
                    }
                }
                None => error!(
                    "Area {} doesn't exist, need to move {} Citizens!",
                    area_index,
                    citizens.len()
                ),
            };
        }
        self.statistics_recorder.update_global_stats_entry(statistics);
        return Ok(exposures);
    }
    /// Applies the exposure cycle on any Citizens that have come in contact with an infected Citizen
    fn apply_exposures(&mut self, exposures: GeneratedExposures) -> anyhow::Result<()> {
        let disease = &self.disease_model;
        let mask_status = &self.interventions.mask_status;
        let output_areas = &self.output_areas;
        let citizen_lookup = &self.citizen_output_area_lookup;
        // Apply building exposures
        let exposure_statistics: Vec<ID> = exposures
            .building_exposure_list
            .par_iter()
            .enumerate()
            .map(|(area_index, building_exposures)| -> Vec<ID> {
                let mut exposures = Vec::new();
                let output_areas = output_areas.read().unwrap();
                let citizen_lookup = citizen_lookup
                    .read()
                    .expect("Failed to retrieve Citizen Lookup lock");
                let area = match output_areas
                    .get(area_index)
                    .context("Failed to retrieve Output Area")
                {
                    Ok(area) => area,
                    Err(e) => {
                        error!("{:?}", e);
                        return exposures;
                    }
                };
                let mut area = area.lock().unwrap();
                for (building_id, infected_citizens) in building_exposures {
                    let building =
                        &area
                            .buildings
                            .get(building_id.building_index())
                            .context(format!(
                                "Failed to retrieve exposure building {}",
                                building_id
                            ));
                    let building = match building {
                        Ok(building) => building,
                        Err(e) => {
                            error!("{:?}", e);
                            continue;
                        }
                    };

                    let building = building.as_ref();
                    let exposure_count = infected_citizens.len();
                    for citizen_id in building.find_exposures(infected_citizens) {
                        let lookup_ref = match citizen_lookup.get(citizen_id.global_index()) {
                            Some(lookup_ref) => lookup_ref,
                            None => {
                                error!("Citizen {} does not exist in global lookup!", citizen_id);
                                continue;
                            }
                        };
                        let lookup_ref = match lookup_ref.lock() {
                            Ok(lookup_ref) => lookup_ref,
                            Err(e) => {
                                error!("Failed to retrieve Citizen Lookup Lock: {}", e);
                                continue;
                            }
                        };
                        // If the Citizen is not currently in the Area, they haven't been exposed!
                        if lookup_ref.0.index() != area_index {
                            continue;
                        }
                        let citizen = match area
                            .citizens
                            .get_mut(lookup_ref.1 as usize)
                            .context("Cannot expose Citizen, as they do not exist!")
                        {
                            Ok(citizen) => citizen,
                            Err(_) => {
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
            })
            .flatten()
            .collect();
        for id in exposure_statistics {
            self.statistics_recorder.add_exposure(id)?;
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
                        if let Err(e) = self
                            .expose_citizens(
                                current_bus.occupants().clone(),
                                current_bus.exposure_count,
                                ID::PublicTransport(current_bus.id().clone()),
                            )
                            .context(format!("Failed to expose bus: {}", current_bus.id()))
                        {
                            error!("{:?}", e);
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
                if let Err(e) = self
                    .expose_citizens(
                        current_bus.occupants().clone(),
                        current_bus.exposure_count,
                        ID::PublicTransport(current_bus.id().clone()),
                    )
                    .context(format!("Failed to expose bus: {}", current_bus.id()))
                {
                    error!("{:?}", e);
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
        let mut area_ref = self.output_areas.write().unwrap();
        let citizen_lookup_ref = self.citizen_output_area_lookup.read().unwrap();
        for citizen_id in citizens {
            let citizen_ref =
                citizen_lookup_ref
                    .get(citizen_id.global_index())
                    .context(format!(
                        "Citizen {}, does not exist in Output Area Lookup",
                        citizen_id
                    ))?;
            let citizen_ref = citizen_ref.lock().unwrap();
            let area = area_ref
                .get_mut(citizen_ref.0.index())
                .context(format!("Area id {} does not exist!", citizen_ref.0))?;
            let mut area = area.lock().unwrap();
            let citizen = area.citizens.get_mut(citizen_ref.1 as usize);
            if citizen.is_none() {
                panic!(
                    "Citizen {:?} does not exist in Output Area {:?}!",
                    citizen_id, citizen_ref.0
                );
            }
            let citizen = citizen.unwrap();
            if citizen.is_susceptible()
                & &citizen.expose(
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
        Ok(())
    }

    fn apply_interventions(&mut self) -> anyhow::Result<()> {
        // TODO Check vaccinations -> `citizens_eligible_for_vaccine` still works?
        let infected_percent = self.statistics_recorder.infected_percentage();
        //debug!("Infected percent: {}",infected_percent);
        let new_interventions = self.interventions.update_status(infected_percent);
        for intervention in new_interventions {
            match intervention {
                InterventionsEnabled::Lockdown => {
                    info!(
                        "Lockdown is enabled at hour {}",
                        self.statistics_recorder.current_time_step()
                    );
                    // TODO THIS IS BROKEN, and Citizens are gonna get stuck...
                    self.output_areas
                        .write()
                        .expect("Failed to retrive global Citizen lock")
                        .par_iter_mut()
                        .for_each(|area| {
                            let mut area = area.lock().unwrap();
                            // Send every Citizen home
                            for citizen in area.citizens.iter_mut() {
                                let _home = citizen.household_code.clone();
                                //citizen.current_building_position = home;
                            }
                        });
                }
                InterventionsEnabled::Vaccination => {
                    // TODO Somehow more Citizens are vaccinated than the amount reduced from Susceptible? Suspicious....
                    info!(
                        "Starting vaccination program at hour: {}",
                        self.statistics_recorder.current_time_step()
                    );
                    let eligible = self
                        .output_areas
                        .write()
                        .expect("Failed to retrieve global Citizen lock")
                        .par_iter_mut()
                        .fold(
                            || HashSet::new(),
                            |mut accum, area| {
                                let area = area.lock().unwrap();
                                area.citizens.iter().for_each(|citizen| {
                                    if citizen.disease_status == DiseaseStatus::Susceptible {
                                        accum.insert(citizen.id());
                                    }
                                });
                                accum
                            },
                        )
                        .reduce(
                            || HashSet::new(),
                            |mut a, b| {
                                for entry in b {
                                    a.insert(entry);
                                }
                                a
                            },
                        );
                    self.citizens_eligible_for_vaccine = Some(eligible);
                }
                InterventionsEnabled::MaskWearing(status) => {
                    info!(
                        "Mask wearing status has changed: {} at hour {}",
                        status,
                        self.statistics_recorder.current_time_step()
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

                let citizen_ref = citizen_lookup_ref
                    .get(citizen_id.global_index())
                    .context("Can't find Output Area that Citizen belongs to!")?;
                let citizen_ref = citizen_ref.lock().unwrap();

                let areas_ref = self.output_areas.read().unwrap();

                let output_area_ref = areas_ref
                    .get(citizen_ref.0.index())
                    .context("Area doesn't exist!")?;
                let mut output_area_ref = output_area_ref.lock().unwrap();

                let citizen = output_area_ref
                    .citizens
                    .get_mut(citizen_ref.1 as usize)
                    .context("Citizen '{}' due to be vaccinated, doesn't exist!")?;

                citizen.disease_status = DiseaseStatus::Vaccinated;
            }
        }

        Ok(())
    }
}
/*
impl Simulator {
    pub fn error_dump(self) -> anyhow::Result<()> {
        println!("Creating Core Dump!");
        let file = File::create("crash.dump")?;
        let mut file = LineWriter::new(file);
        writeln!(file, "{}", self.statistics)?;
        for area in self.output_areas.read().unwrap().iter() {
            writeln!(file, "Output Area: {}", area)?;
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
}*/

impl From<SimulatorBuilder> for Simulator {
    fn from(builder: SimulatorBuilder) -> Self {
        let current_population = builder.citizen_output_area_lookup.len() as u32;
        let output_areas = RwLock::new(
            builder
                .output_areas
                .into_par_iter()
                .map(|area| Mutex::new(area))
                .collect(),
        );
        let citizen_output_area_lookup = RwLock::new(
            builder
                .citizen_output_area_lookup
                .into_par_iter()
                .map(|area_id| Mutex::new(area_id))
                .collect(),
        );

        let sim = Simulator {
            area_code: builder.area_code,
            output_area_lookup: builder.output_area_lookup,
            current_population,
            output_areas,
            citizen_output_area_lookup,
            citizens_eligible_for_vaccine: None,
            statistics_recorder: StatisticsRecorder::default(),
            interventions: Default::default(),
            disease_model: builder.disease_model,
            public_transport: Default::default(),
            rng: thread_rng(),
        };
        for (_code, index) in &sim.output_area_lookup {
            assert!(
                sim.output_areas
                    .read()
                    .unwrap()
                    .get(*index as usize)
                    .is_some(),
                "Output Area cannot be retrieved"
            );
        }
        sim
    }
}
