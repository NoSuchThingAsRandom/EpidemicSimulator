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
use std::time::Instant;

use geo_types::{Coordinate, Polygon};
use log::{debug, info};
use plotters::chart::ChartContext;
use plotters::coord::types::RangedCoordi32;
use plotters::prelude::{
    BitMapBackend, Cartesian2d, ChartBuilder, IntoDrawingArea, IntoFont, RED, WHITE,
};
use plotters::style::TextStyle;
use polylabel::polylabel;

use crate::error::{DrawingResult, MyDrawingError};

mod error;

pub const GRID_SIZE: u32 = 32800;
const X_OFFSET: i32 = 75000;
const Y_OFFSET: i32 = 1000;

/// Converts a geo_types::Coordinate to a Pixel Mapping on the GRID
fn convert_geo_point_to_pixel(coords: Coordinate<f64>) -> DrawingResult<(i32, i32)> {
    let coords = (
        (coords.x - X_OFFSET as f64) as i32 / 21,
        (coords.y - Y_OFFSET as f64) as i32 / 21,
    );
    if coords.0 > GRID_SIZE as i32 {
        return Err(MyDrawingError::ConversionError {
            message: format!(
                "X Coordinate {} exceeds maximum Grid Size {}",
                coords.0, GRID_SIZE
            ),
            value: Some(coords.0.to_string()),
        });
    } else if coords.1 > GRID_SIZE as i32 {
        return Err(MyDrawingError::ConversionError {
            message: format!(
                "Y Coordinate {} exceeds maximum Grid Size {}",
                coords.0, GRID_SIZE
            ),
            value: Some(coords.1.to_string()),
        });
    } else if coords.0 < 0 {
        return Err(MyDrawingError::ConversionError {
            message: "X Coordinate is negative!".to_string(),
            value: Some(coords.0.to_string()),
        });
    } else if coords.1 < 0 {
        return Err(MyDrawingError::ConversionError {
            message: "Y Coordinate is negative!".to_string(),
            value: Some(coords.1.to_string()),
        });
    }

    Ok((coords.0 as i32, coords.1 as i32))
}

/// This is a representation of an Output Area to be passed to the Draw function
pub struct DrawingRecord {
    /// The name of the output area
    pub code: String,
    /// The polyon representing it's shape
    pub polygon: Polygon<f64>,
    /// What percentage of the default colour to apply
    pub percentage_highlighting: Option<f64>,
    /// If a label should be placed on the image
    pub label: Option<String>,
}

impl DrawingRecord {
    pub fn from_hashmap(output_areas: HashMap<String, Polygon<f64>>) -> Vec<DrawingRecord> {
        output_areas.iter().map(DrawingRecord::from).collect()
    }
}

impl From<(String, Polygon<f64>)> for DrawingRecord {
    fn from(data: (String, Polygon<f64>)) -> Self {
        DrawingRecord {
            code: data.0,
            polygon: data.1,
            percentage_highlighting: None,
            label: None,
        }
    }
}

impl From<(&String, &Polygon<f64>)> for DrawingRecord {
    fn from(data: (&String, &Polygon<f64>)) -> Self {
        DrawingRecord {
            code: data.0.to_string(),
            polygon: data.1.clone(),
            percentage_highlighting: None,
            label: None,
        }
    }
}

/// Creates a png at the given filename, from the List of Output Areas
pub fn draw(filename: String, data: Vec<DrawingRecord>) -> DrawingResult<()> {
    let start_time = Instant::now();
    info!("Drawing output areas on map...");
    let draw_backend = BitMapBackend::new(&filename, (GRID_SIZE, GRID_SIZE)).into_drawing_area();
    draw_backend.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&draw_backend)
        .build_cartesian_2d(0..(GRID_SIZE as i32), 0..(GRID_SIZE as i32))?;

    let style = TextStyle::from(("sans-serif", 20).into_font()).color(&RED);
    for (index, area) in data.iter().enumerate() {
        if let Some(label) = &area.label {
            let centre = Coordinate::from(polylabel(&area.polygon, &0.1)?);
            let centre = convert_geo_point_to_pixel(centre)?;
            draw_backend.draw_text(label, &style, centre).unwrap();
        }

        // Draw exterior ring
        let c = (area.percentage_highlighting.unwrap_or(1.0) * 255.0).ceil() as u8;
        let colour = plotters::style::RGBColor(c, 0, 0);
        draw_polygon_ring(&mut chart, &area.polygon.exterior().0, colour)?;
        for p in area.polygon.interiors() {
            draw_polygon_ring(&mut chart, &p.0, colour)?;
        }

        if index % 100 == 0 {
            debug!(
                "  Drawing the {} output area at time {:?}",
                index,
                start_time.elapsed()
            );
        }
    }
    draw_backend.present().unwrap();
    info!("Finished drawing in {:?}", start_time.elapsed());
    Ok(())
}

fn draw_polygon_ring(
    chart: &mut ChartContext<BitMapBackend, Cartesian2d<RangedCoordi32, RangedCoordi32>>,
    points: &[Coordinate<f64>],
    colour: plotters::style::RGBColor,
) -> DrawingResult<()> {
    let points = points
        .iter()
        .map(|p| convert_geo_point_to_pixel(*p))
        .collect::<DrawingResult<Vec<(i32, i32)>>>()?;
    chart
        .draw_series(std::iter::once(plotters::prelude::Polygon::new(
            points, &colour,
        )))
        .unwrap();
    Ok(())
}
