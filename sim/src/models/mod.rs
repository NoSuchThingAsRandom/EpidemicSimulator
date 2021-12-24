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

#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::time::Instant;

use anyhow::Context;
use geo_types::{Coordinate, LineString, Point, Polygon};
use log::{debug, error, info};
use serde::Serialize;
use shapefile::dbase::FieldValue;
use shapefile::Shape;

use load_census_data::osm_parsing::GRID_SIZE;
use load_census_data::parsing_error::{DataLoadingError, ParseErrorType};
use load_census_data::parsing_error::ParseErrorType::MissingKey;
use load_census_data::voronoi_generator::{PolygonContainer, Scaling};

use crate::config::get_memory_usage;
use crate::error::Error;
use crate::models::building::BuildingID;
use crate::models::output_area::OutputAreaID;
use crate::models::public_transport_route::PublicTransportID;

pub mod building;
pub mod citizen;
pub mod output_area;
pub mod public_transport_route;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize)]
pub enum ID {
    Building(BuildingID),
    OutputArea(OutputAreaID),
    PublicTransport(PublicTransportID),
}

impl Display for ID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ID::Building(id) => {
                write!(f, "{}", id)
            }
            ID::OutputArea(id) => {
                write!(f, "{}", id)
            }
            ID::PublicTransport(id) => {
                write!(f, "{}", id)
            }
        }
    }
}
/*
pub struct PointLookup {
    // Row -> Column -> Code
    boxes: Vec<Vec<Vec<OutputAreaID>>>,
    box_size: usize,
}

impl std::default::Default for PointLookup {
    fn default() -> Self {
        // TODO Find a better box size?
        PointLookup { boxes: Vec::with_capacity(2000), box_size: 2000 }
    }
}

impl PointLookup {
    pub fn add_area(&mut self, code: String, polygon: &geo_types::Polygon<isize>) {
        return;
        let bounds = polygon.bounding_rect().expect("Failed to obtain bounding rect for polygon!");
        if bounds.min().x < 0 || bounds.min().y < 0 {
            panic!("Don't support negative Coordinates!");
        }
        for row_offset in 0..(bounds.height() as usize / self.box_size) {
            // Extend rows to match new index
            let row_index = bounds.min().y as usize + row_offset;
            if self.boxes.len() < row_index + 1 {
                debug!("Creating {} rows", row_index + 1-self.boxes.len());
            }
            while self.boxes.len() < row_index + 1 {
                self.boxes.push(Vec::new());
            }
            for col_offset in 0..(bounds.width() as usize / self.box_size) {
                let col_index = bounds.min().x as usize + col_offset;
                let row_count = self.boxes.len();
                let current_row = self.boxes.get_mut(row_index).unwrap_or_else(|| panic!("Extending the rows failed in adding area. Row Size: {}, Index: {}", row_count, row_index));
                // Extend cols to match new index
                if current_row.len() < row_index + 1 {
                    debug!("Creating {} cols", row_index + 1-current_row.len());
                }
                while current_row.len() < col_index + 1 {
                    current_row.push(Vec::new());
                }
                let column_size = current_row.len();
                let cell = current_row.get_mut(col_index).unwrap_or_else(|| panic!("Extending the columns failed, in adding area. Col Size: {}, Index: {}", column_size, col_index));
                cell.push(OutputAreaID::from_code(code.to_string()));
            }
        }
    }
    pub fn get_possible_area_codes(&self, point: &geo_types::Point<isize>) -> Option<&Vec<OutputAreaID>> {
        if point.x() < 0 || point.y() < 0 {
            panic!("Don't support negative Coordinates!");
        }
        if let Some(row) = self.boxes.get((point.y() as usize / self.box_size) as usize) {
            if let Some(cell) = row.get((point.x() as usize / self.box_size) as usize) {
                return Some(cell);
            }
        }
        None
    }
}
*/

/// A wrapper around Polygon Container to implement Scaling on Polygons
///
/// Reducing the memory requirements
pub struct OutputAreaPolygons {
    lookup: PolygonContainer<isize>,
    scaling: Scaling,
}

