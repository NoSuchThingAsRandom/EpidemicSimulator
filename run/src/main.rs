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

use std::convert::TryFrom;
use std::time::Instant;

use anyhow::Context;
use clap::{App, Arg};
use log::{error, info};

use load_census_data::CensusData;
use load_census_data::tables::CensusTableNames;
use sim::simulator::Simulator;

//use visualisation::citizen_connections::{connected_groups, draw_graph};
//use visualisation::image_export::DrawingRecord;
#[allow(dead_code)]
fn get_bool_env(env_name: &str) -> anyhow::Result<bool> {
    std::env::var(env_name)
        .context(format!("Missing env variable '{}'", env_name))?
        .parse()
        .context(format!("'{}' is not a bool!", env_name))
}

#[allow(dead_code)]
fn get_string_env(env_name: &str) -> anyhow::Result<String> {
    std::env::var(env_name).context(format!("Missing env variable '{}'", env_name))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    pretty_env_logger::init_timed();
    let matches = App::new("Epidemic Simulation Using Census Data (ESUCD)")
        .version("1.0")
        .author("Sam Ralph <sr1474@york.ac.uk")
        .about("Simulates an Epidemic Using UK Census Data")
        .usage("run \"area_code\" --directory<data_directory> --mode
            \n    The area code which to use must be specified (area)\
            \n    The directory specifying where to store data must be specified (directory)\
            \n    There are 4 modes available to choose from:\
            \n        Download    ->      Downloads and Verifies the data files for a simulation\
            \n        Resume      ->      Used to resume a table download, it if failed for some reason\
            \n        Simulate    ->      Starts a text only logging simulation for the given area\
            \n        Render      ->      Starts a simulation with a live view of what is happening via a rendering engine\n")
        .arg(
            Arg::with_name("data_directory")
                .short("d")
                .long("directory")
                .help("The directory data files are located")
                .required(true)
                .require_equals(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("render")
                .short("r")
                .long("render")
                .help("Whether to use the rendering engine"),
        )
        .arg(
            Arg::with_name("simulate")
                .short("s")
                .long("simulate")
                .help("Whether to start a simulation")
                .requires("area"),
        )
        .arg(
            Arg::with_name("area")
                .help("Specifies the area code to simulate")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("table")
                .long("table")
                .help("Specifies the table name to download")
                .takes_value(true)
                .requires("area"),
        )
        .arg(
            Arg::with_name("download")
                .long("download")
                .help("Download's and verifies all tables for the given area")
                .requires("area")
                .conflicts_with_all(&["simulate", "render", "resume"]),
        )
        .arg(
            Arg::with_name("resume")
                .require_equals(true)
                .long("resume")
                .help("Specifies the row to resume downloading from")
                .takes_value(true)
                .requires_all(&["table", "area"])
                .conflicts_with_all(&["simulate", "render", "download"]),
        )
        .get_matches();

    let directory = matches
        .value_of("data_directory")
        .expect("Missing data directory argument");
    let census_directory = directory.to_owned() + "/tables/";
    let area = matches.value_of("area").expect("Missing area argument");
    info!("Using area: {}", area);
    if matches.is_present("download") {
        info!("Downloading tables for area {}", area);
        CensusData::load_all_tables_async(census_directory, area.to_string(), true)
            .await
            .context("Failed to load census data")
            .unwrap();
        Ok(())
    } else if let Some(row) = matches.value_of("resume") {
        let table =
            CensusTableNames::try_from(matches.value_of("table").expect("Missing table argument"))
                .expect("Unknown table");
        let row: usize = row.parse().expect("Row number is not an integer!");
        info!(
            "Resuming download of table {:?}, at row {} for area {}",
            table, row, area
        );
        CensusData::resume_download(&census_directory, area, table, row)
            .await
            .context("Failed to resume download of table")
    } else if matches.is_present("render") {
        unimplemented!("Cannot use renderer on current Rust version (2018")
    } else if matches.is_present("simulate") {
        info!("Using mode simulate for area '{}'", area);
        let total_time = Instant::now();
        info!("Loading data from disk...");
        let census_data =
            CensusData::load_all_tables_async(census_directory, area.to_string(), true)
                .await
                .context("Failed to load census data")
                .unwrap();
        info!(
            "Finished loading data in {:?},     Now Initialising  simulator",
            total_time.elapsed()
        );
        let mut sim = Simulator::new(census_data)
            .context("Failed to initialise sim")
            .unwrap();
        info!("Initialised simulator, starting sim...");
        if let Err(e) = sim.simulate() {
            error!("{}", e);
            //sim.error_dump_json().expect("Failed to create core dump!");
        } else {
            //sim.statistics.summarise();
        }
        info!("Finished in {:?}", total_time.elapsed());
        Ok(())
    } else {
        error!("No runtime option specified\nQuitting...");
        Ok(())
    }
}
/*
//TODO Enable when compiler on 2021
pub fn build_graphs(sim: &Simulator, save_to_file: bool) {
    let start = Instant::now();
    let graph = visualisation::citizen_connections::build_citizen_graph(sim);
    println!("Built graph in {:?}", start.elapsed());
    println!("There are {} nodes and {} edges", graph.node_count(), graph.edge_count());
    println!("There are {} connected groups", connected_groups(&graph));
    if save_to_file {
        let graph_viz = draw_graph("tiny_graphviz_no_label.dot".to_string(), graph);
    }
}

pub fn draw_census_data(
    census_data: &CensusData,
    output_areas_polygons: HashMap<String, Polygon<f64>>,
) -> anyhow::Result<()> {
    let data: Vec<visualisation::image_export::DrawingRecord> = census_data
        .population_counts
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(0.25),
                label: None,
            })
        })
        .collect();
    visualisation::image_export::draw(String::from("PopulationMap.png"), data)?;

    let data: Vec<DrawingRecord> = census_data
        .residents_workplace
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(0.6),
                label: None,
            })
        })
        .collect();
    visualisation::image_export::draw(String::from("ResidentsWorkplace.png"), data)?;

    let data: Vec<DrawingRecord> = census_data
        .occupation_counts
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(1.0),
                label: None,
            })
        })
        .collect();
    visualisation::image_export::draw(String::from("OccupationCounts.png"), data)?;
    Ok(())
}*/
