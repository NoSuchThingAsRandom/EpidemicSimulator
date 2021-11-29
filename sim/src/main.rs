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
    //sim.simulate().unwrap();
    info!("Finished");
}
