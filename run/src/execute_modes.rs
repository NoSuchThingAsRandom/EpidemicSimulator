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

use std::fmt::{Debug, Display, Formatter};
use std::time::Instant;

use anyhow::Context;
use log::{debug, error, info};

use load_census_data::CensusData;
use osm_data::draw_voronoi::draw_voronoi_polygons;
use osm_data::{OSMRawBuildings, OSM_CACHE_FILENAME, OSM_FILENAME};
use visualisation::image_export::DrawingRecord;

use crate::arguments::SimMode;
use crate::execute_modes::RuntimeError::MissingArguments;
use crate::{load_data, load_data_and_init_sim, Arguments};

pub enum RuntimeError {
    MissingArguments(String),
}

impl Debug for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::MissingArguments(err) => {
                write!(f, "Missing Arguments: {}", err)
            }
        }
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for RuntimeError {}

pub async fn download(arguments: Arguments) -> anyhow::Result<()> {
    info!("Downloading tables for area {}", arguments.area_code);
    if !arguments.allow_downloads {
        return Err(MissingArguments(
            "Cannot download tables without the download flag enabled!".to_string(),
        ))
        .context("Mode: Downloading tables");
    }
    CensusData::load_all_tables_async(
        arguments.data_directory,
        arguments.area_code,
        arguments.allow_downloads,
    )
    .await
    .context("Failed to load census data")?;
    Ok(())
}

pub async fn simulate(arguments: Arguments) -> anyhow::Result<()> {
    info!("Using mode simulate for area '{}'", arguments.area_code);
    let total_time = Instant::now();
    let output_directory = arguments.output_directory.clone();
    let mut sim = load_data_and_init_sim(arguments).await?;
    info!(
        "Finished loading data and Initialising  simulator in {:.2}",
        total_time.elapsed().as_secs_f64()
    );
    if let Err(e) = sim.simulate(output_directory) {
        error!("{}", e);
        //sim.error_dump_json().expect("Failed to create core dump!");
    } else {
        //sim.statistics.summarise();
    }

    info!("Finished in {:?}", total_time.elapsed());
    Ok(())
}

pub fn resume(_arguments: Arguments) -> anyhow::Result<()> {
    /*    let table =
        CensusTableNames::try_from(matches.value_of("table").expect("Missing table argument"))
            .expect("Unknown table");
    let row: usize = row.parse().expect("Row number is not an integer!");
    info!(
            "Resuming download of table {:?}, at row {} for area {}",
            table, row, area
        );
    CensusData::resume_download(&census_directory, area, table, row)
        .await
        .context("Failed to resume download of table")?;*/
    Ok(())
}

pub async fn visualise_output_areas(arguments: Arguments) -> anyhow::Result<()> {
    info!("Visualising map areas");
    let sim = load_data_and_init_sim(arguments).await?;

    let total_buildings = 100.0;
    let data: Vec<visualisation::image_export::DrawingRecord> = sim
        .output_areas
        .read()
        .unwrap()
        .iter()
        .map(|area| {
            let area = area.lock().unwrap();
            DrawingRecord::from((
                area.id().code().to_string(),
                (area.polygon.clone()),
                Some(area.buildings.len() as f64 / total_buildings),
            ))
        })
        .collect();
    visualisation::image_export::draw_output_areas(String::from("BuildingDensity.png"), data)?;

    Ok(())
}

pub async fn visualise_map(arguments: Arguments) -> anyhow::Result<()> {
    let (_census, mut osm, polygons) = load_data(arguments).await?;
    let polygon_data: Vec<visualisation::image_export::DrawingRecord> = polygons
        .polygons
        .iter()
        .map(|(code, area)| DrawingRecord::from((code.to_string(), (area), None, false)))
        .collect();
    let osm_buildings = osm
        .building_locations
        .drain()
        .map(|(_, b)| b)
        .flatten()
        .collect();
    visualisation::image_export::draw_buildings_and_output_areas(
        String::from("images/BuildingsAndOutputAreas.png"),
        polygon_data,
        osm_buildings,
    )?;
    Ok(())
}

pub fn visualise_buildings(arguments: Arguments) -> anyhow::Result<()> {
    let filename = arguments.data_directory.clone();
    let osm_data = OSMRawBuildings::build_osm_data(
        filename.to_string() + OSM_FILENAME,
        filename + OSM_CACHE_FILENAME,
        arguments.use_cache,
        arguments.grid_size,
    )
    .context("Failed to load OSM map")?;
    debug!("Starting drawing");
    for (k, p) in osm_data.voronoi().iter() {
        let polygons: Vec<&geo_types::Polygon<i32>> =
            p.polygons.polygons.iter().map(|(_, p)| p).collect();
        draw_voronoi_polygons(format!("images/{:?}Voronoi.png", k), &polygons, 20000);
    }
    Ok(())
}

pub fn _visualise_stuff(arguments: Arguments) -> anyhow::Result<()> {
    info!("Visualising buildings");
    let osm_buildings = OSMRawBuildings::build_osm_data(
        arguments.data_directory.to_string() + OSM_FILENAME,
        arguments.data_directory + OSM_CACHE_FILENAME,
        arguments.use_cache,
        arguments.grid_size,
    )
    .context("Failed to load OSM map")?
    .building_locations
    .drain()
    .map(|(_, b)| b)
    .flatten()
    .collect();
    visualisation::image_export::draw_buildings("raw_buildings.png".to_string(), osm_buildings)?;
    Ok(())
}

pub async fn execute_arguments(arguments: Arguments) -> anyhow::Result<()> {
    match arguments.mode {
        SimMode::Simulate => simulate(arguments).await?,
        SimMode::Render => {
            unimplemented!("Cannot use renderer on current Rust version (2018")
        }
        SimMode::Download => download(arguments).await?,
        SimMode::Resume => resume(arguments)?,
        SimMode::VisualiseMap => visualise_map(arguments).await?,
        SimMode::VisualiseOutputAreas => visualise_output_areas(arguments).await?,
        SimMode::VisualiseBuildings => visualise_buildings(arguments)?,
    }
    Ok(())
}
