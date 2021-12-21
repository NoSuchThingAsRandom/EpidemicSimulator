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

use std::collections::{HashSet, VecDeque};
use std::fmt::Display;

use geo::centroid::Centroid;
use geo::contains::Contains;
use geo::prelude::BoundingRect;
use geo_types::{CoordNum, LineString, Point};
use log::{debug, info, trace};
use num_traits::PrimInt;
use quadtree_rs::{area::AreaBuilder, point::Point as QuadPoint, Quadtree};
use rand::{Rng, thread_rng};
use rand::prelude::IteratorRandom;

use crate::DataLoadingError;
use crate::parsing_error::ParseErrorType::MathError;

const X_OFFSET: f64 = 3500000.0;
const X_SCALE: f64 = 50.0;
const Y_OFFSET: f64 = 10000.0;
const Y_SCALE: f64 = 25.0;

/// Utilises Jump Fill to build a Vorinni Diagram
pub struct Vorinni {
    pub size: usize,
    pub lookup: Quadtree<isize, (usize, usize)>,
    pub seeds: Vec<(usize, usize)>,
    //, u32>,
    pub polygons: Vec<geo_types::Polygon<isize>>,
}

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_polygon_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(polygon: &geo_types::Polygon<T>) -> Result<quadtree_rs::area::Area<T>, DataLoadingError> {
    let bounds = polygon.bounding_rect().ok_or_else(|| DataLoadingError::ValueParsingError { source: MathError { context: "Failed to generate bounding box for polygon".to_string() } })?;
    let anchor = bounds.min();
    let anchor = (anchor.x, anchor.y);
    let mut height = bounds.height();
    if height <= T::zero() {
        height = T::one();
    }
    let mut width = bounds.width();
    if width <= T::zero() {
        width = T::one();
    }
    assert!(bounds.height() >= T::zero(), "Rect has a height less than zero {:?}", bounds);
    assert!(bounds.width() >= T::zero(), "Rect has a width less than zero {:?}", bounds);
    let area = AreaBuilder::default().anchor(QuadPoint::from(anchor)).dimensions((width, height)).build().unwrap();
    Ok(area)
}

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_point_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(point: &geo_types::Point<T>) -> Result<quadtree_rs::area::Area<T>, DataLoadingError> {
    let anchor = (point.x(), point.y());
    let area = AreaBuilder::default().anchor(QuadPoint::from(anchor)).build().unwrap();
    Ok(area)
}

fn get_random_point_inside_polygon(polygon: &geo_types::Polygon<isize>) -> Option<geo_types::Point<isize>> {
    let mut start = Point::default();
    let mut rng = thread_rng();
    let mut try_count = 0;
    let bounds = polygon.bounding_rect().unwrap();
    if bounds.min().x == bounds.max().x || bounds.min().y == bounds.max().y {
        return None;
    }
    while !polygon.contains(&start) {
        let x: isize = rng.gen_range(bounds.min().x..=bounds.max().x);
        let y: isize = rng.gen_range(bounds.min().y..=bounds.max().y);
        start = Point::new(x, y);
        try_count += 1;
        if try_count == 5 {
            return None;
        }
    }
    Some(start)
}

impl Vorinni {
    pub fn new(size: usize, seeds: Vec<(u32, (usize, usize))>) -> Result<Vorinni, DataLoadingError> {
        info!("Building Vorinni Grid of {} x {} with {} seeds",size,size,seeds.len());
        let seeds: Vec<(usize, usize)> = seeds.iter().choose_multiple(&mut thread_rng(), 500).iter().map(|(id, p)| *p).collect();
        let processed_seeds: Vec<voronoi::Point> = seeds.iter().map(|p| {
            let x = (p.0 as f64 - X_OFFSET) / X_SCALE;
            assert!(x > 0.0, "X Value: {}  is less than zero", x);
            let y = (p.1 as f64 - Y_OFFSET) / Y_SCALE;
            assert!(y > 0.0, "Y Value: {}  is less than zero", y);
            voronoi::Point::new(x, y)
        }).collect();
        trace!("Processed Seeds: {:?}", processed_seeds);
        let output_seeds = processed_seeds.iter().map(|p| (p.x.round() as usize, p.y.round() as usize)).collect();
        let mut polygons = voronoi::make_polygons(&voronoi::voronoi(processed_seeds, size as f64));
        // These polygons don't connect up, so add the final connection
        polygons.iter_mut().for_each(|poly| poly.push(*poly.first().unwrap()));
        debug!("Built {} polygons",polygons.len());
        let polygons: Vec<geo_types::Polygon<isize>> = polygons.iter().map(|points| geo_types::Polygon::new(LineString::from(points.iter().map(|point| geo_types::Point::new(point.x.round() as isize, point.y.round() as isize)).collect::<Vec<geo_types::Point<isize>>>()), Vec::new())).collect();
        //let grid = Vorinni::span_flood_fill(size, &polygons);
        let mut lookup: Quadtree<isize, (usize, usize)> = Quadtree::new((size as f64).log2().ceil() as usize);
        for (index, p) in polygons.iter().enumerate() {
            let seed = seeds.get(index).unwrap();
            lookup.insert(geo_polygon_to_quad_area(p)?, *seed);
        }
        let index = (0..polygons.len()).choose(&mut thread_rng()).unwrap();
        let seed = seeds.get(index).unwrap();
        let polygon = polygons.get(index).unwrap();
        let point = get_random_point_inside_polygon(polygon).unwrap();
        let results = lookup.query(geo_point_to_quad_area(&point)?);
        println!("Chose: {:?}, got: ", seed);
        for entry in results {
            println!("{:?}", entry.value_ref());
        }
        //Vorinni::print_grid(&grid);
        let mut vorinni = Vorinni {
            size,
            lookup,
            seeds: output_seeds,
            polygons,
        };
        info!("Starting generation process");

        Ok(vorinni)
    }
}
