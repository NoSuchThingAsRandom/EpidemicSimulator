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
use std::time::Instant;

use geo_types::{Coordinate, Polygon};
use log::{debug, info};
use plotters::chart::ChartContext;
use plotters::coord::Shift;
use plotters::coord::types::RangedCoordi32;
use plotters::drawing::DrawingArea;
use plotters::prelude::{
    BitMapBackend, Cartesian2d, ChartBuilder, Color, IntoDrawingArea, IntoFont, Palette, Palette99,
    RED, ShapeStyle, WHITE,
};
use plotters::style::TextStyle;
use polylabel::polylabel;

use osm_data::TagClassifiedBuilding;

use crate::{convert_geo_point_to_pixel, GRID_SIZE, PIXEL_SIZE, SCALE};
use crate::error::DrawingResult;

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
    pub filled: bool,
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
            filled: false,
        }
    }
}

impl From<(String, &Polygon<i32>, Option<f64>)> for DrawingRecord {
    fn from(data: (String, &Polygon<i32>, Option<f64>)) -> Self {
        DrawingRecord {
            code: data.0,
            polygon: geo_types::Polygon::new(
                data.1
                    .exterior()
                    .0
                    .iter()
                    .map(|p| (p.x as f64, p.y as f64).into())
                    .collect::<Vec<geo_types::Coordinate<f64>>>()
                    .into(),
                data.1
                    .interiors()
                    .iter()
                    .map(|l| {
                        l.0.iter()
                            .map(|p| (p.x as f64, p.y as f64).into())
                            .collect::<Vec<geo_types::Coordinate<f64>>>()
                            .into()
                    })
                    .collect(),
            ),
            percentage_highlighting: data.2,
            label: None,
            filled: false,
        }
    }
}

impl From<(String, &Polygon<i32>, Option<f64>, bool)> for DrawingRecord {
    fn from(data: (String, &Polygon<i32>, Option<f64>, bool)) -> Self {
        DrawingRecord {
            code: data.0,
            polygon: geo_types::Polygon::new(
                data.1
                    .exterior()
                    .0
                    .iter()
                    .map(|p| (p.x as f64, p.y as f64).into())
                    .collect::<Vec<geo_types::Coordinate<f64>>>()
                    .into(),
                data.1
                    .interiors()
                    .iter()
                    .map(|l| {
                        l.0.iter()
                            .map(|p| (p.x as f64, p.y as f64).into())
                            .collect::<Vec<geo_types::Coordinate<f64>>>()
                            .into()
                    })
                    .collect(),
            ),
            percentage_highlighting: data.2,
            label: None,
            filled: data.3,
        }
    }
}

impl From<(String, Polygon<i32>, Option<f64>)> for DrawingRecord {
    fn from(data: (String, Polygon<i32>, Option<f64>)) -> Self {
        DrawingRecord {
            code: data.0,
            polygon: geo_types::Polygon::new(
                data.1
                    .exterior()
                    .0
                    .iter()
                    .map(|p| (p.x as f64, p.y as f64).into())
                    .collect::<Vec<geo_types::Coordinate<f64>>>()
                    .into(),
                data.1
                    .interiors()
                    .iter()
                    .map(|l| {
                        l.0.iter()
                            .map(|p| (p.x as f64, p.y as f64).into())
                            .collect::<Vec<geo_types::Coordinate<f64>>>()
                            .into()
                    })
                    .collect(),
            ),
            percentage_highlighting: data.2,
            label: None,
            filled: false,
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
            filled: false,
        }
    }
}

fn draw_polygon_ring_filled(
    chart: &mut ChartContext<BitMapBackend, Cartesian2d<RangedCoordi32, RangedCoordi32>>,
    points: &[Coordinate<f64>],
    colour: plotters::style::RGBColor,
) -> DrawingResult<()> {
    let points = points
        .iter()
        .map(|p| convert_geo_point_to_pixel(*p))
        .collect::<DrawingResult<Vec<(i32, i32)>>>()?;
    // Draw Outline
    chart
        .draw_series(std::iter::once(plotters::prelude::PathElement::new(
            points.clone(),
            ShapeStyle {
                color: colour.to_rgba(),
                filled: false,
                stroke_width: 1,
            },
        )))
        .unwrap();
    // Draw fill
    chart
        .draw_series(std::iter::once(plotters::prelude::Polygon::new(
            points,
            ShapeStyle {
                color: RED.to_rgba(),
                filled: true,
                stroke_width: 1,
            },
        )))
        .unwrap();

    Ok(())
}

fn draw_polygon_ring(
    draw_backend: &DrawingArea<BitMapBackend, Shift>,
    points: &[Coordinate<f64>],
    colour: plotters::style::RGBColor,
) -> DrawingResult<()> {
    let points = points
        .iter()
        .map(|p| {
            convert_geo_point_to_pixel(geo_types::Coordinate::from((
                p.x as f64,
                ((p.y as i32 - GRID_SIZE as i32).abs() as f64),
            )))
        })
        .collect::<DrawingResult<Vec<(i32, i32)>>>()?;
    let polygon = plotters::element::Polygon::new(points, colour);
    draw_backend.draw(&polygon)?;
    Ok(())
}

