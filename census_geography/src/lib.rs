#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use geo_types::{Coordinate, LineString};
use log::{debug, info};
use shapefile::Shape;
use shapefile::dbase::FieldValue;
use uuid::Uuid;

use load_census_data::parsing_error::{CensusError, ParseErrorType};
use load_census_data::parsing_error::ParseErrorType::MissingKey;
use load_census_data::table_144_enum_values::AreaClassification;

pub mod output_area;
pub mod household;
pub mod citizen;

const DEBUG_ITERATION: usize = 5000;


/// Generates the polygons for each output area contained in the given file
pub fn build_polygons_for_output_areas(filename: &str) -> Result<HashMap<String, geo_types::Polygon<f64>>, CensusError> {
    let mut reader =
        shapefile::Reader::from_path(filename).map_err(|e| CensusError::IOError { source: Box::new(e) })?;
    let start_time = Instant::now();
    let mut data = HashMap::new();
    info!("Loading map data from file...");
    for (index, shape_record) in reader.iter_shapes_and_records().enumerate() {
        let (shape, record) = shape_record.map_err(|e| CensusError::IOError { source: Box::new(e) })?;
        if let Shape::Polygon(polygon) = shape {
            assert!(!polygon.rings().is_empty());
            let rings: Vec<Coordinate<f64>>;
            let mut interior_ring;
            if polygon.rings().len() == 1 {
                rings = polygon.rings()[0].points().iter().map(|p| geo_types::Coordinate::from(*p)).collect();
                interior_ring = Vec::new();
            } else {
                interior_ring = polygon.rings().iter().map(|r| LineString::from(r.points().iter().map(|p| geo_types::Coordinate::from(*p)).collect::<Vec<Coordinate<f64>>>())).collect();
                rings = interior_ring.pop().ok_or_else(|| CensusError::ValueParsingError { source: ParseErrorType::IsEmpty { message: "Expected an interior ring to exist!".to_string() } })?.0;
            }
            let new_poly = geo_types::Polygon::new(LineString::from(rings), interior_ring);

            // Retrieve the area code:
            let code_record = record.get("code").ok_or_else(|| CensusError::ValueParsingError { source: MissingKey { context: "Output Area is missing it's code".to_string(), key: "code".to_string() } })?;
            let code;
            if let FieldValue::Character(option_val) = code_record {
                code = option_val.clone().unwrap_or_else(|| String::from(""));
            } else {
                return Err(CensusError::ValueParsingError { source: ParseErrorType::InvalidDataType { value: Some(code_record.field_type().to_string()), expected_type: "Expected type 'character' for area code".to_string() } });
            }

            data.insert(code, new_poly);
        } else {
            return Err(CensusError::ValueParsingError { source: ParseErrorType::InvalidDataType { value: Some(shape.shapetype().to_string()), expected_type: "Unexpected shape type!".to_string() } });
        }
        if index % DEBUG_ITERATION == 0 {
            debug!("  At index {} with time {:?}", index, start_time.elapsed());
        }
    }
    info!("Finished loading map data in {:?}", start_time.elapsed());
    Ok(data)
}

/// This is used to represent a building location
///
/// It utilises:
/// * An `OutputArea` - for broad location in the country,
/// * An `AreaClassification` for differentiating between (Rural, Urban, Etc),
/// * A  `Uuid` for a unique building identifier
#[derive(Clone)]
pub struct BuildingCode {
    output_area_code: String,
    area_type: AreaClassification,
    building_id: uuid::Uuid,
}

impl BuildingCode {
    /// Generates a new `BuildingCode` in the given position, with a new random building ID (`Uuid`)
    ///
    /// # Example
    /// ```
    /// use census_geography::BuildingCode;
    /// use load_census_data::table_144_enum_values::AreaClassification;
    ///
    /// let output_area = String::from("1234");
    /// let area_type = AreaClassification::UrbanCity;
    ///
    /// let building_code = BuildingCode::new(output_area, area_type);
    ///
    /// assert_eq!(building_code.output_area_code(), output_area);
    /// assert_eq!(building_code.area_type(), area_type);
    ///
    /// ```
    pub fn new(output_code: String, area_type: AreaClassification) -> BuildingCode {
        BuildingCode {
            output_area_code: output_code,
            area_type,
            building_id: Uuid::new_v4(),
        }
    }
    /// Returns the `OutputArea` code
    pub fn output_area_code(&self) -> String {
        String::from(&self.output_area_code)
    }
    /// Returns the type of area this building is located in
    pub fn area_type(&self) -> AreaClassification {
        self.area_type
    }
    /// Returns the unique ID of this `BuildingCode`
    pub fn building_id(&self) -> Uuid {
        self.building_id
    }
}
