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
use std::fmt::{Debug, Display};

use geo::contains::Contains;
use geo::prelude::BoundingRect;
use geo_types::{CoordNum, LineString, Point, Polygon};
use log::{debug, info};
use num_traits::PrimInt;
use quadtree_rs::{area::AreaBuilder, point::Point as QuadPoint, Quadtree};
use rand::{Rng, thread_rng};
use rand::prelude::IteratorRandom;

use crate::DataLoadingError;
use crate::parsing_error::ParseErrorType;
use crate::parsing_error::ParseErrorType::MathError;

const X_OFFSET: usize = 3500000;
const X_SCALE: usize = 50;
const Y_OFFSET: usize = 10000;
const Y_SCALE: usize = 25;


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
    let area = AreaBuilder::default().anchor(QuadPoint::from(anchor)).dimensions((width, height)).build()?;
    Ok(area)
}

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_point_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(point: &geo_types::Point<T>) -> Result<quadtree_rs::area::Area<T>, DataLoadingError> {
    let anchor = (point.x(), point.y());
    let area = AreaBuilder::default().anchor(QuadPoint::from(anchor)).build()?;
    Ok(area)
}

fn get_random_point_inside_polygon(polygon: &geo_types::Polygon<isize>) -> Option<geo_types::Point<isize>> {
    let mut start = Point::default();
    let mut rng = thread_rng();
    let mut try_count = 0;
    let bounds = polygon.bounding_rect()?;
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

fn voronoi_points_to_polygon(points: &mut Vec<voronoi::Point>) -> geo_types::Polygon<isize> {
    points.push(*points.first().expect("Polygon has too many points, Vec is out of space!"));
    // Convert to ints and build the exterior line
    let points = points.iter().map(|point| geo_types::Point::new(point.x.round() as isize, point.y.round() as isize)).collect::<Vec<geo_types::Point<isize>>>();
    geo_types::Polygon::new(LineString::from(points), Vec::new())
}

#[inline]
fn scale_point(point: (usize, usize)) -> (isize, isize) {
    (((point.0 - X_OFFSET) / X_SCALE) as isize, ((point.1 - Y_OFFSET) / Y_SCALE) as isize)
}

#[inline]
fn scale_geo_point(point: geo_types::Point<isize>) -> (isize, isize) {
    ((point.x() as usize - X_OFFSET / X_SCALE) as isize, ((point.y() as usize - Y_OFFSET) / Y_SCALE) as isize)
}

/// Utilises Jump Fill to build a Vorinni Diagram
pub struct Voronoi {
    pub size: usize,
    pub seeds: Vec<(isize, isize)>,
    pub polygons: PolygonContainer<(usize)>,

}

impl Voronoi {
    pub fn new(size: usize, seeds: Vec<(usize, usize)>) -> Result<Voronoi, DataLoadingError> {
        info!("Building Voronoi Grid of {} x {} with {} seeds",size,size,seeds.len());
        // TODO Increase seed size to all
        let seeds: Vec<(usize, usize)> = seeds.iter().choose_multiple(&mut thread_rng(), 500).iter().map(|p| **p).collect();
        let seeds: Vec<(isize, isize)> = seeds.iter().map(|p| scale_point(*p)).collect();

        let voronoi_seeds = seeds.iter().map(|p| voronoi::Point::new(p.0 as f64, p.1 as f64)).collect();

        // Build the Voronoi polygons
        let mut polygons = voronoi::make_polygons(&voronoi::voronoi(voronoi_seeds, size as f64));
        debug!("Built Voronoi map, with {} polygons",polygons.len());

        // Convert to a lazy iterator of geo polygons
        let polygons: Vec<(geo_types::Polygon<isize>, usize)> = polygons.iter_mut().enumerate().map(|(index, p)| (voronoi_points_to_polygon(p), index)).collect();
        debug!("Converted polygons to geo polygons");
        let container = PolygonContainer::new(polygons, size as f64)?;


        debug!("Built quad tree");
        let vorinni = Voronoi {
            size,
            seeds,
            polygons: container,
        };
        Ok(vorinni)
    }
    pub fn find_seed_for_point(&self, point: geo_types::Point<isize>) -> Result<(isize, isize), DataLoadingError> {
        let seed_index = self.polygons.find_polygon_for_point(point)?;
        Ok(*self.seeds.get(seed_index).ok_or_else(|| DataLoadingError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Cannot seed that contains polygon".to_string(), key: seed_index.to_string() } })?)
    }
}

pub struct PolygonContainer<T: Display + Debug + Copy + Clone + Eq + Ord> {
    pub lookup: Quadtree<isize, usize>,
    /// The polygon and it's ID
    pub polygons: Vec<(geo_types::Polygon<isize>, T)>,
}

impl<T: Display + Debug + Copy + Clone + Eq + Ord> PolygonContainer<T> {
    pub fn new(polygons: Vec<(geo_types::Polygon<isize>, T)>, grid_size: f64) -> Result<PolygonContainer<T>, DataLoadingError> {
        // Build Quadtree, with Coords of isize and values of seed points
        let mut lookup: Quadtree<isize, usize> = Quadtree::new(grid_size.log2().ceil() as usize);
        for (index, (polygon, id)) in polygons.iter().enumerate() {
            //let seed = *seeds.get(index).ok_or_else(|| DataLoadingError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Cannot retrieve seed for polygon".to_string(), key: index.to_string() } })?;
            lookup.insert(geo_polygon_to_quad_area(&polygon)?, index);
        }
        Ok(PolygonContainer { lookup, polygons })
    }

    pub fn find_polygon_for_point(&self, point: geo_types::Point<isize>) -> Result<T, DataLoadingError> {
        let point = scale_geo_point(point);
        let point = geo_types::Point::new(point.0, point.1);
        for entry in self.lookup.query(geo_point_to_quad_area(&point)?) {
            let index = entry.value_ref();
            let (poly, id) = self.polygons.get(*index).unwrap();
            if poly.contains(&point) {
                return Ok(*id);
            }
        }
        Err(DataLoadingError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Can't find nearest seed for point".to_string(), key: format!("{:?}", point) } })
    }
}

#[cfg(test)]
mod tests {
    use log::debug;
    use rand::{Rng, thread_rng};

    use crate::voronoi_generator::Voronoi;

    #[test]
    pub fn test() {
        let mut rng = thread_rng();
        let seeds: Vec<(usize, usize)> = (0..10).map(|_| (rng.gen_range(3600000..3700000), rng.gen_range(20000..30000))).collect();
        println!("Point: {:?}", seeds);
        let diagram = Voronoi::new(20000, seeds.clone());
        assert!(diagram.is_ok(), "Failed to build Voronoi: {:?}", diagram.err());
        let diagram = diagram.unwrap();
        println!("{:?}", diagram.polygons.polygons);
        for seed in seeds {
            let gen = diagram.find_seed_for_point(geo_types::Point::new(seed.0 as isize, seed.1 as isize));
            assert!(gen.is_ok(), "{:?}", gen);
            assert_eq!(gen.unwrap(), (seed.0 as isize, seed.1 as isize))
        }
    }
}