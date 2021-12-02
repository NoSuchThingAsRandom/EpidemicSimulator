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
use std::time::Instant;

use geo_types::{Coordinate, LineString, Polygon};
use log::info;
use shapefile::dbase::FieldValue;
use shapefile::Shape;

use draw::DrawingRecord;
use load_census_data::CensusData;
use load_census_data::parsing_error::{CensusError, ParseErrorType};
use load_census_data::parsing_error::ParseErrorType::MissingKey;

pub mod building;
pub mod citizen;
pub mod output_area;

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

pub fn draw_census_data(
    census_data: &CensusData,
    output_areas_polygons: HashMap<String, Polygon<f64>>,
) -> anyhow::Result<()> {
    let data: Vec<DrawingRecord> = census_data
        .population_counts
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(0.25),
                label: None,
            })
        })
        .collect();
    draw::draw(String::from("PopulationMap.png"), data)?;

    let data: Vec<DrawingRecord> = census_data
        .residents_workplace
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(0.6),
                label: None,
            })
        })
        .collect();
    draw::draw(String::from("ResidentsWorkplace.png"), data)?;

    let data: Vec<DrawingRecord> = census_data
        .occupation_counts
        .iter()
        .filter_map(|(code, _)| {
            Some(DrawingRecord {
                code: code.to_string(),
                polygon: output_areas_polygons.get(code)?.clone(),
                percentage_highlighting: Some(1.0),
                label: None,
            })
        })
        .collect();
    draw::draw(String::from("OccupationCounts.png"), data)?;
    Ok(())
}
