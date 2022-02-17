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

use std::fmt::{Debug, Display, Formatter};

use osm_data::error::OSMError;

pub enum SimError {
    OSMError {
        source: OSMError,
    },
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
        message: String,
    },
    MissingCitizen {
        citizen_id: String,
    },
    OptionRetrievalFailure {
        message: String,
        key: String,
    },
    Error {
        context: String
    },
}

impl SimError {
    pub fn new_simulation_error(message: String) -> SimError {
        SimError::Simulation { message }
    }

    pub fn from_option<T: Display, U>(
        value: Option<U>,
        key: T,
        message: String,
    ) -> Result<U, SimError> {
        if let Some(value) = value {
            Ok(value)
        } else {
            Err(SimError::OptionRetrievalFailure {
                message,
                key: key.to_string(),
            })
        }
    }
}

impl Default for SimError {
    fn default() -> Self {
        SimError::Default {
            message: String::from("An error occurred!"),
        }
    }
}

impl Debug for SimError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::OSMError { source } => {
                write!(f, "Sim Error: {}", source)
            }

            SimError::Default { message } => {
                write!(f, "Error: {}", message)
            }
            SimError::DrawingError { source, context } => {
                write!(f, "Error: {}\n{}", context, source)
            }
            SimError::Simulation { message } => {
                write!(f, "Simulation Error Occurred: {}", message)
            }
            SimError::OptionRetrievalFailure { message, key } => {
                write!(
                    f,
                    "Failed to retrieve value with key ({}), context: {}",
                    key, message
                )
            }
            SimError::InitializationError { message } => {
                write!(f, "{} has not been Initialized", message)
            }
            SimError::Error { context } => {
                write!(f, "An error occurred: {}", context)
            }
            SimError::MissingCitizen { citizen_id } => {
                write!(f, "Citizen ({}) doesn't exist! ", citizen_id)
            }
        }
    }
}

impl Display for SimError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {:?}", self)
    }
}

impl std::error::Error for SimError {}

impl From<osm_data::error::OSMError> for SimError {
    fn from(e: OSMError) -> Self {
        SimError::OSMError { source: e }
    }
}