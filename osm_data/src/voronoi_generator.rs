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

use std::collections::HashMap;
use std::fmt::{Debug, Display};

use geo::contains::Contains;
use geo::prelude::BoundingRect;
use geo_types::{Coordinate, CoordNum, LineString, Point};
use log::{debug, info, trace, warn};
use rand::{Rng, thread_rng};
use voronoice::{ClipBehavior, VoronoiBuilder};

use crate::OSMError;
use crate::polygon_lookup::PolygonContainer;

const MAX_SIZE: i32 = 700000;

#[derive(Debug, Copy, Clone)]
pub struct Scaling {
    x_offset: isize,
    x_scale: isize,
    y_offset: isize,
    y_scale: isize,
}

impl Scaling {
    /// Factor of 16 reduction, with no offsets
    pub const fn yorkshire_national_grid(grid_size: i32) -> Scaling {
        let scale = (MAX_SIZE / grid_size) + 1;
        Scaling {
            x_offset: 0,
            x_scale: scale as isize,
            y_offset: 0,
            y_scale: scale as isize,
        }
    }
    /// Converts a coordinate to fit on the grid
    ///
    /// Used to represent a smaller grid, reducing RAM size
    #[inline]
    pub fn scale_point<T: CoordNum + Display>(&self, point: (T, T), grid_size: T) -> (T, T) {
        assert!(
            T::zero() <= point.0,
            "X scaling cannot be done, as it is negative: {}",
            point.0
        );
        assert!(
            T::zero() <= point.1,
            "Y scaling cannot be done, as it is negative: {}",
            point.1
        );
        let x = (point.0
            - T::from(self.x_offset).expect("Couldn't represent `x_offset` in generic type "))
            / T::from(self.x_scale).expect("Couldn't represent `x_scale` in generic type ");
        let y = (point.1
            - T::from(self.y_offset).expect("Couldn't represent `y_offset` in generic type "))
            / T::from(self.y_scale).expect("Couldn't represent `y_scale` in generic type ");
        assert!(T::zero() <= x, "X Coord {} is less than zero", x);
        assert!(
            x < grid_size,
            "X Coord {} is greater than the grid size {}",
            x,
            grid_size
        );
        assert!(T::zero() <= y, "Y Coord {} is less than zero", y);
        assert!(
            y < grid_size,
            "Y Coord {} is greater than the grid size {}",
            y,
            grid_size
        );
        (x, y)
    }

    pub fn scale_points<T: CoordNum + Display>(
        &self,
        points: &[Coordinate<T>],
        grid_size: T,
    ) -> Vec<Coordinate<T>> {
        points
            .iter()
            .map(|p| {
                assert!(T::zero() <= p.x, "X Coord ({}) is less than zero!", p.x);
                assert!(T::zero() <= p.y, "Y Coord ({}) is less than zero!", p.y);
                let scaled = self.scale_point((p.x, p.y), grid_size);
                scaled.into()
            })
            .collect()
    }
    #[inline]
    pub fn scale_polygon<T: CoordNum + Display>(
        &self,
        polygon: &geo_types::Polygon<T>,
        grid_size: T,
    ) -> geo_types::Polygon<T> {
        geo_types::Polygon::new(
            self.scale_points(&polygon.exterior().0, grid_size).into(),
            polygon
                .interiors()
                .iter()
                .map(|interior| self.scale_points(&interior.0, grid_size).into())
                .collect(),
        )
    }
    pub fn scale_rect<T: CoordNum + Display>(
        &self,
        rect: geo_types::Rect<T>,
        grid_size: T,
    ) -> geo_types::Rect<T> {
        geo_types::Rect::new(
            self.scale_point(rect.min().x_y(), grid_size),
            self.scale_point(rect.max().x_y(), grid_size),
        )
    }
}

impl Default for Scaling {
    fn default() -> Self {
        Scaling {
            x_offset: 0,
            x_scale: 1,
            y_offset: 0,
            y_scale: 1,
        }
    }
}

