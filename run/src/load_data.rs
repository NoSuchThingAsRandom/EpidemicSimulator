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

use anyhow::Context;
use log::info;
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::BufReader;

use crate::Arguments;
use load_census_data::tables::CensusTableNames;
use load_census_data::CensusData;
use osm_data::polygon_lookup::PolygonContainer;
use osm_data::{OSMRawBuildings, OSM_CACHE_FILENAME, OSM_FILENAME};
use sim::interventions::{InterventionStatus, InterventionThresholds};
use sim::simulator::Simulator;
use sim::simulator_builder::SimulatorBuilder;

pub fn load_struct_from_file<T: DeserializeOwned>(filename: String) -> anyhow::Result<T> {
    let file = File::open(filename.to_string()).context(format!(
        "Failed to load config file: '{}'",
        filename.to_string()
    ))?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).context(format!("Failed to parse config file: '{}'", filename))
}

pub async fn load_data(
    arguments: Arguments,
) -> anyhow::Result<(CensusData, OSMRawBuildings, PolygonContainer<String>)> {
    let mut osm_buildings: Option<anyhow::Result<OSMRawBuildings>> = None;
    let mut output_area_polygons: Option<anyhow::Result<PolygonContainer<String>>> = None;
    let _filename = arguments.data_directory.clone();
    let area_code = arguments.area_code.clone();
    let allow_downloads = arguments.allow_downloads;
    let use_cache = arguments.use_cache;
    let grid_size = arguments.grid_size;
    let filename = _filename.clone();
    let census_data = CensusData::load_all_tables_async(filename, area_code, allow_downloads);

    rayon::scope(|s| {
        // Load OSM Buildings
        s.spawn(|_| {
            let filename = _filename.clone();
            let buildings = move || -> anyhow::Result<OSMRawBuildings> {
                OSMRawBuildings::build_osm_data(
                    filename.to_string() + OSM_FILENAME,
                    filename + OSM_CACHE_FILENAME,
                    use_cache,
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
                    CensusTableNames::OutputAreaMap.get_filename(),
                    grid_size,
                )
                .context("Loading polygons for output areas")
            };
            output_area_polygons = Some(polygon());
        });
    });
    let (census_data, osm_buildings, output_area_polygons) = (
        census_data.await?,
        osm_buildings.expect("OSM Buildings Data hasn't been executed!")?,
        output_area_polygons.expect("Output Area Polygons hasn't been executed!")?,
    );
    Ok((census_data, osm_buildings, output_area_polygons))
}

pub async fn load_data_and_init_sim(arguments: Arguments) -> anyhow::Result<Simulator> {
    info!(
        "Using config files: {} and {}",
        arguments.disease_config_filename, arguments.intervention_config_filename
    );
    let area_code = arguments.area_code.to_string();
    let disease_model = load_struct_from_file(arguments.disease_config_filename.to_string())
        .context("Failed to load Disease Model Config")?;
    let intervention_thresholds: InterventionThresholds =
        load_struct_from_file(arguments.intervention_config_filename.to_string())
            .context("Failed to load Intervention Config")?;
    let intervention_status = InterventionStatus::from(intervention_thresholds);
    info!("Starting to loading data from disk");
    let (census_data, osm_buildings, output_area_polygons) = load_data(arguments).await?;
    let mut sim = SimulatorBuilder::new(
        area_code,
        census_data,
        osm_buildings,
        output_area_polygons,
        disease_model,
        intervention_status,
    )
    .context("Failed to initialise sim")?;
    sim.build().context("Failed to initialise sim")?;
    Ok(Simulator::from(sim))
}
