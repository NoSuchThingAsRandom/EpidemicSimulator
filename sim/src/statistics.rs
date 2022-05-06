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
use std::fs;
use std::fs::File;
use std::io::BufWriter;

use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::to_writer;

use crate::disease::DiseaseStatus;
use crate::models::building::BuildingID;
use crate::models::citizen::Citizen;
use crate::models::ID;
use crate::models::output_area::OutputAreaID;
use crate::models::public_transport_route::PublicTransportID;

#[derive(Hash, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub enum StatisticsArea {
    Building { id: BuildingID },
    OutputArea { code: OutputAreaID },
    PublicTransport { id: (OutputAreaID, OutputAreaID) },
    All,
}


#[derive(Debug, Deserialize, Serialize, Default)]
pub struct StatisticsRecorder {
    all_data: HashMap<StatisticsArea, Vec<StatisticEntry>>,
    current_time_step: u32,
    pub current_entry: HashMap<StatisticsArea, StatisticEntry>,
}


impl StatisticsRecorder {
    pub fn dump_to_file(&mut self, directory: String) {
        // Flush the recordings
        self.next();
        fs::create_dir_all(directory.clone()).expect(&format!("Failed to create statistics directory: '{}'", directory));
        let file = File::create(directory.to_owned() + "exposures.json").expect("Failed to create results file!");
        let file_writer = BufWriter::new(file);
        let mut data: HashMap<&str, HashMap<String, Vec<StatisticEntry>>> = HashMap::new();
        for (place, records) in self.all_data.drain() {
            match place {
                StatisticsArea::Building { id } => {}
                StatisticsArea::OutputArea { code } => {
                    let mut entry = data.entry("OutputArea").or_default();
                    entry.insert(code.code().to_string(), records);
                }
                StatisticsArea::PublicTransport { id } => {
                    let mut entry = data.entry("PublicTransport").or_default();
                    let code = String::new() + id.0.code() + "-" + id.1.code();
                    //entry.insert(code, records);
                }
                StatisticsArea::All => {
                    let mut entry = data.entry("All").or_default();
                    entry.insert("All".to_string(), records);
                }
            }
        }
        to_writer(file_writer, &data).expect("Failed to write to file!");
    }
    pub fn current_time_step(&self) -> u32 {
        self.current_time_step
    }
    pub fn record(&mut self) {
        self.current_time_step += 1;
        for (area, entry) in self.current_entry.drain() {
            let mut recording_entry = self.all_data.entry(area).or_default();//tatisticEntry::with_time_step(self.current_time_step));
            recording_entry.push(entry);
        }
    }

    /// Prepares for recording the next step
    pub fn next(&mut self) {
        self.record();
    }
    pub fn add_citizen(&mut self, citizen: &Citizen) {
        let time_step = self.current_time_step();
        let stat_entry = self.current_entry.entry(StatisticsArea::All).or_insert_with(|| StatisticEntry::with_time_step(time_step));
        stat_entry.add_citizen(&citizen.disease_status);
        if let Some(id) = &citizen.on_public_transport {
            let stat_entry = self.current_entry.entry(StatisticsArea::PublicTransport { id: id.clone() }).or_insert_with(|| StatisticEntry::with_time_step(time_step));
            stat_entry.add_citizen(&citizen.disease_status);
        } else {
            let area = StatisticsArea::OutputArea { code: citizen.current_building_position.output_area_code().clone() };
            let stat_entry = self.current_entry.entry(area).or_insert_with(|| StatisticEntry::with_time_step(time_step));
            stat_entry.add_citizen(&citizen.disease_status);
        }
    }
    pub fn expose_citizen(&mut self, citizen: &Citizen, location: &ID) -> Result<(), crate::error::Error> {
        let time_step = self.current_time_step();
        let stat_entry = self.current_entry.entry(StatisticsArea::All).or_insert_with(|| StatisticEntry::with_time_step(time_step));
        stat_entry.citizen_exposed()?;
        match location {
            ID::Building(id) => {
                let area = StatisticsArea::OutputArea { code: id.output_area_code() };
                let stat_entry = self.current_entry.entry(area).or_insert_with(|| StatisticEntry::with_time_step(time_step));
                stat_entry.citizen_exposed()?;
            }
            ID::OutputArea(id) => {
                let area = StatisticsArea::OutputArea { code: id.clone() };
                let stat_entry = self.current_entry.entry(area).or_insert_with(|| StatisticEntry::with_time_step(time_step));
                stat_entry.citizen_exposed()?;
            }
            ID::PublicTransport(id) => {
                let area = StatisticsArea::PublicTransport { id: (id.source.clone(), id.destination.clone()) };
                let stat_entry = self.current_entry.entry(area).or_insert_with(|| StatisticEntry::with_time_step(time_step));
                stat_entry.citizen_exposed()?;
            }
        }
        Ok(())
    }

    pub fn disease_exists(&self) -> bool {
        self.current_entry.get(&StatisticsArea::All).expect("No data recorded").disease_exists()
    }

    pub fn infected_percentage(&self) -> f64 {
        self.current_entry.get(&StatisticsArea::All).expect("No data recorded").infected_percentage()
    }
}

/// A snapshot of the disease per time step
#[derive(Debug, Serialize, Deserialize)]
pub struct StatisticEntry {
    time_step: u32,
    susceptible: u32,
    exposed: u32,
    infected: u32,
    recovered: u32,
    pub vaccinated: u32,
}/*

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
*/

impl StatisticEntry {
    pub fn with_time_step(time_step: u32) -> Self {
        Self {
            time_step,
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
    pub fn citizen_exposed(&mut self) -> Result<(), crate::error::Error> {
        let x = self.susceptible.checked_sub(1);
        if let Some(x) = x {
            self.susceptible = x;
            self.exposed += 1;
            Ok(())
        } else {
            error!("Cannot log citizen being exposed, as no susceptible citizens left");
            Err(crate::error::Error::new_simulation_error(String::from(
                "Cannot expose citizen as no citizens are susceptible!",
            )))
        }
    }
    /// Returns true if at least one Citizen has the Disease
    pub fn disease_exists(&self) -> bool {
        self.exposed != 0 || self.infected != 0 || self.susceptible != 0
    }
    /*
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
        }*/
}


impl Display for StatisticEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hour: {: >4}, Total: {: >8}, Susceptible: {: >8}, Exposed: {: >8}, Infected: {: >8}, Recovered: {: >8} Vaccinated: {: >8}",
            self.time_step, self.total(), self.susceptible(), self.exposed(), self.infected(), self.recovered(), self.vaccinated()
        )
    }
}
