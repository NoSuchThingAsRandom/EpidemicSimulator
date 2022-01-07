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

use geo::prelude::{BoundingRect, Contains};
use geo_types::{CoordNum, LineString};
use log::{debug, info, warn};
use num_traits::PrimInt;
use quadtree_rs::{area::AreaBuilder, point::Point as QuadPoint, Quadtree};
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use shapefile::dbase::FieldValue;
use shapefile::Shape;

use crate::DataLoadingError;
use crate::osm_parsing::convert::decimal_latitude_and_longitude_to_northing_and_eastings;
use crate::osm_parsing::GRID_SIZE;
use crate::parsing_error::ParseErrorType;
use crate::parsing_error::ParseErrorType::{MathError, MissingKey};
use crate::voronoi_generator::Scaling;

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_polygon_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(
    polygon: &geo_types::Polygon<T>,
) -> Result<quadtree_rs::area::Area<T>, DataLoadingError> {
    let bounds = polygon
        .bounding_rect()
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: MathError {
                context: "Failed to generate bounding box for polygon".to_string(),
            },
        })?;
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
    assert!(
        bounds.height() >= T::zero(),
        "Rect has a height less than zero {:?}",
        bounds
    );
    assert!(
        bounds.width() >= T::zero(),
        "Rect has a width less than zero {:?}",
        bounds
    );
    let area = AreaBuilder::default()
        .anchor(QuadPoint::from(anchor))
        .dimensions((width, height))
        .build()?;
    Ok(area)
}

/// Converts a geo type Polygon to a quadtree Area (using the Polygon Bounding Box)
#[inline]
fn geo_point_to_quad_area<T: CoordNum + PrimInt + Display + PartialOrd + Default>(
    point: &geo_types::Point<T>,
) -> Result<quadtree_rs::area::Area<T>, DataLoadingError> {
    let anchor = (point.x(), point.y());
    let area = AreaBuilder::default()
        .anchor(QuadPoint::from(anchor))
        .build()?;
    Ok(area)
}

#[derive(Debug)]
pub struct PolygonContainer<T: Debug + Clone + Eq + Ord + Hash> {
    pub lookup: Quadtree<isize, T>,
    /// The polygon and it's ID
    pub polygons: HashMap<T, geo_types::Polygon<isize>>,
    pub scaling: Scaling,
    pub grid_size: f64,
}

impl<T: Debug + Clone + Eq + Ord + Hash> PolygonContainer<T> {
    ///
    pub fn new(
        polygons: HashMap<T, geo_types::Polygon<isize>>,
        scaling: Scaling,
        grid_size: f64,
    ) -> Result<PolygonContainer<T>, DataLoadingError> {
        // Build Quadtree, with Coords of isize and values of seed points
        let mut lookup: Quadtree<isize, T> = Quadtree::new((grid_size).log2().ceil() as usize);
        for (id, polygon) in &polygons {
            let bounds =
                polygon
                    .bounding_rect()
                    .ok_or_else(|| DataLoadingError::ValueParsingError {
                        source: MathError {
                            context: "Failed to generate bounding box for polygon".to_string(),
                        },
                    })?;
            let mut bounds = scaling.scale_rect(bounds, grid_size as isize);
            if bounds.width() == 0 {
                bounds.set_max((bounds.max().x + 1, bounds.max().y));
            }
            if bounds.height() == 0 {
                bounds.set_max((bounds.max().x, bounds.max().y + 1));
            }
            if (grid_size as isize) < bounds.max().x {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::OutOfBounds {
                        context: "Max X Coordinate is outside bounding rect".to_string(),
                        max_size: grid_size.to_string(),
                        actual_size: bounds.max().x.to_string(),
                    },
                });
            }
            if (grid_size as isize) < bounds.max().y {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::OutOfBounds {
                        context: "Max Y Coordinate is outside bounding rect".to_string(),
                        max_size: grid_size.to_string(),
                        actual_size: bounds.max().y.to_string(),
                    },
                });
            }
            if bounds.min().x < 0 {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::OutOfBounds {
                        context: "Min X Coordinate is outside bounding rect".to_string(),
                        max_size: "0".to_string(),
                        actual_size: bounds.min().x.to_string(),
                    },
                });
            }
            if bounds.min().y < 0 {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::OutOfBounds {
                        context: "Min Y Coordinate is outside bounding rect".to_string(),
                        max_size: "0".to_string(),
                        actual_size: bounds.min().y.to_string(),
                    },
                });
            }
            let region = AreaBuilder::default()
                .anchor(QuadPoint::from(bounds.min().x_y()))
                .dimensions((bounds.width(), bounds.height()))
                .build();
            //let seed = *seeds.get(index).ok_or_else(|| DataLoadingError::ValueParsingError { source: ParseErrorType::MissingKey { context: "Cannot retrieve seed for polygon".to_string(), key: index.to_string() } })?;
            match region {
                Ok(region) => {
                    lookup
                        .insert(region, id.clone())
                        .unwrap_or_else(|| panic!("Polygon insertion failed!: {:?}", polygon));
                }
                Err(e) => {
                    warn!(
                        "Failed to build region for polygon with boundary ({:?})! {:?}",
                        bounds, e
                    );
                }
            }
        }
        Ok(PolygonContainer {
            lookup,
            polygons,
            grid_size,
            scaling,
        })
    }

    /// Finds the polygon that contains the given point
    ///
    /// Note the point needs to be scaled
    pub fn find_polygon_for_point(
        &self,
        point: &geo_types::Point<isize>,
    ) -> Result<&T, DataLoadingError> {
        // TODO Move this scaling
        let scaled_point: geo_types::Point<isize> = self
            .scaling
            .scale_point((point.x(), point.y()), self.grid_size as isize)
            .into();
        assert!(
            scaled_point.x() < self.grid_size as isize,
            "X Coordinate is out of range!"
        );
        assert!(
            scaled_point.y() < self.grid_size as isize,
            "Y Coordinate is out of range!"
        );
        let res = self.lookup.query(geo_point_to_quad_area(&scaled_point)?);
        for entry in res {
            let id = entry.value_ref();
            let poly =
                self.polygons
                    .get(id)
                    .ok_or_else(|| DataLoadingError::ValueParsingError {
                        source: ParseErrorType::MissingKey {
                            context: "Can't find polygon with id".to_string(),
                            key: format!("{:?}", point),
                        },
                    })?;
            if poly.contains(point) {
                return Ok(id);
            }
        }
        Err(DataLoadingError::ValueParsingError {
            source: ParseErrorType::MissingKey {
                context: "Can't find nearest seed for point".to_string(),
                key: format!("{:?}", point),
            },
        })
    }
}

