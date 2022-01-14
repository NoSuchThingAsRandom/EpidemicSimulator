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

use geo_types::LineString;
use plotters::chart::{ChartBuilder, ChartContext};
use plotters::coord::types::RangedCoordi32;
use plotters::prelude::{BitMapBackend, Cartesian2d, Color, IntoDrawingArea, Palette, Palette99};
use plotters::style::{RGBColor, WHITE};

use crate::{OSMRawBuildings, TagClassifiedBuilding};

fn draw_polygon_ring(
    chart: &mut ChartContext<BitMapBackend, Cartesian2d<RangedCoordi32, RangedCoordi32>>,
    points: &LineString<i32>,
    colour: plotters::style::RGBAColor,
) {
    let points: Vec<(i32, i32)> = points.0.iter().map(|p| (p.x / 25, p.y / 25)).collect();
    chart
        .draw_series(std::iter::once(plotters::prelude::Polygon::new(
            points,
            plotters::style::ShapeStyle {
                color: colour,
                filled: true,
                stroke_width: 1,
            },
        )))
        .unwrap();
}

pub fn draw_osm_buildings_polygons(
    filename: String,
    data: &OSMRawBuildings,
    building_type: TagClassifiedBuilding, grid_size: i32,
) {
    println!("Drawing at: {}", filename);
    let draw_backend =
        BitMapBackend::new(&filename, (grid_size as u32, grid_size as u32)).into_drawing_area();
    draw_backend.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&draw_backend)
        .build_cartesian_2d(0..grid_size, 0..grid_size)
        .unwrap();
    let voroinoi = data.voronoi();
    let chosen_voronoi = &voroinoi[&building_type];
    for (index, p) in &chosen_voronoi.polygons.polygons {
        let c = &Palette99::COLORS[index % 20];
        let c = &RGBColor(c.0, c.1, c.2);
        draw_polygon_ring(&mut chart, p.exterior(), c.to_rgba());
    }
    draw_backend.present().unwrap();
}

pub fn draw_voronoi_polygons(
    filename: String,
    polygons: &[&geo_types::Polygon<i32>],
    grid_size: u32,
) {
    println!("Drawing at: {}", filename);
    let draw_backend = BitMapBackend::new(&filename, (grid_size, grid_size)).into_drawing_area();
    draw_backend.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&draw_backend)
        .build_cartesian_2d(0..(grid_size as i32), 0..(grid_size as i32))
        .unwrap();
    for (index, p) in polygons.iter().enumerate() {
        let c = &Palette99::COLORS[index % 20];
        let c = &RGBColor(c.0, c.1, c.2);
        draw_polygon_ring(&mut chart, p.exterior(), c.to_rgba());
    }
    draw_backend.present().unwrap();
}