fn render_output_areas(
    data: Vec<DrawingRecord>,
    draw_backend: &DrawingArea<BitMapBackend, Shift>,
    chart: &mut ChartContext<BitMapBackend, Cartesian2d<RangedCoordi32, RangedCoordi32>>,
) -> DrawingResult<()> {
    let start_time = Instant::now();
    let style = TextStyle::from(("sans-serif", 20).into_font()).color(&RED);
    for (index, area) in data.iter().enumerate() {
        if let Some(label) = &area.label {
            let centre = Coordinate::from(polylabel(&area.polygon, &0.1)?);
            let centre = convert_geo_point_to_pixel(centre)?;
            draw_backend.draw_text(label, &style, centre).unwrap();
        }

        // Draw exterior ring
        let c = (area.percentage_highlighting.unwrap_or(1.0) * 255.0).ceil() as u8;
        let mut colour = plotters::style::RGBColor(0, 0, 0);
        if area.filled {
            colour = RED;
            /*draw_polygon_ring_filled(chart, &area.polygon.exterior().0, colour)?;
                for p in area.polygon.interiors() {
                    draw_polygon_ring_filled(chart, &area.polygon.exterior().0, colour)?;
                }
            } else {*/
        }
        draw_polygon_ring(draw_backend, &area.polygon.exterior().0, colour)?;
        for p in area.polygon.interiors() {
            draw_polygon_ring(draw_backend, &p.0, colour)?;
        }

        if index % 1000 == 0 {
            debug!(
                "  Drawing the {} output area at time {:?}",
                index,
                start_time.elapsed()
            );
        }
    }
    Ok(())
}

/// Creates a png at the given filename, from the List of Output Areas
pub fn draw_output_areas(filename: String, data: Vec<DrawingRecord>) -> DrawingResult<()> {
    let start_time = Instant::now();
    info!("Drawing output areas on map...");
    let draw_backend = BitMapBackend::new(&filename, (PIXEL_SIZE, PIXEL_SIZE)).into_drawing_area();
    draw_backend.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&draw_backend)
        .build_cartesian_2d(0..(PIXEL_SIZE as i32), 0..(PIXEL_SIZE as i32))?;
    render_output_areas(data, &draw_backend, &mut chart)?;
    draw_backend.present().unwrap();
    info!("Finished drawing in {:?}", start_time.elapsed());
    Ok(())
}

#[inline]
fn building_colour(
    class: osm_data::TagClassifiedBuilding,
) -> plotters::style::RGBColor {
    let index = match class {
        TagClassifiedBuilding::Shop => 1,
        TagClassifiedBuilding::School => 2,
        TagClassifiedBuilding::Hospital => 3,
        TagClassifiedBuilding::Household => 4,
        TagClassifiedBuilding::WorkPlace => 5,
        TagClassifiedBuilding::Unknown => 6,
    };
    let c = &Palette99::COLORS[index];
    plotters::style::RGBColor(c.0, c.1, c.2)
}

fn render_buildings(
    buildings: Vec<osm_data::RawBuilding>,
    draw_backend: &DrawingArea<BitMapBackend, Shift>,
) -> DrawingResult<()> {
    let start_time = Instant::now();
    for (index, building) in buildings.iter().enumerate() {
        let colour = building_colour(building.classification());
        let size = ((building.size().max(1) / SCALE) as f64).sqrt().ceil() as i32;
        let side_length = size / 2;

        let p = convert_geo_point_to_pixel(geo_types::Coordinate::from((
            building.center().x() as f64,
            ((building.center().y() as i32 - GRID_SIZE as i32).abs() as f64),
        )))?;
        let top_left = (p.0 - side_length, p.1 - side_length);
        let bottom_right = (p.0 + side_length, p.1 + side_length);
        let rect = plotters::element::Rectangle::new([top_left, bottom_right], colour);

        if index % 1000000 == 0 {
            debug!(
                "  Drawing the {} rect ({:?},{:?}) with colour {:?} at time {:?}",
                index,
                top_left,
                bottom_right,
                colour,
                start_time.elapsed()
            );
        }
        draw_backend.draw(&rect)?;
    }
    Ok(())
}

pub fn draw_buildings(
    filename: String,
    buildings: Vec<osm_data::RawBuilding>,
) -> DrawingResult<()> {
    let start_time = Instant::now();
    info!("Drawing buildings on map...");
    // TODO Did we fuck up the lat and lon somewhere?
    //let scale = ((700000 as f64 / PIXEL_SIZE as f64).ceil() as i32).max(1);
    let draw_backend = BitMapBackend::new(&filename, (PIXEL_SIZE, PIXEL_SIZE)).into_drawing_area();
    draw_backend.fill(&WHITE)?;
    render_buildings(buildings, &draw_backend)?;
    draw_backend.present().unwrap();
    info!("Finished drawing in {:?}", start_time.elapsed());
    Ok(())
}

/// Creates a png at the given filename, from the List of Output Areas
pub fn draw_buildings_and_output_areas(
    filename: String,
    data: Vec<DrawingRecord>,
    buildings: Vec<osm_data::RawBuilding>,
) -> DrawingResult<()> {
    let start_time = Instant::now();
    info!("Drawing output areas and buildings on map");
    let draw_backend = BitMapBackend::new(&filename, (PIXEL_SIZE, PIXEL_SIZE)).into_drawing_area();
    draw_backend.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&draw_backend)
        .build_cartesian_2d(0..(PIXEL_SIZE as i32), 0..(PIXEL_SIZE as i32))?;
    render_output_areas(data, &draw_backend, &mut chart)?;
    render_buildings(buildings, &draw_backend)?;
    draw_backend.present().unwrap();
    info!("Finished drawing in {:?}", start_time.elapsed());
    Ok(())
}