//impl<T: Debug + Clone + Eq + Ord + Hash> PolygonContainer<T> {
impl PolygonContainer<String> {
    /// Generates the polygons for each output area contained in the given file
    pub fn load_polygons_from_file(
        filename: &str,
    ) -> Result<PolygonContainer<String>, DataLoadingError> {
        let mut reader =
            shapefile::Reader::from_path(filename).map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: format!("Shape File '{}' doesn't exist!", filename),
            })?;
        let mut start_time = Instant::now();
        //let mut data = HashMap::new();
        info!("Loading map data from file {}",filename);
        let mut data = reader.read()?.par_iter().enumerate().map(|(index, (shape, record))| {
            let polygon = if let Shape::Polygon(polygon) = shape {
                if index % 50000 == 0 {
                    debug!(
                    "Built {} polygons in time: {}",
                    index * 10000,
                    start_time.elapsed().as_secs_f64()
                );
                }
                assert!(!polygon.rings().is_empty());
                let rings: Vec<geo_types::Coordinate<isize>>;
                let mut interior_ring;
                if polygon.rings().len() == 1 {
                    rings = polygon.rings()[0]
                        .points()
                        .iter()
                        .map(|p| {
                            // TODO Reenable this if using old system
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
                                    .collect::<Vec<geo_types::Coordinate<isize>>>(),
                            )
                        })
                        .collect();
                    rings = interior_ring
                        .pop()
                        .ok_or_else(|| DataLoadingError::ValueParsingError {
                            source: ParseErrorType::IsEmpty {
                                message: "Expected an interior ring to exist!".to_string(),
                            },
                        })?
                        .0;
                }
                geo_types::Polygon::new(LineString::from(rings), interior_ring)
            } else {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::InvalidDataType {
                        value: Some(shape.shapetype().to_string()),
                        expected_type: "Unexpected shape type!".to_string(),
                    },
                });
            };

            // Retrieve the area code:
            let code_record =
                record
                    .get("code")
                    .ok_or_else(|| DataLoadingError::ValueParsingError {
                        source: MissingKey {
                            context: "Output Area is missing it's code".to_string(),
                            key: "code".to_string(),
                        },
                    })?;
            let code: String;
            match code_record {
                FieldValue::Character(value) => {
                    if let Some(value) = value {
                        code = value.to_string()
                    } else {
                        return Err(DataLoadingError::ValueParsingError {
                            source: ParseErrorType::IsEmpty {
                                message: "The code for an Output Area is empty".to_string(),
                            }
                        });
                    }
                }
                _ => {
                    return Err(DataLoadingError::ValueParsingError {
                        source: ParseErrorType::InvalidDataType {
                            value: Some(code_record.field_type().to_string()),
                            expected_type: "Expected type 'character' for area code".to_string(),
                        },
                    });
                }
            }

            Ok((code, polygon))
        }).collect::<Result<HashMap<String, geo_types::Polygon<isize>>, DataLoadingError>>()?;
        info!("Finished loading map data in {:?}", start_time.elapsed());
        let scaling = Scaling::output_areas();
        PolygonContainer::new(data, scaling, GRID_SIZE as f64)
    }
    /*    pub fn remove_polygon(&mut self, output_area_id: T) {
        let poly=self.polygons.remove(&output_area_id).unwrap();
        //let poly = Error::from_option(self.polygons.remove(output_area_id), output_area_id, "Cannot delete polygon!".to_string())?;
        self.lookup.delete(geo_polygon_to_quad_area(&poly).unwrap())
    }*/
}
