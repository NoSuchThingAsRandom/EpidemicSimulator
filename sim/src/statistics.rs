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
use std::fs::File;
use std::io::BufWriter;
use std::ops::AddAssign;
use std::time::Instant;

use log::{error, info};
use num_format::Locale::{el, lo};
use num_format::ToFormattedString;
use serde::{Deserialize, Serialize};
use serde_json::to_writer;

use crate::config::{get_memory_usage, NUMBER_FORMATTING};
use crate::disease::DiseaseStatus;
use crate::error::SimError;
use crate::models::building::BuildingID;
use crate::models::citizen::Citizen;
use crate::models::ID;
use crate::models::output_area::OutputAreaID;
use crate::models::public_transport_route::PublicTransportID;

/// A simple struct for benchmarking how long a block of code takes
#[derive(Debug)]
pub struct Timer {
    function_timer: Instant,
    code_block_timer: Instant,
    pub function_times: HashMap<String, f64>,
}

impl Timer {
    /// Call this to record how long has elapsed since the last call
    #[inline]
    pub fn code_block_finished(&mut self, message: String) {
        let elapsed = self.code_block_timer.elapsed().as_secs_f64();
        self.function_times.insert(message, elapsed);
        self.code_block_timer = Instant::now();
    }
    /// Call this to record how long has elapsed since the last call
    #[inline]
    pub fn code_block_finished_with_print(&mut self, message: String) -> anyhow::Result<()> {
        let elapsed = self.code_block_timer.elapsed().as_secs_f64();
        println!(
            "{} in {:.2} seconds. Total function time: {:.2} seconds, Memory usage: {}",
            message,
            elapsed,
            self.function_timer.elapsed().as_secs_f64(),
            get_memory_usage()?
        );
        self.function_times.insert(message, elapsed);
        self.code_block_timer = Instant::now();
        Ok(())
    }
    pub fn finished(&mut self) -> HashMap<String, f64> {
        self.function_times.insert("total".to_string(), self.function_timer.elapsed().as_secs_f64());
        self.function_times.clone()
    }
    pub fn finished_with_print(&mut self, function_name: String) -> HashMap<String, f64> {
        self.function_times.insert("total".to_string(), self.function_timer.elapsed().as_secs_f64());
        println!("{} finished in {:.2} seconds", function_name, self.function_timer.elapsed().as_secs_f64());
        self.function_times.clone()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            function_timer: Instant::now(),
            code_block_timer: Instant::now(),
            function_times: Default::default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct StatisticsRecorder {
    #[serde(skip)]
    timer: Timer,
    timer_entries: Vec<HashMap<String, f64>>,
    memory_usage_entries: Vec<String>,
    current_time_step: u32,
    pub global_stats: Vec<StatisticEntry>,
    /// The infections counts per time step, per area
    all_stats_per_building: HashMap<ID, Vec<StatisticEntry>>,
    /// The time steps currently being altered
    pub current_entry: HashMap<ID, StatisticEntry>,
}


impl StatisticsRecorder {
    pub fn dump_to_file(&mut self, filename: &str) {
        // Flush the recordings
        self.next();
        let file = File::create(filename).expect("Failed to create results file!");
        let file_writer = BufWriter::new(file);
        let mut data: HashMap<&str, HashMap<String, Vec<StatisticEntry>>> = HashMap::new();
        for (place, records) in self.all_stats_per_building.drain() {
            let mut entry = data.entry("All").or_default();
            entry.insert("All".to_string(), records.clone());
            match place {
                ID::Building(id) => {}
                ID::OutputArea(code) => {
                    let mut entry = data.entry("OutputArea").or_default();
                    entry.insert(code.code().to_string(), records);
                }
                ID::PublicTransport(id) => {
                    let mut entry = data.entry("PublicTransport").or_default();
                    let code = String::new() + id.source.code() + "-" + id.destination.code();
                    //entry.insert(code, records);
                }
            }
        }
        info!("Dumped data to file: {}",filename);
        to_writer(file_writer, &data).expect("Failed to write to file!");
    }
    pub fn current_time_step(&self) -> u32 {
        self.current_time_step
    }

    /// Prepares for recording the next step
    pub fn next(&mut self) -> anyhow::Result<()> {
        // If we have started recording, update the previous data
        if !self.global_stats.is_empty() {
            self.timer_entries.push(self.timer.finished());
            self.memory_usage_entries.push(get_memory_usage()?);
            for (area, entry) in self.current_entry.drain() {
                let mut recording_entry = self.all_stats_per_building.entry(area).or_default();//tatisticEntry::with_time_step(self.current_time_step));
                recording_entry.push(entry);
            }
        }
        self.timer = Timer::default();
        self.current_time_step += 1;
        self.global_stats.push(StatisticEntry::with_time_step(self.current_time_step()));
        self.current_entry = HashMap::new();
        Ok(())
    }
    pub fn record_function_time(&mut self, function_name: String) {
        self.timer.code_block_finished(function_name)
    }

    /// Increment the current global stats, with the other
    pub fn update_global_stats_entry(&mut self, entry: StatisticEntry) {
        let mut current = self.global_stats.last_mut().expect("Need to call next() to start a recording!");
        *current += entry;
    }
    pub fn add_citizen(&mut self, disease_status: &DiseaseStatus) {
        self.global_stats.last_mut().expect("No global data recorded").add_citizen(disease_status)
    }
    pub fn add_exposure(&mut self, location: ID) -> Result<(), SimError> {
        self.global_stats.last_mut().expect("No global data recorded").citizen_exposed()?;
        // If building, expose the Output Area as well
        let time_step = self.current_time_step;
        let current_entry = &mut self.current_entry;
        if let ID::Building(building) = &location {
            let area_id = ID::OutputArea(building.output_area_code());
            let stat_entry = current_entry.entry(area_id).or_insert_with(|| StatisticEntry::with_time_step(time_step));
            stat_entry.citizen_exposed()?;
        }
        let stat_entry = current_entry.entry(location).or_insert_with(|| StatisticEntry::with_time_step(time_step));
        stat_entry.citizen_exposed()?;
        Ok(())
    }
    pub fn disease_exists(&self) -> bool {
        self.global_stats.last().expect("No data recorded").disease_exists()
    }

    pub fn infected_percentage(&self) -> f64 {
        self.global_stats.last().expect("No data recorded").infected_percentage()
    }
    pub fn time_step(&self) -> u32 { self.current_time_step }
}

/// A snapshot of the disease per time step
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatisticEntry {
    time_step: u32,
    susceptible: u32,
    exposed: u32,
    infected: u32,
    recovered: u32,
    pub vaccinated: u32,
}

impl StatisticEntry {
    pub fn with_time_step(hour: u32) -> StatisticEntry {
        StatisticEntry {
            time_step: hour,
            susceptible: 0,
            exposed: 0,
            infected: 0,
            recovered: 0,
            vaccinated: 0,
        }
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
    pub fn citizen_exposed(&mut self) -> Result<(), crate::error::SimError> {
        let x = self.susceptible.checked_sub(1);
        if let Some(x) = x {
            self.susceptible = x;
            self.exposed += 1;
            Ok(())
        } else {
            error!("Cannot log citizen being exposed, as no susceptible citizens left");
            Err(crate::error::SimError::new_simulation_error(String::from(
                "Cannot expose citizen as no citizens are susceptible!",
            )))
        }
    }
    /// Returns true if at least one Citizen has the Disease
    pub fn disease_exists(&self) -> bool {
        self.exposed != 0 || self.infected != 0 || self.susceptible != 0
    }
}

impl AddAssign for StatisticEntry {
    fn add_assign(&mut self, rhs: Self) {
        self.susceptible += rhs.susceptible;
        self.exposed += rhs.exposed;
        self.infected += rhs.infected;
        self.recovered += rhs.recovered;
        self.vaccinated += rhs.vaccinated;
    }
}

impl Default for StatisticEntry {
    fn default() -> Self {
        StatisticEntry {
            time_step: 0,
            susceptible: 0,
            exposed: 0,
            infected: 0,
            recovered: 0,
            vaccinated: 0,
        }
    }
}

impl Display for StatisticEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hour: {: >4}, Total: {: >10}, Susceptible: {: >10}, {:.2}%, Exposed: {: >10}, {:.2}%, Infected: {: >10}, {:.2}%, Recovered: {: >10}, {:.2}% Vaccinated: {: >10}, {:.2}%",
            self.time_step, self.total().to_formatted_string(&NUMBER_FORMATTING), self.susceptible().to_formatted_string(&NUMBER_FORMATTING), (self.susceptible() as f64 / self.total() as f64) * 100.0, self.exposed().to_formatted_string(&NUMBER_FORMATTING), (self.exposed() as f64 / self.total() as f64) * 100.0, self.infected().to_formatted_string(&NUMBER_FORMATTING), (self.infected() as f64 / self.total() as f64) * 100.0, self.recovered().to_formatted_string(&NUMBER_FORMATTING), (self.recovered() as f64 / self.total() as f64) * 100.0, self.vaccinated().to_formatted_string(&NUMBER_FORMATTING), (self.vaccinated() as f64 / self.total() as f64) * 100.0,
        )
    }
}
