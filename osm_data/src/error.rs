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

pub enum OSMError {
    OSMError {
        source: osmpbf::Error,
    },
    /// An error occurs reading from disk
    IOError {
        source: Box<dyn std::error::Error + Send + Sync>,
        context: String,
    },
    Misc {
        source: String,
    },
    ValueParsingError {
        source: String,
    },
    OutOfBounds {
        context: String,
        max_size: String,
        actual_size: String,
    },
    /// This occurs when the value corresponding to a key in a map is not there
    MissingKey {
        context: String,
        key: String,
    },
    /// The collection is expected to contain one or more values
    IsEmpty {
        context: String,
    },
    ShapeFileError {
        source: shapefile::Error,
    },
}

impl From<osmpbf::Error> for OSMError {
    fn from(err: osmpbf::Error) -> Self {
        OSMError::OSMError { source: err }
    }
}

impl From<String> for OSMError {
    fn from(err: String) -> Self {
        OSMError::Misc { source: err }
    }
}

impl From<shapefile::Error> for OSMError {
    fn from(e: shapefile::Error) -> Self {
        OSMError::ShapeFileError { source: e }
    }
}

impl Display for OSMError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OSMError::ShapeFileError { source } => {
                write!(f, "\nAn error occurred loading a shapefile: {}", source)
            }
            OSMError::OSMError { source } => {
                write!(
                    f,
                    "\nAn error occurred loading OSM data\n:\tType: OSMError\n\tSource: {}",
                    source
                )
            }
            OSMError::IOError { source, context } => {
                write!(f, "\nAn error occurred loading OSM data\n:\tType: IOError\n\tSource: {}\n\tContext: {}", source, context)
            }
            OSMError::Misc { source } => {
                write!(
                    f,
                    "\nAn error occurred loading OSM data\n:\tType: MiscError\n\tSource: {}",
                    source
                )
            }
            OSMError::ValueParsingError { source } => {
                write!(f, "\nAn error occurred loading OSM data\n:\tType: ValueParsingError\n\tSource: {}", source)
            }
            OSMError::OutOfBounds {
                context,
                max_size,
                actual_size,
            } => {
                write!(f, "\nAn error occurred loading OSM data\n tOutOfBounds Error: {},\tMax Size: {},\tActual Size: {}", context, max_size, actual_size)
            }
            OSMError::MissingKey { context, key } => {
                write!(f, "\nAn error occurred loading OSM data\n:\tType: MissingKeyError\n\tContext: {}\n\tKey: {}", context, key)
            }
            OSMError::IsEmpty { context } => {
                write!(
                    f,
                    "\nAn error occurred loading OSM data\n:\tType: Object is Empty\n\tContext: {}",
                    context
                )
            }
        }
    }
}

impl Debug for OSMError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for OSMError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            OSMError::OSMError { ref source } => Some(source),
            OSMError::IOError { ref source, .. } => source.source(),
            OSMError::Misc { .. } => None,
            OSMError::ValueParsingError { .. } => None,
            OSMError::OutOfBounds { .. } => None,
            OSMError::MissingKey { .. } => None,
            OSMError::IsEmpty { .. } => None,
            OSMError::ShapeFileError { ref source } => Some(source),
        }
    }
}
