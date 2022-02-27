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
use std::fmt::Debug;
use std::fs::File;
use std::hash::Hash;
use std::io::{BufWriter, Write};

use anyhow::Context;
use bimap::BiMap;
use log::info;
use petgraph::{Directed, EdgeType, Undirected};
use petgraph::dot::Config::{EdgeNoLabel, NodeIndexLabel};
use petgraph::graphmap::GraphMap;

use load_census_data::tables::resides_vs_workplace::WorkplaceResidentialRecord;
use sim::models::citizen::{Citizen, CitizenID};

pub fn build_citizen_graph(
    simulation: &sim::simulator::Simulator,
) -> GraphMap<u128, u8, Undirected> {
    let area_ref = simulation.output_areas.read().unwrap();
    let citizens = simulation.citizen_output_area_lookup.read().unwrap();
    let mut graph: GraphMap<u128, u8, Undirected> =
        GraphMap::with_capacity(citizens.len(), 20 * citizens.len());

    area_ref.iter().for_each(|area| {
        let area = area.lock().unwrap();
        area.citizens.iter().for_each(|citizen| {
            graph.add_node(citizen.id().uuid_id().as_u128());
            for (building) in &area.buildings {
                let citizens = building.occupants();
                for outer_citizen in &citizens {
                    for inner_citizen in &citizens {
                        graph.add_edge(
                            outer_citizen.uuid_id().as_u128(),
                            inner_citizen.uuid_id().as_u128(),
                            1,
                        );
                    }
                }
            }
        });
    });
    graph
}

pub fn build_workplace_output_area_graph(
    residents_workplace: HashMap<String, WorkplaceResidentialRecord>,
) -> GraphMap<u32, u32, Directed> {
    // Stupid workaround because String doesn't have copy
    let mut area_map_lookup = BiMap::with_capacity(residents_workplace.len());
    let mut graph: GraphMap<u32, u32, Directed> =
        GraphMap::with_capacity(residents_workplace.len(), 20 * residents_workplace.len());

    residents_workplace.keys().for_each(|area| {
        let id = area_map_lookup.len() as u32 + 1;
        area_map_lookup.insert(id, area.to_string());
        graph.add_node(id);
    });
    residents_workplace.iter().for_each(|(household_code, record)| {
        let house_id = area_map_lookup.get_by_right(household_code).expect("Household code doesn't exist!");
        for (area, amount) in &record.workplace_count {
            let work_id = area_map_lookup.get_by_right(area).expect("Workplace code doesn't exist!");
            graph.add_edge(*house_id, *work_id, *amount);
        }
    }
    );
    graph
}

pub fn build_building_graph(
    simulation: &sim::simulator::Simulator,
) -> GraphMap<u128, u8, Undirected> {
    let area_ref = simulation.output_areas.read().unwrap();
    let citizens: Vec<Citizen> = area_ref.iter().map(|area| area.lock().unwrap().citizens.clone()).flatten().collect();
    let mut graph: GraphMap<u128, u8, Undirected> =
        GraphMap::with_capacity(citizens.len(), 20 * citizens.len());

    citizens.iter().for_each(|citizen| {
        let weight = graph.edge_weight_mut(
            citizen.household_code.building_id().as_u128(),
            citizen.workplace_code.building_id().as_u128(),
        );
        if let Some(weight) = weight {
            *weight += 1;
        } else {
            graph.add_edge(
                citizen.household_code.building_id().as_u128(),
                citizen.workplace_code.building_id().as_u128(),
                1,
            );
        }
    });
    graph
}

pub fn connected_groups(graph: &GraphMap<u128, u8, Undirected>) -> usize {
    petgraph::algo::connected_components(graph)
}

pub fn draw_graph<T: Copy + Ord + Hash + Debug, U: Copy + Ord + Hash + Debug, V: EdgeType>(filename: String, graph: GraphMap<T, U, V>) -> anyhow::Result<()> {
    let dot = petgraph::dot::Dot::with_config(&graph, &[NodeIndexLabel, EdgeNoLabel]);
    info!("Creaeting file: {}",filename);
    let file = File::create(filename.to_string())
        .context(format!("Failed to create file: {}", filename))?;
    let mut writer = BufWriter::new(file);
    //writer.write_all(dot.);
    write!(writer, "{:?}", dot)?;
    info!("Dumped to fikle");
    writer.flush().expect("Failed to flush to file");
    Ok(())
}
