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

use std::collections::{HashMap, HashSet, VecDeque};
use std::iter::FromIterator;
use std::time::Instant;

use geo::contains::Contains;
use geo::prelude::BoundingRect;
use geo_types::{Coordinate, LineString, point, Point};
use log::{debug, info, trace};
use ndarray::s;
use rand::{Rng, thread_rng};
use rand::prelude::IteratorRandom;

const X_OFFSET: f64 = 3500000.0;
const X_SCALE: f64 = 50.0;
const Y_OFFSET: f64 = 10000.0;
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
        let seeds: Vec<(usize, usize)> = seeds.iter().choose_multiple(&mut thread_rng(), 500).iter().map(|p| **p).collect();
        let seeds: Vec<voronoi::Point> = seeds.iter().map(|p| {
            let x = (p.0 as f64 - X_OFFSET) / X_SCALE;
            assert!(x > 0.0, "X Value: {}  is less than zero", x);
            let y = (p.1 as f64 - Y_OFFSET) / Y_SCALE;
            assert!(y > 0.0, "Y Value: {}  is less than zero", y);
            voronoi::Point::new(x, y)
        }).collect();
        trace!("Processed Seeds: {:?}", seeds);
        let output_seeds = seeds.iter().map(|p| (p.x.round() as usize, p.y.round() as usize)).collect();
        let mut polygons = voronoi::make_polygons(&voronoi::voronoi(seeds, size as f64));
        // These polygons don't connect up, so add the final connection
        polygons.iter_mut().for_each(|poly| poly.push(*poly.first().unwrap()));
        debug!("Built {} polygons",polygons.len());
        let polygons: Vec<geo_types::Polygon<isize>> = polygons.iter().map(|points| geo_types::Polygon::new(LineString::from(points.iter().map(|point| geo_types::Point::new(point.x.round() as isize, point.y.round() as isize)).collect::<Vec<geo_types::Point<isize>>>()), Vec::new())).collect();
        let grid = Vorinni::span_flood_fill(size, &polygons);
        //Vorinni::print_grid(&grid);
        let mut vorinni = Vorinni {
            size,
            grid,
            seeds: output_seeds,
            polygons,
        };
        info!("Starting generation process");

        vorinni
    }

    fn flood_fill(size: usize, polygons: &Vec<geo_types::Polygon<isize>>) -> Vec<Vec<u32>> {
        let mut grid = vec![vec![0_u32; size]; size];
        for (id, polygon) in polygons.iter().enumerate() {
            let mut stack = VecDeque::new();
            let bounds = polygon.bounding_rect().expect("Failed to get boundary for polygon");
            let mut start = Point::default();
            let mut rng = thread_rng();
            let mut try_count = 0;
            if bounds.min().x == bounds.max().x || bounds.min().y == bounds.max().y {
                continue
            }
            while !polygon.contains(&start) {
                let x: isize = rng.gen_range(bounds.min().x..=bounds.max().x);
                let y: isize = rng.gen_range(bounds.min().y..=bounds.max().y);
                start = Point::new(x, y);
                try_count += 1;
                if try_count == 5 {
                    trace!("Fail");
                    break;
                }
            }
            let mut index = 0;
            trace!("Starting at pos: {:?}",start);
            stack.push_back((start.x(), start.y()));
            while let Some((x, y)) = stack.pop_front() {
                if polygon.contains(&Point::new(x, y)) && grid[y as usize][x as usize] != id as u32 {
                    grid[y as usize][x as usize] = id as u32;
                    stack.push_back((x + 1, y));
                    stack.push_back((x - 1, y));
                    stack.push_back((x, y + 1));
                    stack.push_back((x, y - 1));
                }
            }
            if id % 5 == 0 {
                debug!("Flood filled {} polygon",id);
                //Vorinni::print_grid(&grid);
            }
        }
        grid
    }
    /// Very slow - don't know if works
    fn span_flood_fill(size: usize, polygons: &Vec<geo_types::Polygon<isize>>) -> Vec<Vec<u32>> {
        let mut grid = vec![vec![0_u32; size]; size];
        let mut filled_count = 0;
        for (id, polygon) in polygons.iter().enumerate() {
            let mut stack = VecDeque::new();
            let bounds = polygon.bounding_rect().expect("Failed to get boundary for polygon");
            let mut start = Point::default();
            let mut rng = thread_rng();
            let mut try_count = 0;
            if bounds.min().x == bounds.max().x || bounds.min().y == bounds.max().y {
                continue
            }
            while !polygon.contains(&start) {
                let x: isize = rng.gen_range(bounds.min().x..=bounds.max().x);
                let y: isize = rng.gen_range(bounds.min().y..=bounds.max().y);
                start = Point::new(x, y);
                try_count += 1;
                if try_count == 5 {
                    trace!("Fail");
                    break;
                }
            }
            let mut index = 0;
            trace!("Starting at pos: {:?}",start);
            stack.push_back((start.x(), start.y()));

            while let Some((x, y)) = stack.pop_front() {
                let mut lx = x;
                let mut added_next_up_row = false;
                let mut added_next_down_row = false;
                while polygon.contains(&Point::new(lx, y)) {
                    //println!("LX At {} {} stack size: {}",lx,y,stack.len());
                    grid[y as usize][lx as usize] = id as u32;
                    filled_count += 1;
                    if polygon.contains(&Point::new(lx, y + 1)) {
                        if !added_next_up_row && grid[y as usize + 1][lx as usize] == 0 {
                            stack.push_back((lx, y + 1));
                            added_next_up_row = true;
                        }
                    } else {
                        added_next_up_row = false;
                    }
                    if polygon.contains(&Point::new(lx, y - 1)) {
                        if !added_next_down_row && grid[y as usize - 1][lx as usize] == 0 {
                            stack.push_back((lx, y - 1));
                            added_next_down_row = true;
                        }
                    } else {
                        added_next_down_row = false;
                    }

                    lx -= 1;
                }
                let mut rx = x;
                while polygon.contains(&Point::new(rx, y)) {
                    //println!("RX At {} {} stack size: {}",rx,y,stack.len());
                    grid[y as usize][rx as usize] = id as u32;
                    filled_count += 1;
                    if polygon.contains(&Point::new(rx, y + 1)) {
                        if !added_next_up_row && grid[y as usize + 1][rx as usize] == 0 {
                            stack.push_back((rx, y + 1));
                            added_next_up_row = true;
                        }
                    } else {
                        added_next_up_row = false;
                    }
                    if polygon.contains(&Point::new(rx, y - 1)) {
                        if !added_next_down_row && grid[y as usize - 1][rx as usize] == 0 {
                            stack.push_back((rx, y - 1));
                            added_next_down_row = true;
                        }
                    } else {
                        added_next_down_row = false;
                    }
                    rx += 1;
                }
                if filled_count % 100000 == 0 {
                    debug!("Filled {} pixels",filled_count);
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
                if *col != 0 {
                    print!("{:>4} ", *col);
                }
            }
            println!();
        }
        println!("\n-----------\n-----------\n-----------");
    }
}
