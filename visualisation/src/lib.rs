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

use geo_types::Coordinate;

use crate::error::{DrawingResult, MyDrawingError};

pub mod citizen_connections;
pub mod error;
pub mod image_export;
#[cfg(feature = "webp")]
pub mod live_render;

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
