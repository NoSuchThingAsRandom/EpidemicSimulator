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

//! This is a wrapper around a QuadTree to find the polygon that contains a given point, from a list of polygons
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
use std::hash::Hash;
use std::time::Instant;

use geo::prelude::BoundingRect;
use geo_types::{CoordNum, LineString};
use log::{debug, info, trace};
use num_traits::PrimInt;
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use shapefile::dbase::FieldValue;
use shapefile::Shape;

use crate::convert::decimal_latitude_and_longitude_to_northing_and_eastings;
use crate::OSMError;
use crate::quadtree::QuadTree;
use crate::voronoi_generator::Scaling;

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_polygon_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(
    polygon: &geo_types::Polygon<T>,
) -> Result<geo_types::Rect<T>, OSMError> {
    polygon
        .bounding_rect()
        .ok_or_else(|| OSMError::ValueParsingError {
            source: "Failed to generate bounding box for polygon".to_string(),
        })
}

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_point_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(
    point: &geo_types::Point<T>,
) -> Result<geo_types::Rect<T>, OSMError> {
    Ok(point.bounding_rect())
}

/// A lookup table for finding the closest `seed` to a given point
///
/// T Represents the Data Type used as the index of the `seed`
pub struct PolygonContainer<T: Debug + Clone + Eq + Ord + Hash> {
    pub lookup: QuadTree<T, i32>,
    /// The polygon and it's ID
    ///
    /// Has to be i32, for the geo::Intersects function
    pub polygons: HashMap<T, geo_types::Polygon<i32>>,
    pub scaling: Scaling,
    pub grid_size: i32,
}

impl<T: Debug + Clone + Eq + Ord + Hash> PolygonContainer<T> {
    /// Note that if grid_size is too big, then stack overflows occur
    pub fn new(
        polygons: HashMap<T, geo_types::Polygon<i32>>,
        scaling: Scaling,
        grid_size: i32,
    ) -> Result<PolygonContainer<T>, OSMError> {
        trace!("Building new Polygon Container of size: {}", grid_size);
        // Build Quadtree, with Coords of isize and values of seed points
        let mut lookup = QuadTree::with_size(grid_size, grid_size, 10, 50);
        let mut added = 0;
        for (id, polygon) in &polygons {
            let bounds = match polygon
                .bounding_rect()
                .ok_or_else(|| OSMError::ValueParsingError {
                    source: "Failed to generate bounding box for polygon".to_string(),
                }) {
                Ok(p) => p,
                Err(_e) => {
                    //error!("{}",e);
                    continue;
                }
            };

            let mut bounds = scaling.scale_rect(bounds, grid_size);
            if bounds.width() == 0 {
                bounds.set_max((bounds.max().x + 1, bounds.max().y));
            }
            if bounds.height() == 0 {
                bounds.set_max((bounds.max().x, bounds.max().y + 1));
            }
            if (grid_size) < bounds.max().x {
                return Err(OSMError::OutOfBounds {
                    context: "Max X Coordinate is outside bounding rect".to_string(),
                    max_size: grid_size.to_string(),
                    actual_size: bounds.max().x.to_string(),
                });
            }
            if (grid_size) < bounds.max().y {
                return Err(OSMError::OutOfBounds {
                    context: "Max Y Coordinate is outside bounding rect".to_string(),
                    max_size: grid_size.to_string(),
                    actual_size: bounds.max().y.to_string(),
                });
            }
            if bounds.min().x < 0 {
                return Err(OSMError::OutOfBounds {
                    context: "Min X Coordinate is outside bounding rect".to_string(),
                    max_size: "0".to_string(),
                    actual_size: bounds.min().x.to_string(),
                });
            }
            if bounds.min().y < 0 {
                return Err(OSMError::OutOfBounds {
                    context: "Min Y Coordinate is outside bounding rect".to_string(),
                    max_size: "0".to_string(),
                    actual_size: bounds.min().y.to_string(),
                });
            }
            if lookup.add_item(id.clone(), bounds) {
                added += 1;
            } else {
                panic!(
                    "Failed to add Polygon with boundary: {:?}. But succeeded with: {}",
                    bounds, added
                );
            }
        }
        Ok(PolygonContainer {
            lookup,
            polygons,
            grid_size,
            scaling,
        })
    }

