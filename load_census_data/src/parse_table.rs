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

//! Module used for reading and parsing Census Table

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::Write;

use serde_json::Value;

use crate::parsing_error::{DataLoadingError, ParseErrorType};

pub struct TableInfo {
    id: String,
    coded_name: String,
    source: String,
    metadata: String,
    keywords: Vec<String>,
    geo_level: Vec<String>,
}

impl TryFrom<HashMap<String, String>> for TableInfo {
    type Error = DataLoadingError;

    fn try_from(value: HashMap<String, String>) -> Result<Self, Self::Error> {
        let id = value
            .get("id")
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: String::from("Table info"),
                    key: "id".to_string(),
                },
            })?
            .to_string();
        let source = value
            .get("contenttype/sources")
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: String::from("Table info"),
                    key: "contenttype/sources".to_string(),
                },
            })?
            .to_string();
        let coded_name = value
            .get("Mnemonic")
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: String::from("Table info"),
                    key: "Mnemonic".to_string(),
                },
            })?
            .to_string();
        let metadata = value
            .get("MetadataText0")
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: String::from("Table info"),
                    key: "MetadataText0".to_string(),
                },
            })?
            .to_string();
        let keywords = value
            .get("Keywords")
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: String::from("Table info"),
                    key: "Keywords".to_string(),
                },
            })?
            .split(',')
            .map(|s| s.to_string())
            .collect();
        let geo_level = value
            .get("contenttype/geoglevel")
            .ok_or_else(|| DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: String::from("Table info"),
                    key: "contenttype/geoglevel".to_string(),
                },
            })?
            .split(',')
            .map(|s| s.to_string())
            .collect();

        Ok(TableInfo {
            id,
            coded_name,
            source,
            metadata,
            keywords,
            geo_level,
        })
    }
}

impl Display for TableInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ID: {}, Coded Name: {}, Source: {}, Keywords: ({:?}), Geo Levels: {:?}",
            self.id, self.coded_name, self.source, self.keywords, self.geo_level
        )
    }
}

impl Debug for TableInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub fn parse_jsontable_list(json: Value) -> Result<Vec<TableInfo>, DataLoadingError> {
    let mut tables = Vec::new();

    let structure = extract_value_from_json(&json, "structure")?;
    let key_families_object = extract_value_from_json(structure, "keyfamilies")?;
    let keys = extract_array_from_json(key_families_object, "keyfamily")?;
    for key in keys {
        let annotations_object = extract_value_from_json(key, "annotations")?;
        let annotations_array = extract_array_from_json(annotations_object, "annotation")?;
        let mut annotation_properties = HashMap::with_capacity(annotations_array.len());
        let id = extract_string_from_json(key, "id")?;
        annotation_properties.insert("id".to_string(), id);
        for annotation in annotations_array {
            let title = extract_string_from_json(annotation, "annotationtitle")?;
            let text = extract_string_from_json(annotation, "annotationtext")?;
            annotation_properties.insert(title, text);
        }
        let table_info = TableInfo::try_from(annotation_properties);
        if let Ok(table_info) = table_info {
            if table_info.geo_level.contains(&"oa".to_string()) {
                println!("{}", table_info);
                tables.push(table_info);
            }
        }
    }
    Ok(tables)
    //println!("JSON Data: {:?}", json);
    //println!("{:?}", data);
}

pub async fn read_json(filename: String) -> Result<Value, String> {
    let file = File::open(filename).map_err(|e| format!("{:?}", e))?;
    let json: Value = serde_json::from_reader(file).map_err(|e| format!("{:?}", e))?;
    Ok(json)
}

pub fn write_file(filename: String, data: String) -> Result<(), String> {
    let mut file = File::create(filename).map_err(|e| format!("{:?}", e))?;
    file.write_all(data.as_bytes())
        .map_err(|e| format!("{:?}", e))?;
    file.flush().unwrap();
    Ok(())
}

fn extract_value_from_json<'a>(
    object: &'a Value,
    name: &str,
) -> Result<&'a Value, DataLoadingError> {
    let object = object
        .get(name)
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::MissingKey {
                context: "Extracting value from JSON".to_string(),
                key: name.to_string(),
            },
        })?;
    Ok(object)
}

fn extract_string_from_json(object: &Value, name: &str) -> Result<String, DataLoadingError> {
    let object = object
        .get(name)
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::MissingKey {
                context: "Extracting string from JSON".to_string(),
                key: name.to_string(),
            },
        })?;
    if let Value::Number(n) = object {
        return Ok(n.to_string());
    }
    let object = object
        .as_str()
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::InvalidDataType {
                value: Some(object.to_string()),
                expected_type: "String".to_string(),
            },
        })?;
    Ok(object.to_string())
}

fn extract_array_from_json<'a>(
    object: &'a Value,
    name: &str,
) -> Result<&'a Vec<Value>, DataLoadingError> {
    let object = object
        .get(name)
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::MissingKey {
                context: "Extracting array from JSON".to_string(),
                key: name.to_string(),
            },
        })?;
    let object = object
        .as_array()
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::InvalidDataType {
                value: Some(object.to_string()),
                expected_type: "Array".to_string(),
            },
        })?;
    Ok(object)
}

fn extract_map_from_json<'a>(
    object: &'a Value,
    name: &str,
) -> Result<&'a serde_json::Map<String, Value>, DataLoadingError> {
    let object = object
        .get(name)
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::MissingKey {
                context: "Extracting map from JSON".to_string(),
                key: name.to_string(),
            },
        })?;
    let object = object
        .as_object()
        .ok_or_else(|| DataLoadingError::ValueParsingError {
            source: ParseErrorType::InvalidDataType {
                value: Some(object.to_string()),
                expected_type: "Map".to_string(),
            },
        })?;
    Ok(object)
}
