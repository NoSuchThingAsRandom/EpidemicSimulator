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

pub enum Error {
    Default {
        message: String,
    },
    Simulation {
        message: String,
    },
    DrawingError {
        source: Box<dyn std::error::Error + Send + Sync>,
        context: String,
    },
    InitializationError {
        message: String
    },
    OptionRetrievalFailure {
        message: String,
        key: String,
    },
}

impl Error {
    pub fn new_simulation_error(message: String) -> Error {
        Error::Simulation { message }
    }

    pub fn from_option<T: Display, U>(value: Option<U>, key: T, message: String) -> Result<U, Error> {
        if let Some(value) = value {
            Ok(value)
        } else {
            Err(Error::OptionRetrievalFailure { message, key: key.to_string() })
        }
    }
}

impl Default for Error {
    fn default() -> Self {
        Error::Default {
            message: String::from("An error occurred!"),
        }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Default { message } => {
                write!(f, "Error: {}", message)
            }
            Error::DrawingError { source, context } => {
                write!(f, "Error: {}\n{}", context, source)
            }
            Error::Simulation { message } => {
                write!(f, "Simulation Error Occurred: {}", message)
            }
            Error::OptionRetrievalFailure { message, key } => {
                write!(f, "Failed to retrieve value with key ({}), context: {}", key, message)
            }
            Error::InitializationError { message } => {
                write!(f, "{} has not been Initialized", message)
            }
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self)
    }
}

impl std::error::Error for Error {}

