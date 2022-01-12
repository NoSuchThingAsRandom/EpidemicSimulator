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

use std::path::Path;

use cpuprofiler::PROFILER;
use criterion::{Criterion, criterion_group, criterion_main, SamplingMode};
use criterion::profiler::Profiler;

use load_census_data::{CensusData, OSM_CACHE_FILENAME, OSM_FILENAME};
use load_census_data::osm_parsing::OSMRawBuildings;
use load_census_data::polygon_lookup::PolygonContainer;
use load_census_data::tables::CensusTableNames;
use sim::simulator::Simulator;

struct MyProfiler {}

impl Profiler for MyProfiler {
    fn start_profiling(&mut self, benchmark_id: &str, benchmark_dir: &Path) {
        let path = format!("./profiling/{:?}/{}", benchmark_dir, benchmark_id);
        PROFILER.lock().unwrap().start(path).unwrap();
    }

    fn stop_profiling(&mut self, _benchmark_id: &str, _benchmark_dir: &Path) {
        PROFILER.lock().unwrap().stop().unwrap();
    }
}

fn load_census_data(c: &mut Criterion) {
    let directory = "../data/".to_string();
    let mut group = c.benchmark_group("census_tables");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);
    let area = "1946157112TYPE299".to_string();
    //let area = "2013265923TYPE299".to_string();

    group.bench_function("Load Census Tables", |b| b.iter(|| CensusData::load_all_tables(directory.clone(), area.clone(), false).unwrap()));
    // Load OSM Buildings
    group.bench_function("Load OSM Data", |b| b.iter(||
        OSMRawBuildings::build_osm_data(
            directory.to_string() + OSM_FILENAME,
            directory.to_string() + OSM_CACHE_FILENAME,
            false,
            false,
        )
    ));

    // Build output area polygons
    group.bench_function("Load Output Area Polygons", |b| b.iter(||
        PolygonContainer::load_polygons_from_file(
            CensusTableNames::OutputAreaMap.get_filename(),
        )
    ));

    group.finish();
}


fn profiled() -> Criterion {
    Criterion::default().with_profiler(MyProfiler {})
}
criterion_group! {
    name=benches;
    config=profiled();
    targets=load_census_data
}
criterion_main!(benches);