fn get_random_point_inside_polygon(
    polygon: &geo_types::Polygon<isize>,
) -> Option<geo_types::Point<isize>> {
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

fn voronoi_cell_to_polygon<T: CoordNum>(cell: &voronoice::VoronoiCell) -> Option<geo_types::Polygon<T>> {
    // Return None, if no vertices exist
    cell.iter_vertices().next()?;
    //points.push(points.first().expect("Polygon has too many points, Vec is out of space!"));
    // Convert to ints and build the exterior line
    let points = cell
        .iter_vertices()
        .map(|point| {
            assert!(point.x >= 0.0, "Voronoice produced negative X: {}!", point.x);
            assert!(point.y >= 0.0, "Voronoice produced negative Y: {}!", point.x);
            geo_types::Point::new(
                T::from(point.x.round()).expect("Failed to represent f64 x coordinate as T"),
                T::from(point.y.round()).expect("Failed to represent f64 y coordinate as T"),
            )
        })
        .collect::<Vec<geo_types::Point<T>>>();
    Some(geo_types::Polygon::new(LineString::from(points), Vec::new()))
}

/// Returns the minimum and maximum grid size required for the seeds
pub fn find_seed_bounds<T: num_traits::PrimInt + Copy + Clone + Debug>(seeds: &[(T, T)]) -> ((T, T), (T, T)) {
    let mut min_x = T::max_value();
    let mut max_x = T::zero();
    let mut min_y = T::max_value();
    let mut max_y = T::zero();
    for seed in seeds {
        assert!(seed.0 > T::zero(), "X part of Seed {:?} is less than zero!", seed);
        assert!(seed.0 < T::max_value(), "X part of Seed {:?} is greater than the max value!", seed);
        assert!(seed.1 > T::zero(), "Y part of Seed {:?} is less than zero!", seed);
        assert!(seed.1 < T::max_value(), "Y part of Seed {:?} is greater than the max value!", seed);
        if seed.0 < min_x {
            min_x = seed.0;
        }
        if max_x < seed.0 {
            max_x = seed.0;
        }

        if seed.1 < min_y {
            min_y = seed.1;
        }
        if max_y < seed.1 {
            max_y = seed.1
        }
    }
    ((min_x, min_y), (max_x, max_y))
}

pub struct Voronoi {
    pub grid_size: i32,
    pub seeds: Vec<(i32, i32)>,
    // The polygon id?
    pub polygons: PolygonContainer<usize>,

    pub scaling: Scaling,
}

impl Voronoi {
    /// Create a new Voronoi diagram to find the closest seed to a point
    ///
    /// Size represents the grid size to represent
    pub fn new(
        size: i32,
        seeds: Vec<(i32, i32)>,
        scaling: Scaling,
    ) -> Result<Voronoi, OSMError> {
        info!(
            "Building Voronoi Grid of {} x {} with {} seeds",
            size,
            size,
            seeds.len()
        );
        println!("Boundary: {:?}", find_seed_bounds(&seeds));
        //let seeds: Vec<(usize, usize)> = seeds.iter().choose_multiple(&mut thread_rng(), 500).iter().map(|p| **p).collect();
        let voronoi_seeds: Vec<voronoice::Point> = seeds
            .iter()
            .map(|p| {
                let (x, y) = scaling.scale_point((p.0, p.1), size);
                voronoice::Point {
                    x: x as f64,
                    y: y as f64,
                }
            })
            .collect();

        // TODO Can remove this later, when optimum size found
        //debug!("Seeds: {:?}",voronoi_seeds);
        let boundary = find_seed_bounds(
            &voronoi_seeds
                .iter()
                .map(|p| ((p.x.round() as i32), (p.y.round() as i32)))
                .collect::<Vec<(i32, i32)>>(),
        );
        trace!("Voronoi Boundary: {:?}", boundary);
        // The size must be even, otherwise we get a negative bounding box
        let mut grid_size = boundary.1.0.max(boundary.1.1);
        if grid_size % 2 != 0 {
            grid_size += 1;
        }
        // Build the Voronoi polygons
        let bounding_box = voronoice::BoundingBox::new(
            voronoice::Point {
                x: ((grid_size / 2) as f64).floor(),
                y: ((grid_size / 2) as f64).floor(),
            },
            (grid_size) as f64,
            (grid_size) as f64,
        );
        debug!("Voronoi boundary box size: {} -> {:?}", grid_size, bounding_box);
        trace!("Sample of Seeds: {:?}",voronoi_seeds.iter().take(10).collect::<Vec<&voronoice::Point>>());
        let polygons = VoronoiBuilder::default()
            .set_sites(voronoi_seeds)
            .set_bounding_box(bounding_box)
            .set_clip_behavior(ClipBehavior::Clip)
            .set_lloyd_relaxation_iterations(0)
            .build();
        let polygons = if let Some(polygons) = polygons {
            debug!(
                "Built Voronoi map, with {} polygons",
                polygons.cells().len()
            );
            // Convert to a lazy iterator of geo polygons
            let polygons: HashMap<usize, geo_types::Polygon<i32>> = polygons
                .iter_cells()
                .enumerate()
                .filter_map(|(index, p)| Some((index, voronoi_cell_to_polygon(&p)?)))
                .collect();
            trace!("Converted polygons to {} geo polygons",polygons.len());
            polygons
        } else {
            return Err(OSMError::Misc {
                source: "Failed to build Voronoi diagram!".to_string(),
            });
        };

        let container = PolygonContainer::new(polygons, Scaling::yorkshire_national_grid(grid_size), grid_size)?;
        debug!("Built quad tree");

        Ok(Voronoi {
            grid_size,
            seeds,
            polygons: container,
            scaling,
        })
    }
    /// Attempts to find the index of the SINGULAR closest seed to the given point
    pub fn find_seed_for_point(
        &self,
        point: geo_types::Point<i32>,
    ) -> Result<usize, OSMError> {
        let point = self.scaling.scale_point(point.x_y(), self.grid_size);
        let point = geo_types::Point::new(point.0, point.1);
        Ok(*self.polygons.find_polygon_for_point(&point)?)
    }
    /// Attempts to find the index of the closest seed to the given point
    pub fn find_seeds_for_point(
        &self,
        point: geo_types::Point<i32>,
    ) -> Result<Vec<usize>, OSMError> {
        let point = self.scaling.scale_point(point.x_y(), self.grid_size);
        let point = geo_types::Point::new(point.0, point.1);
        let seed_indexes = self.polygons.find_polygons_for_point(&point)?.into_iter().copied().collect();
        Ok(seed_indexes)
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, thread_rng};

    use crate::OSMError;
    use crate::voronoi_generator::{PolygonContainer, Scaling, Voronoi};

    #[test]
    fn voronoi_seed_generation_and_retrieval() {
        let mut rng = thread_rng();
        let grid_size: i32 = 10000;
        let seeds: Vec<(i32, i32)> = (0..100)
            .map(|_| (rng.gen_range(0..grid_size), rng.gen_range(0..grid_size)))
            .collect();
        let diagram = Voronoi::new(grid_size, seeds.clone(), Scaling::default());
        assert!(
            diagram.is_ok(),
            "Failed to build Voronoi: {:?}",
            diagram.err()
        );
        let diagram = diagram.unwrap();
        for seed in seeds {
            let result = diagram.find_seeds_for_point(seed.into());
            assert!(result.is_ok(), "{:?}", result);
            let result = result.unwrap();
            //assert_eq!(result.first().unwrap(), (seed.0, seed.1))
        }
    }

    fn line_string_to_polygon_container(
        points: geo_types::LineString<i32>,
    ) -> Result<PolygonContainer<i32>, OSMError> {
        let polygon = geo_types::Polygon::new(points, vec![]);
        PolygonContainer::new(
            [(0, polygon)].iter().cloned().collect(),
            Scaling::default(),
            100,
        )
    }

    #[test]
    fn quadtree_boundary() {
        let size = 100;
        assert!(
            line_string_to_polygon_container(
                (vec![(0, 0), (0, size), (size, size), (size, 0), (0, 0)]).into()
            )
                .is_ok(),
            "Max Boundaries fail"
        );
        assert!(
            line_string_to_polygon_container(
                (vec![(0, 0), (0, size), (size + 1, size), (size + 1, 0), (0, 0)]).into()
            )
                .is_err(),
            "Exceeding max X isn't detected"
        );
        assert!(
            line_string_to_polygon_container(
                (vec![(0, 0), (0, size + 1), (size, size + 1), (size, 0), (0, 0)]).into()
            )
                .is_err(),
            "Exceeding max Y isn't detected"
        );
        assert!(
            line_string_to_polygon_container(
                (vec![(0, -1), (0, size), (size, size), (size, 0), (0, -1)]).into()
            )
                .is_err(),
            "Negative X isn't detected"
        );
        assert!(
            line_string_to_polygon_container(
                (vec![(0, -1), (0, size), (size, size), (size, 0), (0, -1)]).into()
            )
                .is_err(),
            "Negative Y isn't detected"
        );
    }

    #[test]
    fn scaling_none() {
        let mut rng = thread_rng();
        let point = (rng.gen_range(0..1000), rng.gen_range(0..1000));
        let scaling = Scaling::default();
        let scaled_point = scaling.scale_point(point, 1000);
        assert_eq!(point, scaled_point)
    }
}
