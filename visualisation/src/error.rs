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

use std::fmt::{Debug, Display, Formatter};

use plotters::drawing::DrawingAreaErrorKind;
use polylabel::errors::PolylabelError;

pub type DrawingResult<T> = std::result::Result<T, MyDrawingError>;

pub enum MyDrawingError {
    Default {
        message: String,
    },
    Drawing {
        message: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    ConversionError {
        value: Option<String>,
        message: String,
    },
}

impl Display for MyDrawingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MyDrawingError::Default { message } => {
                write!(f, "Error: {}", message)
            }
            MyDrawingError::ConversionError { message, value } => {
                write!(f, "Failed to convert value {:?}\n  {}", value, message)
            }
            MyDrawingError::Drawing { message, source } => {
                write!(f, "{} -> {}", message, source)
            }
        }
    }
}

impl Debug for MyDrawingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl<T: 'static + std::error::Error + Send + Sync> From<plotters::drawing::DrawingAreaErrorKind<T>>
    for MyDrawingError
{
    fn from(e: DrawingAreaErrorKind<T>) -> Self {
        Self::Drawing {
            message: String::from(""),
            source: Box::new(e),
        }
    }
}

impl From<PolylabelError> for MyDrawingError {
    fn from(e: PolylabelError) -> Self {
        Self::Drawing {
            message: String::from("Failed to obtain the center point"),
            source: Box::new(e),
        }
    }
}

impl std::error::Error for MyDrawingError {}
