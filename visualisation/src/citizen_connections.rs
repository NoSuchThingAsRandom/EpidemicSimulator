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

use std::fs::File;
use std::io::{BufWriter, Write};

use anyhow::Context;
use petgraph::dot::Config::{EdgeNoLabel, NodeIndexLabel};
use petgraph::graphmap::GraphMap;
use petgraph::Undirected;

pub fn build_citizen_graph(
    simulation: &sim::simulator::Simulator,
) -> GraphMap<u128, u8, Undirected> {
    let mut graph: GraphMap<u128, u8, Undirected> =
        GraphMap::with_capacity(simulation.citizens.len(), 20 * simulation.citizens.len());

    simulation.citizens.keys().for_each(|citizen| {
        graph.add_node(citizen.id().as_u128());
    });
    simulation.output_areas.values().for_each(|area| {
        for (_, building) in &area.buildings {
            let citizens = building.occupants();
            for outer_citizen in citizens {
                for inner_citizen in citizens {
                    graph.add_edge(
                        outer_citizen.id().as_u128(),
                        inner_citizen.id().as_u128(),
                        1,
                    );
                }
            }
        }
    });
    graph
}

pub fn build_building_graph(
    simulation: &sim::simulator::Simulator,
) -> GraphMap<u128, u8, Undirected> {
    let mut graph: GraphMap<u128, u8, Undirected> =
        GraphMap::with_capacity(simulation.citizens.len(), 20 * simulation.citizens.len());

    simulation.citizens.values().for_each(|citizen| {
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

pub fn draw_graph(filename: String, graph: GraphMap<u128, u8, Undirected>) -> anyhow::Result<()> {
    let dot = petgraph::dot::Dot::with_config(&graph, &[NodeIndexLabel, EdgeNoLabel]);
    let file = File::create(filename.to_string())
        .context(format!("Failed to create file: {}", filename))?;
    let mut writer = BufWriter::new(file);
    //writer.write_all(dot.);
    write!(writer, "{:?}", dot)?;
    Ok(())
}
