#![allow(dead_code)]

extern crate dotenv;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;

mod simulator;
mod disease;
mod models;
mod error;
mod config;

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
    //let census_table="data/tables/PopulationAndDensityPerEnglandOutputArea(144)-35645376-Records.csv".to_string();
    let census_table = "data/tables/york_population_144.csv".to_string();
    let output_map = "data/census_map_areas/England_oa_2011/england_oa_2011.shp".to_string();
    let sim = simulator::Simulator::new(census_table, output_map);
    let mut sim = sim.unwrap();
    info!("Starting simulator...");
    sim.simulate().unwrap();
    info!("Finished");
}
