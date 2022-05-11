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

use std::cmp::{max, min};
use std::fmt::format;
use std::time::Instant;

use ggez::conf::WindowMode;
use ggez::event::EventHandler;
use ggez::graphics::{Color, DrawMode, DrawParam, FillOptions, Font, PxScale, Rect, StrokeOptions};
use ggez::{event, graphics, ContextBuilder, GameError};
use log::{error, info};

use sim::models::output_area::OutputArea;
use sim::simulator::Simulator;

pub const SCREEN_WIDTH: u32 = 1920;
pub const SCREEN_HEIGHT: u32 = 1080;

pub fn run(simulator: Simulator) -> anyhow::Result<Simulator> {
    let (mut ctx, event_loop) = ContextBuilder::new("my_game", "Cool Game Author")
        .window_mode(WindowMode::dimensions(
            WindowMode::default(),
            2560.0,
            1440.0,
        ))
        .build()
        .expect("aieee, could not create ggez context!");
    let mut render_sim = RenderSim::new(simulator);
    render_sim.set_screen_bounds(&mut ctx);
    event::run(ctx, event_loop, render_sim);
}

pub enum ColourCodingStrategy {
    TotalPopulation { max_size: f32 },
    InfectedCount { default_colour: Color },
}

pub struct RenderSim {
    simulator: Simulator,
    index: usize,
    time_since_last_print: Instant,
    colour_coding_strategy: ColourCodingStrategy,
    screen_coords: Rect,
}

impl RenderSim {
    pub fn new(simulator: Simulator) -> RenderSim {
        RenderSim {
            simulator,
            index: 0,
            time_since_last_print: Instant::now(),
            colour_coding_strategy: ColourCodingStrategy::InfectedCount {
                default_colour: Color::GREEN,
            },
            screen_coords: Rect::new(0.0, 0.0, SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32),
        }
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
        // TODO Figure out why we need this?
        min_width -= 10000.0;
        min_height -= 10000.0;
        max_width += 10000.0;
        max_height += 10000.0;
        info!(
            "Chosen Grid Size: ({}, {}) to ({}, {})",
            min_width, min_height, max_width, max_height
        );
        let r = Rect::new(
            min_width,
            min_height,
            max_width - min_width,
            max_height - min_height,
        );
        self.screen_coords = r;
        ggez::graphics::set_screen_coordinates(ctx, r).unwrap();
    }
    fn get_colour_for_area(&self, area: &OutputArea) -> Color {
        match self.colour_coding_strategy {
            ColourCodingStrategy::TotalPopulation { max_size } => Color::from((
                max(
                    255,
                    (255.0 * (area.total_residents as f32 / max_size)) as u32,
                ) as u8,
                0,
                0,
            )),
            ColourCodingStrategy::InfectedCount { default_colour } => {
                if let Some(amount) = self
                    .simulator
                    .statistics
                    .output_areas_exposed
                    .get(&area.output_area_id)
                {
                    Color::from((
                        max(
                            255,
                            (255.0 * (area.total_residents as f32 / amount.1 as f32)) as u32,
                        ) as u8,
                        0,
                        0,
                    ))
                } else {
                    default_colour
                }
            }
        }
    }
}

impl EventHandler for RenderSim {
    fn update(&mut self, _ctx: &mut ggez::Context) -> Result<(), GameError> {
        self.index += 1;
        if self.index % 100 == 0 {
            info!(
                "At index {} Time Passed: {:?}, - Statistics: {}",
                self.index,
                self.time_since_last_print.elapsed(),
                self.simulator.statistics
            );
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
            let colour = self.get_colour_for_area(area);

            let points = area
                .polygon
                .exterior()
                .0
                .iter()
                .map(|p| [p.x as f32, p.y as f32])
                .collect::<Vec<[f32; 2]>>();
            let stroke_polygon = graphics::Mesh::new_polygon(
                ctx,
                DrawMode::Stroke(StrokeOptions::default().with_line_width(100.0)),
                &points,
                Color::BLACK,
            )?;
            let fill_polygon = graphics::Mesh::new_polygon(
                ctx,
                DrawMode::Fill(FillOptions::default()),
                &points,
                colour,
            )?;
            graphics::draw(ctx, &stroke_polygon, DrawParam::default())?;
            graphics::draw(ctx, &fill_polygon, DrawParam::default())?;
        }
        // Draw Statistics For Epidemic
        let mut statistics = graphics::Text::new(format!("{}", self.simulator.statistics));
        statistics.set_font(
            Font::default(),
            PxScale::from(20.0 * self.screen_coords.w as f32 / 1920.0),
        );
        let coords = [
            self.screen_coords.x + (self.screen_coords.w / 2.0)
                - statistics.width(ctx) as f32 / 2.0,
            self.screen_coords.y + (self.screen_coords.h * 0.02),
        ];
        let mut params = graphics::DrawParam::default().dest(coords);
        params.color = Color::BLACK;
        graphics::draw(ctx, &statistics, params)?;
        graphics::present(ctx)?;
        Ok(())
    }
}
