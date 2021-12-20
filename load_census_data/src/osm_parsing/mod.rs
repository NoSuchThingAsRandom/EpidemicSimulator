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
use crate::osm_parsing::vorini_generator::Vorinni;

mod convert;
mod vorini_generator;

pub const YORKSHIRE_AND_HUMBER_TOP_RIGHT: (u32, u32) = (470338, 519763);
pub const YORKSHIRE_AND_HUMBER_BOTTOM_LEFT: (u32, u32) = (363749, 383066);
pub const TOP_RIGHT_BOUNDARY: (isize, isize) = (YORKSHIRE_AND_HUMBER_TOP_RIGHT.0 as isize, YORKSHIRE_AND_HUMBER_TOP_RIGHT.1 as isize);
pub const BOTTOM_LEFT_BOUNDARY: (isize, isize) = (YORKSHIRE_AND_HUMBER_BOTTOM_LEFT.0 as isize, YORKSHIRE_AND_HUMBER_BOTTOM_LEFT.1 as isize);
//pub const GRID_SIZE: (usize, usize) = ((TOP_RIGHT_BOUNDARY.0 - BOTTOM_LEFT_BOUNDARY.0) as usize, (TOP_RIGHT_BOUNDARY.1 - BOTTOM_LEFT_BOUNDARY.1) as usize);
pub const GRID_SIZE: usize = 15000;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum RawBuildingTypes {
    Shop,
    School,
    Hospital,
    Household,
    WorkPlace,
    Unknown,
}

impl<'a> TryFrom<DenseTagIter<'a>> for RawBuildingTypes {
    type Error = ();

    fn try_from(value: DenseTagIter<'a>) -> Result<Self, Self::Error> {
        let tags: HashMap<&str, &str> = value.collect();
        if !tags.contains_key("building") && !tags.contains_key("abandoned:man_made") {
            return Err(());
        }
        if let Some(building) = tags.get("building") {
            match *building {
                "office" | "industrial" | "commercial" | "retail" | "warehouse" | "civic"
                | "public" => return Ok(RawBuildingTypes::WorkPlace),
                "house" | "detached" | "semidetached_house" | "farm" | "hut" | "static_caravan"
                | "cabin" | "apartments" | "terrace" | "residential" => {
                    return Ok(RawBuildingTypes::Household);
                }
                _ => (),
            }
        }
        if let Some(amenity) = tags.get("amenity") {
            match *amenity {
                "school" => return Ok(RawBuildingTypes::School),
                "hospital" => return Ok(RawBuildingTypes::Hospital),
                _ => (),
            }
        }
        if tags.contains_key("shop") {
            Ok(RawBuildingTypes::Shop)
        } else {
            Ok(RawBuildingTypes::Unknown)
        }
    }
}

#[derive(Clone)]
pub struct OSMRawBuildings {
    pub building_locations: HashMap<RawBuildingTypes, Vec<Point<isize>>>,
    pub school_lookup: Vorinni,
}

impl OSMRawBuildings {
    /// Returns a hashmap of buildings located at which points
    pub fn build_osm_data(filename: String) -> Result<OSMRawBuildings, DataLoadingError> {
        info!("Building OSM Data...");
        debug!("Starting to read data from file");
        let building_locations = OSMRawBuildings::read_buildings_from_osm(filename)?;
        debug!("Starting to generate school vorinni map");
        let schools = building_locations.get(&RawBuildingTypes::School).expect("No school buildings exist in the OSM File!");
        let school_lookup = Vorinni::new(GRID_SIZE, schools.iter().map(|p| (p.0.x as usize / 10, p.0.y as usize / 10)).collect());
        debug!("Finished building OSM data");
        Ok(OSMRawBuildings { building_locations, school_lookup })
    }
    fn read_buildings_from_osm(filename: String) -> Result<HashMap<RawBuildingTypes, Vec<Point<isize>>>, DataLoadingError> {
        use osmpbf::{Element, ElementReader};
        info!("Reading OSM data from file: {}", filename);
        let reader = ElementReader::from_path(filename)?;
        let mut nodes = 0_u64;
        let mut ways = 0_u64;
        let mut relations = 0_u64;
        // Increment the counter by one for each way.
        let mut buildings: HashMap<RawBuildingTypes, Vec<Point<isize>>> = HashMap::with_capacity(20000);
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
                            if BOTTOM_LEFT_BOUNDARY.0 < position.0 && position.1 < TOP_RIGHT_BOUNDARY.1 {
                                let position = geo_types::Coordinate::from(position);
                                let position = geo_types::Point::from(position);
                                if let Ok(building) = RawBuildingTypes::try_from(node.tags()) {
                                    let record = buildings.entry(building).or_default();
                                    record.push(position);
                                }
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
        info!("Loaded {} buildings from {} nodes", buildings.iter().map(|(_,b)|b.len()).sum::<usize>(), nodes);
        Ok(buildings)
    }
}