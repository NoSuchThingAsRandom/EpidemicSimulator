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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use anyhow::Context;
use log::{error, info};
use rand::thread_rng;

use load_census_data::CensusData;
use load_census_data::tables::CensusTableNames;
use osm_data::{OSM_CACHE_FILENAME, OSM_FILENAME, OSMRawBuildings, RawBuilding};
use osm_data::polygon_lookup::PolygonContainer;
use sim::models::output_area::OutputAreaID;
use sim::simulator::Simulator;
use sim::simulator_builder::SimulatorBuilder;
use visualisation::image_export::{draw_buildings_and_output_areas, DrawingRecord};

use crate::visualise::draw_output_areas;

pub async fn load_data(
    area: String,
    census_directory: String,
    grid_size: i32,
    use_cache: bool,
    allow_downloads: bool,
    visualise_building_boundaries: bool,
) -> anyhow::Result<(CensusData, OSMRawBuildings, PolygonContainer<String>)> {
    let _census_data: Option<anyhow::Result<CensusData>> = None;
    let mut osm_buildings: Option<anyhow::Result<OSMRawBuildings>> = None;
    let mut output_area_polygons: Option<anyhow::Result<PolygonContainer<String>>> = None;
    let census_data = Some(CensusData::load_all_tables_async(
        census_directory.to_string(),
        area.to_string(),
        allow_downloads,
    ).await.context("F"));
    rayon::scope(|s| {
        // Load census data
        let _filename = census_directory.clone();
        /*        s.spawn(|_| async {
                    let census_closure = async move || -> anyhow::Result<CensusData> {
                        let census_data = CensusData::load_all_tables_async(
                            filename.to_string(),
                            area.to_string(),
                            allow_downloads,
                        ).await;
                        census_data.context("Failed to load census data")
                    };
                    census_data = Some(census_closure().await);
                });*/

        // Load OSM Buildings
        s.spawn(|_| {
            let filename = census_directory.clone();
            let buildings = move || -> anyhow::Result<OSMRawBuildings> {
                OSMRawBuildings::build_osm_data(
                    filename.to_string() + OSM_FILENAME,
                    filename + OSM_CACHE_FILENAME,
                    use_cache,
                    visualise_building_boundaries,
                    grid_size,
                )
                    .context("Failed to load OSM map")
            };
            osm_buildings = Some(buildings());
        });

        // Build output area polygons
        s.spawn(|_| {
            let polygon = move || -> anyhow::Result<PolygonContainer<String>> {
                PolygonContainer::load_polygons_from_file(
                    CensusTableNames::OutputAreaMap.get_filename(), grid_size,
                )
                    .context("Loading polygons for output areas")
            };
            output_area_polygons = Some(polygon());
        });
    });
    let (census_data, osm_buildings, output_area_polygons) = (
        census_data.expect("Census Data hasn't been executed!")?,
        osm_buildings.expect("OSM Buildings Data hasn't been executed!")?,
        output_area_polygons.expect("Output Area Polygons hasn't been executed!")?,
    );
    Ok((census_data, osm_buildings, output_area_polygons))
}

pub async fn load_data_and_init_sim(
    area: String,
    census_directory: String,
    use_cache: bool,
    allow_downloads: bool,
    visualise_building_boundaries: bool, grid_size: i32,
) -> anyhow::Result<Simulator> {
    info!("Loading data from disk...");
    let (census_data, osm_buildings, output_area_polygons) = load_data(
        area,
        census_directory, grid_size,
        use_cache,
        allow_downloads,
        visualise_building_boundaries,
    )
        .await?;
    let mut sim = SimulatorBuilder::new(census_data, osm_buildings, output_area_polygons)
        .context("Failed to initialise sim")
        .unwrap();
    sim.build().context("Failed to initialise sim").unwrap();
    Ok(Simulator::from(sim))
}
