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

#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::time::Instant;

use geo_types::{Coordinate, LineString};
use log::info;
use serde::{Deserialize, Serialize};
use shapefile::dbase::FieldValue;
use shapefile::Shape;

use load_census_data::parsing_error::{CensusError, ParseErrorType};
use load_census_data::parsing_error::ParseErrorType::MissingKey;

use crate::models::building::BuildingID;
use crate::models::output_area::OutputAreaID;
use crate::models::public_transport_route::PublicTransportID;

pub mod building;
pub mod citizen;
pub mod output_area;
pub mod public_transport_route;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Deserialize, Serialize)]
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

/// Generates the polygons for each output area contained in the given file
pub fn build_polygons_for_output_areas(
    filename: &str,
) -> Result<HashMap<String, geo_types::Polygon<f64>>, CensusError> {
    let mut reader = shapefile::Reader::from_path(filename).map_err(|e| CensusError::IOError {
        source: Box::new(e),
    })?;
    let start_time = Instant::now();
    let mut data = HashMap::new();
    info!("Loading map data from file...");
    for (_, shape_record) in reader.iter_shapes_and_records().enumerate() {
        let (shape, record) = shape_record.map_err(|e| CensusError::IOError {
            source: Box::new(e),
        })?;
        if let Shape::Polygon(polygon) = shape {
            assert!(!polygon.rings().is_empty());
            let rings: Vec<Coordinate<f64>>;
            let mut interior_ring;
            if polygon.rings().len() == 1 {
                rings = polygon.rings()[0]
                    .points()
                    .iter()
                    .map(|p| geo_types::Coordinate::from(*p))
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
                                .map(|p| geo_types::Coordinate::from(*p))
                                .collect::<Vec<Coordinate<f64>>>(),
                        )
                    })
                    .collect();
                rings = interior_ring
                    .pop()
                    .ok_or_else(|| CensusError::ValueParsingError {
                        source: ParseErrorType::IsEmpty {
                            message: "Expected an interior ring to exist!".to_string(),
                        },
                    })?
                    .0;
            }
            let new_poly = geo_types::Polygon::new(LineString::from(rings), interior_ring);

            // Retrieve the area code:
            let code_record = record
                .get("code")
                .ok_or_else(|| CensusError::ValueParsingError {
                    source: MissingKey {
                        context: "Output Area is missing it's code".to_string(),
                        key: "code".to_string(),
                    },
                })?;
            let code;
            if let FieldValue::Character(option_val) = code_record {
                code = option_val.clone().unwrap_or_else(|| String::from(""));
            } else {
                return Err(CensusError::ValueParsingError {
                    source: ParseErrorType::InvalidDataType {
                        value: Some(code_record.field_type().to_string()),
                        expected_type: "Expected type 'character' for area code".to_string(),
                    },
                });
            }

            data.insert(code, new_poly);
        } else {
            return Err(CensusError::ValueParsingError {
                source: ParseErrorType::InvalidDataType {
                    value: Some(shape.shapetype().to_string()),
                    expected_type: "Unexpected shape type!".to_string(),
                },
            });
        }
        /*        if index % DEBUG_ITERATION_PRINT == 0 {
            debug!("  At index {} with time {:?}", index, start_time.elapsed());
        }*/
    }
    info!("Finished loading map data in {:?}", start_time.elapsed());
    Ok(data)
}
