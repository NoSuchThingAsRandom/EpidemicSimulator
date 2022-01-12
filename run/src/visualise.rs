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
use std::time::Instant;

use log::info;

use load_census_data::CensusData;
use sim::models::output_area::{OutputArea, OutputAreaID};
use sim::simulator::Simulator;
use visualisation::citizen_connections::{connected_groups, draw_graph};
use visualisation::image_export::DrawingRecord;

pub fn draw_output_areas(
    filename: String,
    sim: &HashMap<OutputAreaID, OutputArea>,
) -> anyhow::Result<()> {
    info!("Drawing Output Areas to: {}", filename);
    let data: Vec<visualisation::image_export::DrawingRecord> = sim
        .iter()
        .map(|(_, area)| {
            DrawingRecord::from((area.output_area_id.code().to_string(), &area.polygon, None))
        })
        .collect();
    visualisation::image_export::draw_output_areas(filename, data)?;
    Ok(())
}


//TODO Enable when compiler on 2021
pub fn build_graphs(sim: &Simulator, save_to_file: bool) {
    let start = Instant::now();
    let graph = visualisation::citizen_connections::build_citizen_graph(sim);
    println!("Built graph in {:?}", start.elapsed());
    println!(
        "There are {} nodes and {} edges",
        graph.node_count(),
        graph.edge_count()
    );
    println!("There are {} connected groups", connected_groups(&graph));
    if save_to_file {
        draw_graph("tiny_graphviz_no_label.dot".to_string(), graph).expect("Failed to draw graph viz");
    }
}

pub fn draw_census_data(
    census_data: &CensusData,
    output_areas_polygons: HashMap<String, geo_types::Polygon<f64>>,
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
                filled: false,
            })
        })
        .collect();
    visualisation::image_export::draw_output_areas(String::from("PopulationMap.png"), data)?;

    let data: Vec<DrawingRecord> = census_data
        .residents_workplace
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(0.6),
                label: None,
                filled: false,
            })
        })
        .collect();
    visualisation::image_export::draw_output_areas(String::from("ResidentsWorkplace.png"), data)?;

    let data: Vec<DrawingRecord> = census_data
        .occupation_counts
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(1.0),
                label: None,
                filled: false,
            })
        })
        .collect();
    visualisation::image_export::draw_output_areas(String::from("OccupationCounts.png"), data)?;
    Ok(())
}
