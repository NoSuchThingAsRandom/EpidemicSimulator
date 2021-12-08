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

use std::collections::HashMap;
use std::time::Instant;

use anyhow::Context;
use geo_types::Polygon;
use log::{error, info};

use load_census_data::CensusData;
use sim::simulator::Simulator;
use visualisation::citizen_connections::{connected_groups, draw_graph};
use visualisation::image_export::DrawingRecord;

const USE_RENDER: bool = false;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    let total_time = Instant::now();
    //load_census_data::CensusData::download_york_population().await;
    //SimpleLogger::new().init().unwrap();
    info!("Epidemic simulator");
    info!("Loading simulator data...");
    let sim = Simulator::new();
    let mut sim = sim.unwrap();
    build_graphs(&sim, false);
    info!("Starting simulator...");


    if USE_RENDER {
        visualisation::live_render::run(sim).context("Live render").unwrap();
    } else if let Err(e) = sim.simulate() {
        error!("{}", e);
        sim.error_dump_json().expect("Failed to create core dump!");
    } else {
        sim.statistics.summarise();
    }
    info!("Finished in {:?}",total_time.elapsed());
}

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

pub fn run_headless() {}

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
}
