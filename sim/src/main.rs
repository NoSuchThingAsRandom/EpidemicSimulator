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
#![allow(dead_code)]

extern crate dotenv;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;

mod config;
mod disease;
mod error;
mod models;
mod simulator;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    warn!("Test");
    //SimpleLogger::new().init().unwrap();
    info!("Epidemic simulator");
    info!("Download york population data...");
    //(load_census_data::download_york_population().await).unwrap();
    info!("Loading simulator data...");
    let sim = simulator::Simulator::new();
    let mut sim = sim.unwrap();
    info!("Starting simulator...");
    sim.simulate().unwrap();
    info!("Finished");
}
