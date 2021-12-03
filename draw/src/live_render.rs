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

use std::time::Duration;

use anyhow::Context;
use log::{debug, error};
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};

use sim::simulator::Simulator;

use crate::{convert_geo_point_to_pixel, DrawingResult};
use crate::error::MyDrawingError;

pub const SCREEN_WIDTH: u32 = 1920;
pub const SCREEN_HEIGHT: u32 = 1080;

pub fn run(mut simulator: Simulator) -> anyhow::Result<Simulator> {
    let event_loop = EventLoop::new();

    let (window, p_width, p_height, mut _hidpi_factor) =
        create_window("Conway's Game of Life", &event_loop);

    let surface_texture = SurfaceTexture::new(p_width, p_height, &window);

    let mut pixels = Pixels::new(SCREEN_WIDTH, SCREEN_HEIGHT, surface_texture).unwrap();
    let mut paused = false;

    let mut draw_state: Option<bool> = None;
    event_loop.run(move |event, _, control_flow| {
        // The one and only event that winit_input_helper doesn't have for us...
        if let Event::RedrawRequested(_) = event {
            debug!("Drawing");
            let mut screen = pixels.get_frame();
            screen.fill(0);
            render(&simulator, pixels.get_frame()).unwrap();
            if pixels
                .render()
                .map_err(|e| error!("pixels.render() failed: {:?}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }
        // Do update stuff here?
        if !simulator.step().context("Simulation time step failed").unwrap() {
            return;
        }
        /*        // Adjust high DPI factor
                if let Some(factor) = input.scale_factor_changed() {
                    _hidpi_factor = factor;
                }
                // Resize the window
                if let Some(size) = input.window_resized() {
                    pixels.resize_surface(size.width, size.height);
                }
                if !paused || input.key_pressed(VirtualKeyCode::Space) {
                    life.update();
                }*/
        //std::thread::sleep(Duration::from_secs(1));
        window.request_redraw();
    });
    Ok(simulator)
}

/// Copy pasted from: https://github.com/parasyte/pixels/blob/2a4ebbf19d47fd8e5cdc4d4311e82393ec50ca1d/examples/conway/src/main.rs#L129
/// Create a window for the game.
///
/// Automatically scales the window to cover about 2/3 of the monitor height.
///
/// # Returns
///
/// Tuple of `(window, surface, width, height, hidpi_factor)`
/// `width` and `height` are in `PhysicalSize` units.
fn create_window(
    title: &str,
    event_loop: &EventLoop<()>,
) -> (winit::window::Window, u32, u32, f64) {
    // Create a hidden window so we can estimate a good default window size
    let window = winit::window::WindowBuilder::new()
        .with_visible(false)
        .with_title(title)
        .build(event_loop)
        .unwrap();
    let hidpi_factor = window.scale_factor();

    // Get dimensions
    let width = SCREEN_WIDTH as f64;
    let height = SCREEN_HEIGHT as f64;
    let (monitor_width, monitor_height) = {
        if let Some(monitor) = window.current_monitor() {
            let size = monitor.size().to_logical(hidpi_factor);
            (size.width, size.height)
        } else {
            (width, height)
        }
    };
    let scale = (monitor_height / height * 2.0 / 3.0).round().max(1.0);

    // Resize, center, and display the window
    let min_size: winit::dpi::LogicalSize<f64> =
        PhysicalSize::new(width, height).to_logical(hidpi_factor);
    let default_size = LogicalSize::new(width * scale, height * scale);
    let center = LogicalPosition::new(
        (monitor_width - width * scale) / 2.0,
        (monitor_height - height * scale) / 2.0,
    );
    window.set_inner_size(default_size);
    window.set_min_inner_size(Some(min_size));
    window.set_outer_position(center);
    window.set_visible(true);

    let size = default_size.to_physical::<f64>(hidpi_factor);

    (
        window,
        size.width.round() as u32,
        size.height.round() as u32,
        hidpi_factor,
    )
}

pub fn draw_pixel(x: usize, y: usize, screen: &mut [u8]) {
    let pixel_size = 4;
    let pixel_offset = (SCREEN_WIDTH as usize * y * pixel_size) + (x * pixel_size);
    let c: &[u8] = &[255, 255, 255, 255];
    for (index, c_val) in c.iter().enumerate() {
        screen[pixel_offset + index] = *c_val;
    }
}

pub fn render(simulator: &Simulator, screen: &mut [u8]) -> anyhow::Result<()> {
    for (index, area) in &simulator.output_areas {
        area.polygon.exterior().0
            .iter().for_each(|p| {
            let p = convert_geo_point_to_pixel(*p).unwrap();
            let p = (p.0 as usize, p.1 as usize);
            draw_pixel(p.0, p.1, screen);
        });
    }
    Ok(())
}