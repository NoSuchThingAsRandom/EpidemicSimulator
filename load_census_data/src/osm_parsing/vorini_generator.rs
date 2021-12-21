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
        let grid = Vorinni::flood_fill(size, &polygons);
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
    /// Uses Span Filling Algorithm to generate a fast lookup of closest locations
    ///
    /// https://en.wikipedia.org/wiki/Flood_fill
    pub fn flood_fill_broke(size: usize, polygons: &Vec<geo_types::Polygon<isize>>) -> Vec<Vec<u32>> {
        let mut grid = vec![vec![0_u32; size]; size];
        for (id, polygon) in polygons.iter().enumerate() {
            let mut stack = VecDeque::new();
            let bounds = polygon.bounding_rect().expect("Failed to get boundary for polygon");
            let mut start = Point::default();
            let mut rng = thread_rng();
            while !polygon.contains(&start) {
                let x: isize = rng.gen_range(bounds.min().x..bounds.max().x);
                let y: isize = rng.gen_range(bounds.min().y..bounds.max().y);
                start = Point::new(x, y);
            }

            stack.push_back((start.x(), start.x(), start.y(), 1));
            stack.push_back((start.x(), start.x(), start.y() - 1, -1));
            let mut index = 0;
            while let Some((mut x1, x2, y, dy)) = stack.pop_front() {
                index += 1;
                if index % 100 == 0 {
                    let a = stack.iter();
                    let mut h = HashSet::with_capacity(a.len());
                    for p in a { h.insert(p); }
                    debug!("{}/{} Unique",h.len(),stack.len());
                }
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
                        x -= 1;
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
                        x1 += 1;
                    }
                    stack.push_back((x, x1 - 1, y + dy, dy));
                    if x1 - 1 > x2 {
                        stack.push_back((x2 + 1, x1 - 1, y - dy, -dy));
                    }
                    while x1 < x2 && !polygon.contains(&Point::new(x1, y)) {
                        x1 += 1;
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
    pub fn flood_fill_simplified(size: usize, polygons: &Vec<geo_types::Polygon<isize>>) -> Vec<Vec<u32>> {
        let mut grid = vec![vec![0_u32; size]; size];

        for (id, polygon) in polygons.iter().enumerate() {
            let mut stack = VecDeque::new();
            let bounds = polygon.bounding_rect().expect("Failed to get boundary for polygon");
            let mut start = Point::default();
            let mut rng = thread_rng();
            while !polygon.contains(&start) {
                let x: isize = rng.gen_range(bounds.min().x..bounds.max().x);
                let y: isize = rng.gen_range(bounds.min().y..bounds.max().y);
                start = Point::new(x, y);
            }
            debug!("Starting with point: {:?}",start);
            stack.push_back((start.x(), start.y()));

            let mut index = 0;
            while let Some((mut x, y)) = stack.pop_front() {
                index += 1;
                if index % 100 == 0 {
                    let a = stack.iter();
                    let mut h = HashSet::with_capacity(a.len());
                    for p in a { h.insert(p); }
                    debug!("{}/{} Unique",h.len(),stack.len());
                }
                if y < 0 {
                    continue;
                }
                let row = &mut grid[y as usize];

                let mut lx = x - 1;
                while polygon.contains(&Point::new(lx, y)) {
                    if 0 <= (lx) {
                        row[(lx) as usize] = id as u32;
                    }
                    lx -= 1;
                }
                let lx = x;

                while polygon.contains(&Point::new(x, y)) {
                    if 0 <= x {
                        row[x as usize] = id as u32;
                    }
                    x += 1;
                    ;
                }
                Vorinni::scan(polygon, lx, x - 1, y + 1, &mut stack);
                Vorinni::scan(polygon, lx, x - 1, y - 1, &mut stack);
            }
            if id % 5 == 0 {
                debug!("Flood filled {} polygon",id);
                //Vorinni::print_grid(&grid);
            }
        }
        let mut grid = vec![vec![0_u32; size]; size];
        grid
    }
    fn scan(polygon: &geo_types::Polygon<isize>, lx: isize, rx: isize, y: isize, stack: &mut VecDeque<(isize, isize)>) {
        let mut added = false;
        for x in lx..rx {
            if !polygon.contains(&Point::new(x, y)) {
                added = false;
            } else if !added {
                stack.push_back((x, y));
                added = true;
            }
        }
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
