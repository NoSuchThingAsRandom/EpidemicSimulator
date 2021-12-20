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
use std::iter::FromIterator;
use std::time::Instant;

use log::{debug, info, trace};
use ndarray::s;

/// Utilises Jump Fill to build a Vorinni Diagram
#[derive(Clone)]
pub struct Vorinni {
    pub size: usize,
    pub grid: Vec<Vec<u32>>,
    pub seeds: Vec<(usize, usize)>,//, u32>,
}

impl Vorinni {
    pub fn new(size: usize, seeds: Vec<(usize, usize)>) -> Vorinni {
        info!("Building Vorinni Grid of {} x {}",size,size);
        let mut vorinni = Vorinni {
            size,
            grid: vec![vec![0; size]; size],
            seeds,// HashMap::from_iter(seeds.iter().enumerate().map(|(id, coords)| (*coords, id as u32))),
        };
        info!("Starting generation process");
        vorinni.generate();
        vorinni
    }
    pub fn generate(&mut self) {
        for step in 1..8 {
            let time = Instant::now();
            let N = self.size / (step * 2);
            self.new_step(N as isize);
            debug!("Completed step {} in {}",N,time.elapsed().as_secs());
        }
    }
    pub fn step(&mut self, step_size: isize) {
        let mut new_grid = vec![vec![0; self.size]; self.size];
        let old_grid = self.grid.as_slice();
        for y in 0..self.grid.len() {
            let row = old_grid[y].as_slice();
            for x in 0..row.len() {
                let cell = row[x];
                for i in -step_size..step_size {
                    if i < 0 {
                        continue;
                    }
                    let i = i as usize;
                    for j in -step_size..step_size {
                        if j < 0 {
                            continue;
                        }
                        let j = j as usize;
                        let neighbour_colour = old_grid[y + j].as_slice()[x + i];
                        if cell == 0 && neighbour_colour != 0 || self.seed_distance(neighbour_colour, x + i, y + j) < self.seed_distance(cell, x, y) {
                            new_grid[y][x] = neighbour_colour;
                        }
                    }
                }
            }
        }
    }
    fn seed_distance(&self, id: u32, x: usize, y: usize) -> usize {
        let seed_location = self.seeds.get(id as usize).expect("Seed {} doesn't exist!");
        ((x as isize - seed_location.0 as isize).abs() + (y as isize - seed_location.1 as isize).abs()) as usize
    }
    pub fn new_step(&mut self, step_size: isize) {
        //let old_grid=self.grid.clone().as_slice();
        let mut time = Instant::now();
        let new_grid = self.grid.iter().enumerate().map(|(y, row)|
            {
                if y % 1000 == 0 {
                    trace!("At row: {}, Completed 1000 rows in time: {}",y,time.elapsed().as_secs());
                    time = Instant::now();
                }
                return row.iter().enumerate().map(|(x, cell)|
                    {
                        let mut new_colour = *cell;
                        for i in -step_size..step_size {
                            if x as isize + i < 0 {
                                continue;
                            }
                            let i = i as usize;
                            for j in -step_size..step_size {
                                if y as isize + j < 0 {
                                    continue;
                                }
                                let j = j as usize;
                                let neighbour_id = self.grid[y + j].as_slice()[x + i];
                                if new_colour == 0 && neighbour_id != 0 || self.seed_distance(*cell, x + i, y + j) < self.seed_distance(neighbour_id, x, y) {
                                    new_colour = neighbour_id;
                                }
                            }
                        }
                        new_colour
                    }
                ).collect::<Vec<u32>>();
            }
        ).collect::<Vec<Vec<u32>>>();
        self.grid = new_grid;
    }
}
/*
fn get_colour(x: usize, y: usize, cell: u32, step_size: isize) ->u32{
    let mut new_colour = cell;
    for i in -step_size..step_size {
        if i < 0 {
            continue;
        }
        let i = i as usize;
        for j in -step_size..step_size {
            if j < 0 {
                continue;
            }
            let j = j as usize;
            let neighbour_colour = old_grid[y + j].as_slice()[x + i];
            if new_colour == 0 && neighbour_colour != 0 {
                new_colour = neighbour_colour;
            } else {
                if self.seed_distance(x + i, y + j) < self.seed_distance(x, y) {
                    new_colour = neighbour_colour;
                }
            }
        }
    }
    new_colour
}


*/