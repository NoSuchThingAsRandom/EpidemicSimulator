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
use std::num::{ParseFloatError, ParseIntError};

#[derive(Debug)]
pub enum SerdeErrors {
    Json { source: serde_json::Error },
    Plain { source: serde_plain::Error },
}

#[derive(Debug)]
pub enum ParseErrorType {
    /// Cannot parse the value into a float
    Int {
        source: ParseIntError,
    },
    /// Cannot parse the value into a float
    Float {
        source: ParseFloatError,
    },
    /// The value is not the expected data type
    InvalidDataType {
        value: Option<String>,
        expected_type: String,
    },
    /// The collection is expected to contain one or more values
    IsEmpty {
        message: String,
    },
    /// Two values should be equal, but are not
    Mismatching {
        message: String,
        value_1: String,
        value_2: String,
    },
    /// This occurs when the value corresponding to a key in a map is not there
    MissingKey {
        context: String,
        key: String,
    },
    MathError {
        context: String,
    },
    OutOfBounds {
        context: String,
        max_size: String,
        actual_size: String,
    },
}

impl Display for ParseErrorType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseErrorType::Int { ref source } => {
                write!(f, "{}", source)
            }
            ParseErrorType::Float { ref source } => {
                write!(f, "{}", source)
            }
            ParseErrorType::InvalidDataType {
                value,
                expected_type,
            } => {
                write!(
                    f,
                    "Invalid Data Type: Expected {} got {:?}",
                    expected_type, value
                )
            }
            ParseErrorType::IsEmpty { message } => {
                write!(f, "Object is empty: {}", message)
            }
            ParseErrorType::Mismatching {
                message,
                value_1,
                value_2,
            } => {
                write!(
                    f,
                    "Values ({}) and  ({}) should be matching. {}",
                    value_1, value_2, message
                )
            }
            ParseErrorType::MissingKey { context, key } => {
                write!(f, "Missing Key! Context: {}, \tKey: {}", context, key)
            }
            ParseErrorType::MathError { context } => {
                write!(f, "Math error! Context: {}", context)
            }
            ParseErrorType::OutOfBounds {
                context,
                max_size,
                actual_size,
            } => {
                write!(
                    f,
                    "Out Of Bounds Error: {},\tMax Size: {},\tActual Size: {}",
                    context, max_size, actual_size
                )
            }
        }
    }
}

impl std::error::Error for ParseErrorType {}

pub enum DataLoadingError {
    ShapeFileError {
        source: shapefile::Error,
    },
    /// An error occurs fetching via reqwest
    NetworkError {
        source: reqwest::Error,
    },
    /// An error occurs parsing data with Serde
    SerdeParseError {
        source: SerdeErrors,
    },
    /// An error occurs trying to parse or convert a Value
    ValueParsingError {
        source: ParseErrorType,
    },
    /// An error occurs reading from disk
    IOError {
        source: Box<dyn std::error::Error + Send + Sync>,
        context: String,
    },
    Misc {
        source: String,
    },
}

impl From<reqwest::Error> for DataLoadingError {
    fn from(err: reqwest::Error) -> Self {
        DataLoadingError::NetworkError { source: err }
    }
}

impl From<serde_json::Error> for DataLoadingError {
    fn from(err: serde_json::Error) -> Self {
        DataLoadingError::SerdeParseError {
            source: SerdeErrors::Json { source: err },
        }
    }
}

impl From<serde_plain::Error> for DataLoadingError {
    fn from(err: serde_plain::Error) -> Self {
        DataLoadingError::SerdeParseError {
            source: SerdeErrors::Plain { source: err },
        }
    }
}

impl From<ParseIntError> for DataLoadingError {
    fn from(err: ParseIntError) -> Self {
        DataLoadingError::ValueParsingError {
            source: ParseErrorType::Int { source: err },
        }
    }
}

impl From<ParseFloatError> for DataLoadingError {
    fn from(err: ParseFloatError) -> Self {
        DataLoadingError::ValueParsingError {
            source: ParseErrorType::Float { source: err },
        }
    }
}

impl From<String> for DataLoadingError {
    fn from(err: String) -> Self {
        DataLoadingError::Misc { source: err }
    }
}

impl From<shapefile::Error> for DataLoadingError {
    fn from(e: shapefile::Error) -> Self {
        DataLoadingError::ShapeFileError { source: e }
    }
}

impl Display for DataLoadingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DataLoadingError::NetworkError { source } => {
                write!(
                    f,
                    "\nAn error occurred loading Census Data\n\tType: NetworkError\n\tSource: {} ",
                    source
                )
            }
            DataLoadingError::SerdeParseError { source } => {
                write!(
                    f,
                    "\nAn error occurred loading Census Data\n\tType: SerdeError\n\tSource: {:#?} ",
                    source
                )
            }
            DataLoadingError::ValueParsingError { source } => {
                write!(
                    f,
                    "\nAn error occurred loading Census Data\n\tType: ParsingError\n\tSource: {} ",
                    source
                )
            }
            DataLoadingError::IOError { source, context } => {
                write!(
                    f,
                    "\nAn error occurred loading Census Data\n\tType: IoError\n\tSource: {}\n\tContext: {}",
                    source,
                    context
                )
            }
            DataLoadingError::Misc { source } => {
                write!(f, "{}", source)
            }
            DataLoadingError::ShapeFileError { source } => {
                write!(f, "\nAn error occurred loading a shapefile: {}", source)
            }
        }
    }
}

impl Debug for DataLoadingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for DataLoadingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            DataLoadingError::NetworkError { ref source } => Some(source),
            DataLoadingError::SerdeParseError { ref source } => match source {
                SerdeErrors::Json { ref source } => Some(source),
                SerdeErrors::Plain { ref source } => Some(source),
            },
            DataLoadingError::ValueParsingError { ref source } => Some(source),
            DataLoadingError::IOError { ref source, .. } => source.source(),
            DataLoadingError::Misc { .. } => None,
            DataLoadingError::ShapeFileError { ref source } => Some(source),
        }
    }
}
