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

use criterion::{black_box, Criterion, criterion_group, criterion_main};
use log::info;

use load_census_data::CensusData;
use sim::simulator::Simulator;

fn sim() {}

fn load_data(c: &mut Criterion) {
    let directory = "../data/copy/tables/".to_string();
    let area = "1946157112TYPE299".to_string();
    let area = "2013265923TYPE299".to_string();
    let census_data = CensusData::load_all_tables(directory, area, false)
        .unwrap();

    c.bench_function("Time Step", |b| b.iter(|| {
        let mut sim = Simulator::new(census_data.clone())
            .expect("Failed to initialise sim");
        for _ in 0..100 {
            sim.step();
        }
    }));
}

criterion_group!(benches, load_data);
criterion_main!(benches);