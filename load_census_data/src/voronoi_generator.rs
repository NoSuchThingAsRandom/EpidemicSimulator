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

use geo::contains::Contains;
use geo::prelude::BoundingRect;
use geo_types::{LineString, Point};
use log::{debug, info, trace};
use rand::{Rng, thread_rng};
use voronoice::{ClipBehavior, VoronoiBuilder};

use crate::DataLoadingError;
use crate::osm_parsing::GRID_SIZE;
use crate::parsing_error::ParseErrorType;
use crate::polygon_lookup::PolygonContainer;

#[derive(Debug, Copy, Clone)]
pub struct Scaling {
    x_offset: isize,
    x_scale: isize,
    y_offset: isize,
    y_scale: isize,
}

impl Scaling {
    pub const fn output_areas() -> Scaling {
        Scaling {
            x_offset: 0,
            x_scale: 16,
            y_offset: 0,
            y_scale: 16,
        }
    }
    pub const fn yorkshire_national_grid() -> Scaling {
        Scaling {
            x_offset: 0000,
            x_scale: 1,
            y_offset: 0,
            y_scale: 1,
        }
    }
    /// Converts a coordinate to fit on the grid
    ///
    /// Used to represent a smaller grid, reducing RAM size
    #[inline]
    pub fn scale_point(&self, point: (isize, isize), grid_size: isize) -> (isize, isize) {
        assert!(
            0 <= point.0 as isize,
            "X scaling cannot be done, as it is negative: {}",
            point.0
        );
        assert!(
            0 <= point.1 as isize,
            "X scaling cannot be done, as it is negative: {}",
            point.1
        );
        let x = (point.0 as isize - self.x_offset) / self.x_scale;
        let y = (point.1 as isize - self.y_offset) / self.y_scale;
        assert!(0 <= x, "X Coord {} is less than zero", x);
        assert!(x < grid_size, "X Coord {} is greater than the grid size {}", x, grid_size);
        assert!(0 <= y, "Y Coord {} is less than zero", y);
        assert!(y < grid_size, "Y Coord {} is greater than the grid size {}", y, grid_size);
        (x, y)
    }
    pub fn scale_polygon(
        &self,
        polygon: &geo_types::Polygon<isize>,
        grid_size: isize,
    ) -> geo_types::Polygon<isize> {
        geo_types::Polygon::new(
            polygon
                .exterior()
                .0
                .iter()
                .map(|p| {
                    assert!(0 <= p.x, "X Coord ({}) is less than zero!", p.x);
                    assert!(0 <= p.y, "Y Coord ({}) is less than zero!", p.y);
                    let x = self.scale_point((p.x, p.y), grid_size);
                    let p: geo_types::Coordinate<isize> = x.into();
                    return p;
                })
                .collect(),
            vec![],
        )
    }
    pub fn scale_rect(
        &self,
        rect: geo_types::Rect<isize>,
        grid_size: isize,
    ) -> geo_types::Rect<isize> {
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

fn voronoi_cell_to_polygon(cell: &voronoice::VoronoiCell) -> geo_types::Polygon<isize> {
    //points.push(points.first().expect("Polygon has too many points, Vec is out of space!"));
    // Convert to ints and build the exterior line
    let points = cell
        .iter_vertices()
        .map(|point| geo_types::Point::new(point.x.round() as isize, point.y.round() as isize))
        .collect::<Vec<geo_types::Point<isize>>>();
    geo_types::Polygon::new(LineString::from(points), Vec::new())
}

/// Returns the minimum and maximum grid size required for the seeds
pub fn find_seed_bounds<T: num_traits::PrimInt + Copy>(seeds: &[(T, T)]) -> ((T, T), (T, T)) {
    let mut min_x = T::max_value();
    let mut max_x = T::zero();
    let mut min_y = T::max_value();
    let mut max_y = T::zero();
    for seed in seeds {
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

#[derive(Debug)]
pub struct Voronoi {
    pub grid_size: usize,
    pub seeds: Vec<(usize, usize)>,
    pub polygons: PolygonContainer<usize>,

    pub scaling: Scaling,
}

impl Voronoi {
    /// Create a new Voronoi diagram to find the closest seed to a point
    ///
    /// Size represents the grid size to represent
    pub fn new(
        size: usize,
        seeds: Vec<(usize, usize)>,
        scaling: Scaling,
    ) -> Result<Voronoi, DataLoadingError> {
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
                let (x, y) = scaling.scale_point((p.0 as isize, p.1 as isize), size as isize);
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
                .map(|p| (p.x as isize, p.y as isize))
                .collect::<Vec<(isize, isize)>>(),
        );
        trace!("Voronoi Boundary: {:?}", boundary);
        // The size must be even, otherwise we get a negative bounding box
        let mut size = boundary.1.0.max(boundary.1.1) as usize;
        if size % 2 != 0 {
            size += 1;
        }
        // Build the Voronoi polygons
        let bounding_box = voronoice::BoundingBox::new(
            voronoice::Point {
                x: ((size / 2) as f64).floor(),
                y: ((size / 2) as f64).floor(),
            },
            size as f64,
            size as f64,
        );
        debug!("Voronoi boundary box size: {} -> {:?}", size, bounding_box);
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
            let polygons: HashMap<usize, geo_types::Polygon<isize>> = polygons
                .iter_cells()
                .enumerate()
                .map(|(index, p)| (index, voronoi_cell_to_polygon(&p)))
                .collect();
            trace!("Converted polygons to geo polygons");
            if polygons.len() < 100 {
                println!("{:?}", polygons);
            }
            polygons
        } else {
            return Err(DataLoadingError::Misc {
                source: "Failed to build Voronoi diagram!".to_string(),
            });
        };

        let container = PolygonContainer::new(polygons, Scaling::output_areas(), GRID_SIZE as f64)?;
        debug!("Built quad tree");

        Ok(Voronoi {
            grid_size: size,
            seeds,
            polygons: container,
            scaling,
        })
    }
    /// Attempts to find the closest seed to the given point
    pub fn find_seed_for_point(
        &self,
        point: geo_types::Point<isize>,
    ) -> Result<(usize, usize), DataLoadingError> {
        let point = self
            .scaling
            .scale_point(point.x_y(), self.grid_size as isize);
        let point = geo_types::Point::new(point.0, point.1);
        let seed_index = self.polygons.find_polygon_for_point(&point)?;
        Ok(*self
            .seeds
            .get(*seed_index)
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: "Cannot seed that contains polygon".to_string(),
                    key: seed_index.to_string(),
                },
            })?)
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, thread_rng};

    use crate::DataLoadingError;
    use crate::voronoi_generator::{PolygonContainer, Scaling, Voronoi};

    #[test]
    fn voronoi_seed_generation_and_retrieval() {
        let mut rng = thread_rng();
        let grid_size: usize = 10000;
        let seeds: Vec<(usize, usize)> = (0..100)
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
            let result = diagram
                .find_seed_for_point(geo_types::Point::new(seed.0 as isize, seed.1 as isize));
            assert!(result.is_ok(), "{:?}", result);
            assert_eq!(result.unwrap(), (seed.0, seed.1))
        }
    }

    fn line_string_to_polygon_container(
        points: geo_types::LineString<isize>,
    ) -> Result<PolygonContainer<i32>, DataLoadingError> {
        let polygon = geo_types::Polygon::new(points, vec![]);
        PolygonContainer::new(
            [(0, polygon)].iter().cloned().collect(),
            Scaling::default(),
            100.0,
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
