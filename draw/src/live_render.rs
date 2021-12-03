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

use std::cmp::{max, min};
use std::time::{Duration, Instant};

use anyhow::Context;
use geo_types::Coordinate;
use ggez::{ContextBuilder, event, GameError, graphics, mint};
use ggez::conf::WindowMode;
use ggez::event::EventHandler;
use ggez::event::KeyCode::I;
use ggez::graphics::{Color, DrawMode, DrawParam, FillOptions, Rect, StrokeOptions, Transform};
use ggez::mint::Point2;
use log::{debug, error, info};

use sim::simulator::Simulator;

use crate::{convert_geo_point_to_pixel, DrawingResult, GRID_SIZE, X_OFFSET, Y_OFFSET};
use crate::error::MyDrawingError;

pub const SCREEN_WIDTH: u32 = 1920;
pub const SCREEN_HEIGHT: u32 = 1080;

pub fn run(mut simulator: Simulator) -> anyhow::Result<Simulator> {
    //.window_mode()

    let (mut ctx, event_loop) = ContextBuilder::new("my_game", "Cool Game Author").window_mode(WindowMode::dimensions(WindowMode::default(), 2560.0, 1440.0))
        .build()
        .expect("aieee, could not create ggez context!");
    let mut render_sim = RenderSim::new(simulator);
    render_sim.set_screen_bounds(&mut ctx);
    event::run(ctx, event_loop, render_sim);
    Ok(simulator)
}


pub struct RenderSim {
    simulator: Simulator,
    index: usize,
    time_since_last_print: Instant,
    scale: [f32; 2],
}

impl RenderSim {
    pub fn new(simulator: Simulator) -> RenderSim {
        RenderSim { simulator, index: 0, time_since_last_print: Instant::now(), scale: [1.0, 1.0] }
    }
    pub fn set_screen_bounds(&mut self, ctx: &mut ggez::Context) {
        let mut min_width: f32 = f32::MAX;
        let mut min_height: f32 = f32::MAX;
        let mut max_width = 0.0;
        let mut max_height = 0.0;

        for (_, area) in &self.simulator.output_areas {
            for point in &area.polygon.exterior().0 {
                min_width = min(min_width as u32, point.x as u32) as f32;
                min_height = min(min_width as u32, point.y as u32) as f32;
                max_width = max(max_width as u32, point.x as u32) as f32;
                max_height = max(max_height as u32, point.y as u32) as f32;
            }
        }
        min_width -= 10000.0;
        min_height -= 10000.0;
        max_width += 10000.0;
        max_height += 10000.0;
        info!("Chosen Grid Size: ({}, {}) to ({}, {})",min_width,min_height,max_width,max_height);
        info!("Chosen Scale: {:?}",self.scale);
        ggez::graphics::set_screen_coordinates(ctx, Rect::new(min_width, min_height, max_width - min_width, max_height - min_height)).unwrap();
    }
}

impl EventHandler for RenderSim {
    fn update(&mut self, _ctx: &mut ggez::Context) -> Result<(), GameError> {
        self.index += 1;
        if self.index % 100 == 0 {
            info!("At index {} Time Passed: {:?}, - Statistics: {}",self.index,self.time_since_last_print.elapsed(),self.simulator.statistics);
            self.time_since_last_print = Instant::now();
        }
        if !self.simulator.step().unwrap() {
            panic!("Finished");
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut ggez::Context) -> Result<(), GameError> {
        graphics::clear(ctx, [1.0, 1.0, 1.0, 1.0].into());
        for (_, area) in &self.simulator.output_areas {
            let points = area.polygon.exterior().0
                .iter().map(|p| {
                let p = [p.x as f32, p.y as f32];
                p
            }).collect::<Vec<[f32; 2]>>();
            let stroke_polygon = graphics::Mesh::new_polygon(ctx, DrawMode::Stroke(StrokeOptions::default().with_line_width(100.0)), &points, Color::BLACK)?;
            let fill_polygon = graphics::Mesh::new_polygon(ctx, DrawMode::Fill(FillOptions::default()), &points, Color::RED)?;
            graphics::draw(ctx, &stroke_polygon, DrawParam::default().scale(self.scale))?;//ggez::mint::Point2 { x: 0, y: 0 })?;
            graphics::draw(ctx, &fill_polygon, DrawParam::default().scale(self.scale))?;//ggez::mint::Point2 { x: 0, y: 0 })?;
        }
        graphics::present(ctx)?;
        Ok(())
    }
}
