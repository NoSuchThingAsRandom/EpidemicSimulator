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

use geo_types::{Coordinate, LineString};
use geo_types::Geometry::{Point, Rect};
use log::debug;
use plotters::chart::{ChartBuilder, ChartContext};
use plotters::coord::types::RangedCoordi32;
use plotters::prelude::{BitMapBackend, Cartesian2d, Color, IntoDrawingArea, Palette, Palette99, RED};
use plotters::style::{BLACK, RGBAColor, RGBColor, WHITE};
use rand::prelude::IteratorRandom;
use rand::thread_rng;

use crate::{OSMRawBuildings, RawBuildingTypes};
use crate::osm_parsing::GRID_SIZE;

fn draw_polygon_ring(
    chart: &mut ChartContext<BitMapBackend, Cartesian2d<RangedCoordi32, RangedCoordi32>>,
    points: &LineString<isize>,
    colour: plotters::style::RGBAColor,
) {
    let points: Vec<(i32, i32)> = points.0.iter().map(|p| (p.x as i32, p.y as i32)).collect();
    chart
        .draw_series(std::iter::once(plotters::prelude::Polygon::new(
            points, plotters::style::ShapeStyle {
                color: colour,
                filled: false,
                stroke_width: 1,
            },
        )))
        .unwrap();
}

const use_fill: bool = true;

pub fn draw(filename: String, data: &OSMRawBuildings, building_type: RawBuildingTypes) {
    println!("Drawing at: {}", filename);
    let draw_backend = BitMapBackend::new(&filename, (GRID_SIZE as u32, GRID_SIZE as u32)).into_drawing_area();
    draw_backend.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&draw_backend)
        .build_cartesian_2d(0..(GRID_SIZE as i32), 0..(GRID_SIZE as i32)).unwrap();
    let mut index = 0;
    let chosen_vorinni = &data.building_vorinnis[&building_type];
    if use_fill {
        for (y, row) in chosen_vorinni.grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                //let c = Palette99::COLORS[*cell as usize];
                let c = ((*cell / 20) as u8, (*cell / 20) as u8, (*cell / 20) as u8);
                let x = x as i32;
                let y = y as i32;


                if index % 100000 == 0 {
                    debug!("Drawing pixel at {} {} with colour {:?}, cell: {}",x,y,c,cell);
                }
                draw_backend.draw_pixel((x, y), &RGBColor(c.0, c.1, c.2)).unwrap();
                index += 1;
            }
        }
    } else {
        for (index, p) in chosen_vorinni.polygons.iter().enumerate() {
            let c = &Palette99::COLORS[index % 20];
            let c = &RGBColor(c.0, c.1, c.2);
            for p in &p.exterior().0 {
                draw_backend.draw_pixel((p.x as i32, p.y as i32), c).unwrap();
            }
            draw_polygon_ring(&mut chart, p.exterior(), c.to_rgba());
        }
    }
    for (x, y) in &chosen_vorinni.seeds {
        draw_backend.draw_pixel((*x as i32, *y as i32), &BLACK).unwrap();
    }
    draw_backend.present().unwrap();
}