    /// Finds the index of all polygons that containing the given polygon
    ///
    /// Note the point needs to be scaled
    pub fn find_polygons_containing_polygon<'a>(
        &'a self,
        polygon: &'a geo_types::Polygon<i32>,
    ) -> Result<Box<dyn Iterator<Item=&T> + 'a>, OSMError> {
        // TODO Move this scaling

        let scaled_polygon: geo_types::Polygon<i32> =
            self.scaling.scale_polygon(polygon, self.grid_size);
        let boundary = geo_polygon_to_quad_area(&scaled_polygon)?;
        let results = self.lookup.get_items(boundary).into_iter();
        Ok(Box::new(results))
    }
    /// Finds index of the SINGULAR polygon that contains the given point
    ///
    /// Note the point needs to be scaled
    pub fn find_polygon_for_point<'a>(
        &'a self,
        point: &'a geo_types::Point<i32>,
    ) -> Result<&'a T, OSMError> {
        // TODO Move this scaling
        let scaled_point: geo_types::Point<i32> = self
            .scaling
            .scale_point((point.x(), point.y()), self.grid_size)
            .into();
        assert!(
            scaled_point.x() < self.grid_size,
            "X Coordinate is out of range!"
        );
        assert!(
            scaled_point.y() < self.grid_size,
            "Y Coordinate is out of range!"
        );
        let boundary = geo_point_to_quad_area(&scaled_point)?;
        let results = self.lookup.get_items(boundary);
        Ok(results.first().unwrap())
    }
    /// Finds index of the polygon that contains the given point
    ///
    /// Note the point needs to be scaled
    pub fn find_polygons_for_point<'a>(
        &'a self,
        point: &'a geo_types::Point<i32>,
    ) -> Result<Vec<&T>, OSMError> {
        // TODO Move this scaling
        let scaled_point: geo_types::Point<i32> = self
            .scaling
            .scale_point((point.x(), point.y()), self.grid_size)
            .into();
        assert!(
            scaled_point.x() < self.grid_size,
            "X Coordinate is out of range!"
        );
        assert!(
            scaled_point.y() < self.grid_size,
            "Y Coordinate is out of range!"
        );
        let boundary = geo_point_to_quad_area(&scaled_point)?;
        let results = self
            .lookup
            .get_multiple_items(boundary)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        /*        let results = results
        .filter_map(move |(id, _distance)| {
            let poly =
                self.polygons
                    .get(id);
            if let Some(poly) = poly {
                if poly.intersects(poly) {
                    return Some(id);
                }
            }
            None
        });*/
        Ok(results)
    }
}

//impl<T: Debug + Clone + Eq + Ord + Hash> PolygonContainer<T> {
impl PolygonContainer<String> {
    /// Generates the polygons for each output area contained in the given file
    pub fn load_polygons_from_file(
        filename: &str,
        grid_size: i32,
    ) -> Result<PolygonContainer<String>, OSMError> {
        let mut reader = shapefile::Reader::from_path(filename).map_err(|e| OSMError::IOError {
            source: Box::new(e),
            context: format!("Shape File '{}' doesn't exist!", filename),
        })?;
        let start_time = Instant::now();
        //let mut data = HashMap::new();
        info!("Loading map data from file {}", filename);
        let data = reader.read()?.par_iter().enumerate().map(|(index, (shape, record))| {
            let polygon = if let Shape::Polygon(polygon) = shape {
                if index % 50000 == 0 {
                    debug!(
                    "Built {} polygons in time: {}",
                    index * 10000,
                    start_time.elapsed().as_secs_f64()
                );
                }
                assert!(!polygon.rings().is_empty());
                let rings: Vec<geo_types::Coordinate<i32>>;
                let mut interior_ring;
                if polygon.rings().len() == 1 {
                    rings = polygon.rings()[0]
                        .points()
                        .iter()
                        .map(|p| {
                            // TODO Reenable this if using old system
                            /*
                            geo_types::Coordinate::from((
                                p.x.round() as isize,
                                p.y.round() as isize,
                            ))*/
                            geo_types::Coordinate::from(decimal_latitude_and_longitude_to_northing_and_eastings(
                                p.y,
                                p.x,
                            ))
                        })
                        .collect();
                    interior_ring = Vec::new();
                } else {
                    interior_ring = polygon
                        .rings()
                        .iter()
                        .map(|r| {
                            LineString::from(
                                r.points()
                                    .iter()
                                    .map(|p| {
                                        geo_types::Coordinate::from(decimal_latitude_and_longitude_to_northing_and_eastings(
                                            p.y,
                                            p.x,
                                        ))
                                        // TODO Reenable this if using old system
                                        /*
                                        geo_types::Coordinate::from((
                                            p.x.round() as isize,
                                            p.y.round() as isize,
                                        ))*/
                                    })
                                    .collect::<Vec<geo_types::Coordinate<i32>>>(),
                            )
                        })
                        .collect();
                    rings = interior_ring
                        .pop()
                        .ok_or_else(|| OSMError::IsEmpty {
                            context: "Expected an interior ring to exist!".to_string(),
                        })?
                        .0;
                }
                geo_types::Polygon::new(LineString::from(rings), interior_ring)
            } else {
                return Err(OSMError::ValueParsingError {
                    source: format!("Unexpected shape type: {}", shape.shapetype().to_string())
                });
            };

            // Retrieve the area code:
            let code_record =
                record
                    .get("code")
                    .ok_or_else(|| OSMError::ValueParsingError {
                        source: "Output Area is missing it's code".to_string()
                    })?;
            let code: String;
            match code_record {
                FieldValue::Character(value) => {
                    if let Some(value) = value {
                        code = value.to_string()
                    } else {
                        return Err(OSMError::IsEmpty {
                            context: "The code for an Output Area is empty".to_string()
                        });
                    }
                }
                _ => {
                    return Err(OSMError::ValueParsingError {
                        source: "Expected type 'character' for area code".to_string()
                    });
                }
            }

            Ok((code, polygon))
        }).collect::<Result<HashMap<String, geo_types::Polygon<i32>>, OSMError>>()?;
        info!("Finished loading map data in {:?}", start_time.elapsed());
        let scaling = Scaling::yorkshire_national_grid(grid_size);
        PolygonContainer::new(data, scaling, grid_size)
    }
    /*    pub fn remove_polygon(&mut self, output_area_id: T) {
        let poly=self.polygons.remove(&output_area_id).unwrap();
        //let poly = Error::from_option(self.polygons.remove(output_area_id), output_area_id, "Cannot delete polygon!".to_string())?;
        self.lookup.delete(geo_polygon_to_quad_area(&poly).unwrap())
    }*/
}
