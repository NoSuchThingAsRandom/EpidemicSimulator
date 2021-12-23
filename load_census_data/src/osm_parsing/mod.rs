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
use std::fs::File;
use std::io::{Read, Write};

use geo_types::Point;
use log::{debug, info};
use osmpbf::{DenseNode, DenseTagIter};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

use crate::DataLoadingError;
use crate::osm_parsing::draw_vorinni::{draw_osm_buildings_polygons, draw_voronoi_polygons};
use crate::voronoi_generator::{Scaling, Voronoi};

pub mod convert;
pub mod draw_vorinni;

// From guesstimating on: https://maps.nls.uk/geo/explore/#zoom=19&lat=53.94849&lon=-1.03067&layers=170&b=1&marker=53.948300,-1.030701
pub const YORKSHIRE_AND_HUMBER_TOP_RIGHT: (u32, u32) = (4500000, 400000);
pub const YORKSHIRE_AND_HUMBER_BOTTOM_LEFT: (u32, u32) = (3500000, 100000);
pub const TOP_RIGHT_BOUNDARY: (isize, isize) = (YORKSHIRE_AND_HUMBER_TOP_RIGHT.0 as isize, YORKSHIRE_AND_HUMBER_TOP_RIGHT.1 as isize);
pub const BOTTOM_LEFT_BOUNDARY: (isize, isize) = (YORKSHIRE_AND_HUMBER_BOTTOM_LEFT.0 as isize, YORKSHIRE_AND_HUMBER_BOTTOM_LEFT.1 as isize);
//pub const GRID_SIZE: (usize, usize) = ((TOP_RIGHT_BOUNDARY.0 - BOTTOM_LEFT_BOUNDARY.0) as usize, (TOP_RIGHT_BOUNDARY.1 - BOTTOM_LEFT_BOUNDARY.1) as usize);


// Use 200 units per building
pub const GRID_SIZE: usize = 100;
const DUMP_TO_FILE: bool = false;
const DRAW_VORONOI_DIAGRAMS: bool = false;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
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


pub struct OSMRawBuildings {
    pub building_locations: HashMap<RawBuildingTypes, Vec<Point<isize>>>,
    pub building_vorinnis: HashMap<RawBuildingTypes, Voronoi>,
}

impl OSMRawBuildings {
    /// Returns a hashmap of buildings located at which points
    pub fn build_osm_data(filename: String) -> Result<OSMRawBuildings, DataLoadingError> {
        info!("Building OSM Data...");
        debug!("Starting to read data from file");
        let building_locations = if DUMP_TO_FILE {
            debug!("Parsing data from raw OSM file");
            let building_locations = OSMRawBuildings::read_buildings_from_osm(filename)?;
            let mut file = File::create("osm_dump.json").unwrap();

            file.write_all(&serde_json::to_vec(&building_locations).unwrap()).unwrap();
            file.flush().unwrap();
            debug!("Completed and saved parsing data");
            building_locations
        } else {
            debug!("Reading cached parsing data");
            let mut file = File::open("osm_dump.json").unwrap();
            let mut data = String::with_capacity(1000);
            file.read_to_string(&mut data).unwrap();
            serde_json::from_str(&data).unwrap()
        };
        debug!("Loaded OSM data");


        let mut building_vorinnis = HashMap::new();
        for (building_type, locations) in &building_locations {
            info!("Building voronoi diagram for {:?} with {} buildings",building_type,locations.len());
            //GRID_SIZE * locations.len()
            let lookup = Voronoi::new(500000, locations.iter().map(|p| (p.0.x as usize, p.0.y as usize)).collect(), Scaling::yorkshire_national_grid())?;
            let f = lookup.polygons.polygons.iter().map(|(p, i)| p.clone()).collect();
            draw_voronoi_polygons(format!("images/{:?}Vorinni.png", building_type), &f, 20000);
            // TODO Add to hashmap when memory isn't an issue
            building_vorinnis.insert(*building_type, lookup);
        }
        let data = OSMRawBuildings { building_locations, building_vorinnis };
        if DRAW_VORONOI_DIAGRAMS {
            debug!("Starting drawing");
            for (k, p) in data.building_vorinnis.iter() {//keys().clone().for_each(|k| {
                let f = p.polygons.polygons.iter().map(|(p, i)| p.clone()).collect();
                draw_voronoi_polygons(format!("images/{:?}Vorinni.png", k), &f, 20000);
                //   });
            }
        }
        debug!("Finished building OSM data");
        Ok(data)
    }
    /// Extract the building type, and it's location from an OSM Node
    ///
    /// Returns None, if outside the boundary, not visible or unsupported node type
    fn parse_node(node: DenseNode) -> Option<(RawBuildingTypes, geo_types::Point<isize>)> {
        let visible = node.info().map(|info| info.visible()).unwrap_or(true);
        if visible {
            let position = convert::decimal_latitude_and_longitude_to_coordinates(
                node.lat(),
                node.lon(),
            );
            if let Some(position) = position {
                //println!("{} {}",position.0,position.1);
                if BOTTOM_LEFT_BOUNDARY.0 < position.0 && position.0 < TOP_RIGHT_BOUNDARY.0 && BOTTOM_LEFT_BOUNDARY.1 < position.1 && position.1 < TOP_RIGHT_BOUNDARY.1 {//3000000 < position.0 && position.0 < 6000000 && 3000000 < position.1 && position.1 < 6000000 {
                    let position = geo_types::Coordinate::from(position);
                    let position = geo_types::Point::from(position);
                    if let Ok(building) = RawBuildingTypes::try_from(node.tags()) {
                        return Some((building, position));
                    }
                }
            }
        }
        None
    }
    fn read_buildings_from_osm(filename: String) -> Result<HashMap<RawBuildingTypes, Vec<Point<isize>>>, DataLoadingError> {
        use osmpbf::{Element, ElementReader};
        info!("Reading OSM data from file: {}", filename);
        let reader = ElementReader::from_path(filename)?;
        // Read the OSM data, only select buildings, and build a hashmap of building types, with a list of locations
        debug!("Built reader, now loading Nodes...");
        let buildings: HashMap<RawBuildingTypes, Vec<Point<isize>>> = reader.par_map_reduce(|element| {
            match element {
                Element::DenseNode(node) => {
                    // Extract the building type and location from the node
                    // Then if a valid building time,instantiate a new Hashmap to be merged
                    OSMRawBuildings::parse_node(node).map(|data| HashMap::from([(data.0, vec![data.1])]))
                }
                //Discard all other OSM elements (Like roads)
                _ => None
            }
        }, || None, |a, b| {
            // Fold the multiple hashmaps into a singular hashmap
            match (a, b) {
                (Some(mut a), Some(mut b)) => {
                    b.drain().for_each(|(k, v)| {
                        let entry = a.entry(k).or_insert_with(|| Vec::with_capacity(v.len()));
                        entry.extend(v);
                    });
                    a.extend(b);
                    Some(a)
                }
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None
            }
        })?.expect("No buildings loaded from osm data");

        // Count the number of unique buildings
        info!("Loaded {} buildings from OSM data", buildings.iter().map(|(_,b)|b.len()).sum::<usize>());

        Ok(buildings)
    }
}