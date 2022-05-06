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

use load_census_data::CensusData;
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

fn load_data(c: &mut Criterion) {
    let directory = "../data/copy/tables/".to_string();
    let mut group = c.benchmark_group("bench-group");
    group.sampling_mode(SamplingMode::Flat);
    //let area = "1946157112TYPE299".to_string();
    let area = "2013265923TYPE299".to_string();
    let census_data = CensusData::load_all_tables(directory, area, false).unwrap();
    let mut sim = Simulator::new(area, census_data).expect("Failed to initialise sim");
    for _ in 0..540 {
        sim.step().expect("Sim step failed!");
    }
    println!("Starting benchmarks at: {}", sim.statistics);
    group.bench_function("Time Step", |b| b.iter(|| sim.step()));
    group.finish();
}

fn profiled() -> Criterion {
    Criterion::default().with_profiler(MyProfiler {})
}
criterion_group! {
    name=benches;
    config=profiled();
    targets=load_data
}
criterion_main!(benches);
