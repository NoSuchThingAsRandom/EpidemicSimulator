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

use std::collections::{HashMap, VecDeque};
use std::iter::FromIterator;
use std::time::Instant;

use geo::contains::Contains;
use geo_types::{Coordinate, LineString, point, Point};
use log::{debug, info, trace};
use ndarray::s;
use rand::prelude::IteratorRandom;
use rand::thread_rng;

const X_OFFSET: f64 = 3500000.0;
const X_SCALE: f64 = 50.0;
const Y_OFFSET: f64 = 20000.0;
const Y_SCALE: f64 = 25.0;

/// Utilises Jump Fill to build a Vorinni Diagram
#[derive(Clone)]
pub struct Vorinni {
    pub size: usize,
    pub grid: Vec<Vec<u32>>,
    pub seeds: Vec<(usize, usize)>,
    //, u32>,
    pub polygons: Vec<geo_types::Polygon<isize>>,
}

impl Vorinni {
    pub fn new(size: usize, seeds: Vec<(usize, usize)>) -> Vorinni {
        info!("Building Vorinni Grid of {} x {} with {} seeds",size,size,seeds.len());
        let seeds: Vec<(usize, usize)> = seeds.iter().choose_multiple(&mut thread_rng(), 100).iter().map(|p| **p).collect();
        trace!("Old Seeds:\n{:?}", seeds);
        let new_seeds = seeds.iter().map(|p| {
            let x = (p.0 as f64 - X_OFFSET) / X_SCALE;
            assert!(x > 0.0, "{}  is less than zero", x);
            let y = (p.1 as f64 - Y_OFFSET) / Y_SCALE;
            assert!(y > 0.0, "{}  is less than zero", y);
            voronoi::Point::new(x, y)
        }).collect();
        trace!("New Seeds:\n{:?}", new_seeds);
        let mut polygons = voronoi::make_polygons(&voronoi::voronoi(new_seeds, size as f64));
        // IT DOESN'T FUCKING CONNECT UP
        polygons.iter_mut().for_each(|poly| poly.push(*poly.first().unwrap()));
        debug!("Built {} polygons",polygons.len());
        for p in polygons.iter().choose_multiple(&mut thread_rng(), 20) {
            println!("{:?}", p);
        }
        let polygons: Vec<geo_types::Polygon<isize>> = polygons.iter().map(|points| geo_types::Polygon::new(LineString::from(points.iter().map(|point| geo_types::Point::new(point.x.round() as isize, point.y.round() as isize)).collect::<Vec<geo_types::Point<isize>>>()), Vec::new())).collect();
        let grid = Vorinni::flood_fill(size, &polygons);
        let mut vorinni = Vorinni {
            size,
            grid,
            seeds,
            polygons,// HashMap::from_iter(seeds.iter().enumerate().map(|(id, coords)| (*coords, id as u32))),
        };
        info!("Starting generation process");
        //vorinni.generate();
        vorinni
    }
    /// Uses Span Filling Algorithm to generate a fast lookup of closest locations
    ///
    /// https://en.wikipedia.org/wiki/Flood_fill
    pub fn flood_fill(size: usize, polygons: &Vec<geo_types::Polygon<isize>>) -> Vec<Vec<u32>> {
        let mut grid = vec![vec![0_u32; size]; size];
        for (id, polygon) in polygons.iter().enumerate() {
            let mut stack = VecDeque::new();
            let start = polygon.exterior().0.get(0).unwrap();
            if !polygon.contains(start) {
                continue;
            }
            stack.push_back((start.x, start.x, start.y, 1));
            stack.push_back((start.x, start.x, start.y - 1, -1));
            // TODO Occasionally stack size grows exponentially
            while let Some((mut x1, x2, y, dy)) = stack.pop_front() {
                if y < 0 {
                    continue;
                }
                let row = &mut grid[y as usize];

                let mut x = x1;
                if polygon.contains(&Point::new(x, y)) {
                    while polygon.contains(&Point::new(x - 1, y)) {
                        if 0 <= x {
                            row[x as usize] = id as u32;
                        }
                        x = x - 1;
                    }
                }
                if x < x1 {
                    stack.push_back((x, x1 - 1, y - dy, -dy));
                }
                while x1 < x2 {
                    while polygon.contains(&Point::new(x1, y)) {
                        if 0 <= x1 {
                            row[x1 as usize] = id as u32;
                        }
                        x1 = x1 + 1;
                    }
                    stack.push_back((x, x1 - 1, y + dy, dy));
                    if x1 - 1 > x2 {
                        stack.push_back((x2 + 1, x1 - 1, y - dy, -dy));
                    }
                    while x1 < x2 && !polygon.contains(&Point::new(x1, y)) {
                        x1 = x1 + 1;
                    }
                    x = x1;
                }
            }
            if id % 5 == 0 {
                debug!("Flood filled {} polygon",id);
                //Vorinni::print_grid(&grid);
            }
        }
        grid
    }

    fn print_grid(grid: &[Vec<u32>]) {
        println!("Grid\n-----------\n-----------\n-----------");
        for row in grid {
            for col in row {
                print!("{:>4} ", *col);
            }
            println!();
        }
        println!("\n-----------\n-----------\n-----------");
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