impl OutputAreaPolygons {
    /// Generates the polygons for each output area contained in the given file
    pub fn load_polygons_from_file(
        filename: &str,
    ) -> Result<
        OutputAreaPolygons,
        DataLoadingError,
    > {
        let mut reader =
            shapefile::Reader::from_path(filename).map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
            })?;
        let mut start_time = Instant::now();
        let mut data = HashMap::new();
        let mut polygons = Vec::new();
        info!("Loading map data from file...");
        for (index, shape_record) in reader.iter_shapes_and_records().enumerate() {
            let (shape, record) = shape_record.map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
            })?;
            if let Shape::Polygon(polygon) = shape {
                assert!(!polygon.rings().is_empty());
                let rings: Vec<Coordinate<isize>>;
                let mut interior_ring;
                if polygon.rings().len() == 1 {
                    rings = polygon.rings()[0]
                        .points()
                        .iter()
                        .map(|p| {
                            geo_types::Coordinate::from((p.x.round() as isize, p.y.round() as isize))
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
                                        geo_types::Coordinate::from((
                                            p.x.round() as isize,
                                            p.y.round() as isize,
                                        ))
                                    })
                                    .collect::<Vec<Coordinate<isize>>>(),
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
                let new_poly = geo_types::Polygon::new(LineString::from(rings), interior_ring);

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
                let code;
                match code_record {
                    FieldValue::Character(value) => {
                        code = value.ok_or_else(|| DataLoadingError::ValueParsingError {
                            source: ParseErrorType::IsEmpty {
                                message: "The code for an Output Area is empty".to_string(),
                            }
                        })?;
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

                data.insert(OutputAreaID::from_code(code), new_poly);
                if index % 10000 == 0 {
                    debug!(
                    "Built {} polygons in time: {}",
                    index * 10000,
                    start_time.elapsed().as_secs_f64()
                );
                    start_time = Instant::now();
                    debug!(
                    "Current memory usage: {}",
                    get_memory_usage().expect("Failed to retrieve memory usage")
                );
                }
            } else {
                return Err(DataLoadingError::ValueParsingError {
                    source: ParseErrorType::InvalidDataType {
                        value: Some(shape.shapetype().to_string()),
                        expected_type: "Unexpected shape type!".to_string(),
                    },
                });
            }
        }
        info!("Finished loading map data in {:?}", start_time.elapsed());
        let scaling = Scaling::output_areas();
        Ok(OutputAreaPolygons {
            polygons: data,
            lookup: PolygonContainer::new(data, GRID_SIZE),
            scaling,
            grid_size: GRID_SIZE,
        })
    }
    pub fn build_lookup(&mut self) -> Result<(), DataLoadingError> {
        info!("Building lookup for Output Areas");
        let polygons = self.polygons.iter().map(|(id, polygon)| (self.scaling.scale_polygon(polygon, GRID_SIZE as isize), id.to_string())).collect();
        self.lookup = PolygonContainer::new(polygons, self.grid_size as f64)?
    }
    pub fn remove_polygon(&mut self, output_area_id: &OutputAreaID) {
        let poly = Error::from_option(self.polygons.remove(output_area_id), output_area_id, "Cannot delete polygon!".to_string())?;
        if let Some(lookup) = &mut self.lookup {
            lookup.lookup.delete()
        }
    }

    pub fn get_output_area_containing_point(&self, point: &Point<isize>) -> anyhow::Result<OutputAreaID> {
        if let Some(lookup) = &self.lookup {
            let point = self.scaling.scale_point((point.x() as usize, point.y() as usize), self.grid_size as isize).into();
            let areas = lookup
                .find_polygon_for_point(point)
                .context("Finding output area containing point")?;
            Ok(OutputAreaID::from_code(areas.to_string()))
        } else {
            error!("Output Area Polygon lookup has not been initialised");
            Err(crate::error::Error::InitializationError { message: "Output Area Polygon lookup has not been initialised".to_string() }.into())
        }
    }
}
