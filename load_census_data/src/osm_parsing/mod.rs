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
//! Used to load in building types and locations from an OSM file
use std::collections::HashMap;
use std::convert::TryFrom;

use geo_types::Point;
use log::{debug, info};
use osmpbf::DenseTagIter;

use crate::DataLoadingError;

mod convert;

pub enum BuildingTypes {
    Shop,
    School,
    Hospital,
    Household,
    WorkPlace,
    Unknown,
}

impl<'a> TryFrom<DenseTagIter<'a>> for BuildingTypes {
    type Error = ();

    fn try_from(value: DenseTagIter<'a>) -> Result<Self, Self::Error> {
        let tags: HashMap<&str, &str> = value.collect();
        if !tags.contains_key("building") && !tags.contains_key("abandoned:man_made") {
            return Err(());
        }
        if let Some(building) = tags.get("building") {
            match *building {
                "office" | "industrial" | "commercial" | "retail" | "warehouse" | "civic"
                | "public" => return Ok(BuildingTypes::WorkPlace),
                "house" | "detached" | "semidetached_house" | "farm" | "hut" | "static_caravan"
                | "cabin" | "apartments" | "terrace" | "residential" => {
                    return Ok(BuildingTypes::Household);
                }
                _ => (),
            }
        }
        if let Some(amenity) = tags.get("amenity") {
            match *amenity {
                "school" => return Ok(BuildingTypes::School),
                "hospital" => return Ok(BuildingTypes::Hospital),
                _ => (),
            }
        }
        if tags.contains_key("shop") {
            Ok(BuildingTypes::Shop)
        } else {
            Ok(BuildingTypes::Unknown)
        }
    }
}

/// Returns a hashmap of buildings located at which points
pub fn read_osm_data(
    filename: String,
) -> Result<HashMap<Point<u64>, BuildingTypes>, DataLoadingError> {
    use osmpbf::{Element, ElementReader};
    info!("Reading OSM data from file: {}", filename);
    let reader = ElementReader::from_path(filename)?;
    let mut nodes = 0_u64;
    let mut ways = 0_u64;
    let mut relations = 0_u64;
    // Increment the counter by one for each way.
    let mut buildings = HashMap::with_capacity(20000);
    let mut count = 0;
    debug!("Built reader, now loading Nodes...");
    reader
        .for_each(|element| {
            match element {
                Element::Node(node) => {
                    // TODO Maybe implement this?
                    panic!("Got a Node! ({:?})", node);
                }
                Element::DenseNode(node) => {
                    let visible = node.info().map(|info| info.visible()).unwrap_or(true);
                    if visible {
                        let position = convert::decimal_latitude_and_longitude_to_coordinates(
                            node.lat(),
                            node.lon(),
                        );
                        // TODO Do we need to keep decimal precision?
                        let position = (
                            (position.0 * 1.0).round() as u64,
                            (position.0 * 1.0).round() as u64,
                        );
                        let position = geo_types::Coordinate::from(position);
                        let position = geo_types::Point::from(position);
                        if let Ok(building) = BuildingTypes::try_from(node.tags()) {
                            buildings.insert(position, building);
                        }
                    }
                    nodes += 1;
                }
                Element::Way(_) => {
                    ways += 1;
                }
                Element::Relation(_) => {
                    relations += 1;
                }
            }
            count += 1;
            if count % 10000000 == 0 {
                debug!("At node count: {} million", count / 10000000);
            }
        })?;

    debug!(
        "Total Number of nodes: {} Ways: {}, Relations: {}",
        nodes, ways, relations
    );
    info!("Loaded {} buildings from {} nodes", buildings.len(), nodes);
    Ok(buildings)
}
