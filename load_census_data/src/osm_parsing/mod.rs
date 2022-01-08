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
//! Used to load in building types and locations from an OSM file
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::convert::TryFrom;
use std::fs::File;
use std::io::{Read, Write};

use geo::area::Area;
use geo::centroid::Centroid;
use geo_types::{CoordFloat, CoordNum, Point, Polygon};
use log::{debug, error, info, warn};
use osmpbf::{DenseNode, DenseTagIter, TagIter};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::DataLoadingError;
use crate::osm_parsing::draw_vorinni::draw_voronoi_polygons;
use crate::voronoi_generator::{Scaling, Voronoi};

pub mod convert;
pub mod draw_vorinni;

// From guesstimating on: https://maps.nls.uk/geo/explore/#zoom=19&lat=53.94849&lon=-1.03067&layers=170&b=1&marker=53.948300,-1.030701
pub const YORKSHIRE_AND_HUMBER_TOP_RIGHT: (u32, u32) = (450000, 400000);
pub const YORKSHIRE_AND_HUMBER_BOTTOM_LEFT: (u32, u32) = (3500000, 100000);
pub const TOP_RIGHT_BOUNDARY: (isize, isize) = (
    YORKSHIRE_AND_HUMBER_TOP_RIGHT.0 as isize,
    YORKSHIRE_AND_HUMBER_TOP_RIGHT.1 as isize,
);
pub const BOTTOM_LEFT_BOUNDARY: (isize, isize) = (
    YORKSHIRE_AND_HUMBER_BOTTOM_LEFT.0 as isize,
    YORKSHIRE_AND_HUMBER_BOTTOM_LEFT.1 as isize,
);

/// The size of grids to use
pub const GRID_SIZE: usize = 50000;
const DUMP_TO_FILE: bool = false;
const DRAW_VORONOI_DIAGRAMS: bool = false;

fn convert_points_to_floats<T: CoordNum, U: CoordFloat>(
    points: &Vec<(geo_types::Coordinate<T>)>,
) -> Vec<(geo_types::Coordinate<U>)> {
    points
        .iter()
        .map(|p| {
            let p: geo_types::Coordinate<U> = (
                U::from(p.x).expect(
                    format!("Failed to convert X coordinate ({:?}) to float", p.x).as_str(),
                ),
                U::from(p.y).expect(
                    format!("Failed to convert Y coordinate ({:?}) to float", p.y).as_str(),
                ),
            )
                .into();
            return p;
        })
        .collect()
}

pub fn convert_polygon_to_float<T: CoordNum, U: CoordFloat>(
    polygon: &geo_types::Polygon<T>,
) -> geo_types::Polygon<U> {
    geo_types::Polygon::new(
        convert_points_to_floats(&polygon.exterior().0).into(),
        polygon
            .interiors()
            .iter()
            .map(|interior| convert_points_to_floats(&interior.0).into())
            .collect(),
    )
}

/// The type of buildings that are supported from the OSM Tag lists
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum TagClassifiedBuilding {
    Shop,
    School,
    Hospital,
    Household,
    WorkPlace,
    /// Not a building
    Unknown,
}

impl<'a> From<HashMap<&'a str, &'a str>> for TagClassifiedBuilding {
    fn from(tags: HashMap<&'a str, &'a str>) -> Self {
        if let Some(amenity) = tags.get("amenity") {
            match *amenity {
                "school" => return TagClassifiedBuilding::School,
                "hospital" => return TagClassifiedBuilding::Hospital,
                _ => (),
            }
        }
        if tags.contains_key("shop") {
            return TagClassifiedBuilding::Shop;
        }
        if let Some(building) = tags.get("building") {
            return match *building {
                "office" | "industrial" | "commercial" | "retail" | "warehouse" | "civic"
                | "public" => TagClassifiedBuilding::WorkPlace,
                "house" | "detached" | "semidetached_house" | "farm" | "hut" | "static_caravan"
                | "cabin" | "apartments" | "terrace" | "residential" => {
                    TagClassifiedBuilding::Household
                }
                "school" => TagClassifiedBuilding::School,
                "hospital" => TagClassifiedBuilding::Hospital,
                // Unknown buildings can be workplaces?
                _ => TagClassifiedBuilding::WorkPlace,
            };
        }
        TagClassifiedBuilding::Unknown
    }
}

impl<'a> From<TagIter<'a>> for TagClassifiedBuilding {
    fn from(tags: TagIter<'a>) -> Self {
        TagClassifiedBuilding::from(tags.collect::<HashMap<&'a str, &'a str>>())
    }
}

impl<'a> From<DenseTagIter<'a>> for TagClassifiedBuilding {
    fn from(tags: DenseTagIter<'a>) -> Self {
        TagClassifiedBuilding::from(tags.collect::<HashMap<&'a str, &'a str>>())
    }
}

/// A wrapper for an Open Street Map Way
struct RawOSMWay {
    id: i64,
    classification: TagClassifiedBuilding,
    /// The set of [`RawOSMNode`] that make up this `OSM Way`
    node_ids: Vec<i64>,
}

/// A wrapper for an Open Street Map Node
struct RawOSMNode {
    id: i64,
    classification: TagClassifiedBuilding,
    location: Point<i32>,
}

/// To avoid storing multiple copies of a buildings outline (as one OSM building, can exist multiple times),
/// We create a global hashmap, and use this Struct as an ID
///
/// This Struct exists, solely so we don't get confused which uuid is which
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct BuildingBoundaryID {
    id: uuid::Uuid,
}

/// This is a representation of an OSM building
///
/// It has a type, given by the OSM Tags, as well as a center point, and an approximate size
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RawBuilding {
    classification: TagClassifiedBuilding,
    /// The approximate center of this building
    center: Point<i32>,
    /// The ID in the global hashmap, containing the outline of this buildings
    boundary_id: BuildingBoundaryID,
    /// The floor space of this building, approximated from the polygon
    size: i32,
}

impl RawBuilding {
    /// Generates a new RawBuilding from the OSM Node and approximates the center position and floor space ([`size`])
    pub fn new(
        classification: TagClassifiedBuilding,
        boundary: &Polygon<i32>,
        boundary_id: BuildingBoundaryID,
    ) -> Option<RawBuilding> {
        let float_boundary: geo_types::Polygon<f64> = convert_polygon_to_float(boundary);
        // Can't find center with integer points
        let size = float_boundary.unsigned_area().round() as i32;
        Some(RawBuilding {
            classification,
            center: float_boundary
                .centroid()
                .map(|p| geo_types::Point::from((p.x().round() as i32, (p.y().round()) as i32)))?,
            boundary_id,
            size,
        })
    }
    pub fn center(&self) -> Point<i32> {
        self.center
    }
    pub fn size(&self) -> i32 {
        self.size
    }
    pub fn classification(&self) -> TagClassifiedBuilding {
        self.classification
    }
    pub fn boundary_id(&self) -> BuildingBoundaryID {
        self.boundary_id
    }
}

impl<'a> TryFrom<DenseNode<'a>> for RawOSMNode {
    type Error = ();

    fn try_from(node: DenseNode<'a>) -> Result<Self, Self::Error> {
        let visible = node.info().map(|info| info.visible()).unwrap_or(true);
        if visible {
            let position = convert::decimal_latitude_and_longitude_to_northing_and_eastings(
                node.lat(),
                node.lon(),
            );
            //let position = geo_types::Coordinate::from(position);
            let position: Point<i32> = position.into(); //geo_types::Point::from(position);
            return Ok(RawOSMNode {
                id: node.id,
                classification: TagClassifiedBuilding::from(node.tags()),
                location: position,
            });
        }
        Err(())
    }
}

/// Merges two optional iterators
///
/// DO NOT USE WITH HASHMAPS/BTREEMAPS AS DUPLICATE KEYS ARE REMOVED
///
/// # Example
/// ```
///     let mut a=HashMap::from([(1,2),(2,3)]);
///     let mut b=HashMap::from([(1,2),(2,4),(3,2)]);
///     println!("{:?}",a); //{1: 2, 2: 3}
///     a.extend(b);
///     println!("{:?}",a); // {2: 4, 1: 2, 3: 2}
/// ```
pub fn merge_iterators<T, U: Extend<T> + IntoIterator<Item=T>>(
    a: Option<U>,
    b: Option<U>,
) -> Option<U> {
    match (a, b) {
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// The container for the procesed OSM Data
#[derive(Debug)]
pub struct OSMRawBuildings {
    /// A hashmap for referencing a buildings outline/boundaries
    ///
    /// Stored in a Hashmap to attempt to reduce copies of the same Polygons, as multiple [`RawBuilding`]s can share the same
    pub building_boundaries: HashMap<BuildingBoundaryID, Polygon<i32>>,
    pub building_locations: HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
    pub building_voronois: HashMap<TagClassifiedBuilding, Voronoi>,
}

impl OSMRawBuildings {
    fn read_cached_osm_data(
        cache_filename: String,
    ) -> Result<
        (
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
            HashMap<BuildingBoundaryID, Polygon<i32>>,
        ),
        DataLoadingError,
    > {
        debug!("Reading cached parsing data");
        let mut file =
            File::open(cache_filename.to_string()).map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: format!("File '{}' doesn't exist!", cache_filename),
            })?;
        let mut data = String::with_capacity(1000);
        file.read_to_string(&mut data)
            .map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: "Failed to read data!".to_string(),
            })?;
        serde_json::from_str(&data).map_err(|e| DataLoadingError::IOError {
            source: Box::new(e),
            context: "Failed to parse OSM cached data with serde!".to_string(),
        })
    }
    fn load_and_write_cache(
        raw_filename: String,
        cache_filename: String,
    ) -> Result<
        (
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
            HashMap<BuildingBoundaryID, Polygon<i32>>,
        ),
        DataLoadingError,
    > {
        debug!("Parsing data from raw OSM file");
        let building_locations = OSMRawBuildings::read_buildings_from_osm(raw_filename)?;
        let mut file =
            File::create(cache_filename.to_string()).map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: format!("Failed to create file '{}'", cache_filename),
            })?;

        file.write_all(&serde_json::to_vec(&building_locations).map_err(|e| {
            DataLoadingError::IOError {
                source: Box::new(e),
                context: "Failed to serialize OSM data with serde!".to_string(),
            }
        })?)
            .map_err(|e| DataLoadingError::IOError {
                source: Box::new(e),
                context: "Failed to write serde data to file!".to_string(),
            })?;
        file.flush().map_err(|e| DataLoadingError::IOError {
            source: Box::new(e),
            context: "Failed to flush data to file!".to_string(),
        })?;
        debug!("Completed and saved parsing data");
        Ok(building_locations)
    }
    /// Returns a hashmap of buildings located at which points
    ///
    /// # Parameters
    /// * `filename` - The file to read osm map data from
    /// * `cache_filename` - The file to store parsed osm data
    /// * `use_cache` - If true, stores the results of loading the OSM file to the `cache_filename` file, otherwise skips parsing the OSM file, and uses the cache instead
    /// * `visualise_building_boundaries` - If true, generates images representing the Voronoi diagrams for each building type
    pub fn build_osm_data(
        filename: String,
        cache_filename: String,
        use_cache: bool,
        visualise_building_boundaries: bool,
    ) -> Result<OSMRawBuildings, DataLoadingError> {
        info!("Building OSM Data...");
        debug!("Starting to read data from file");
        // If using cache, attempt to load data from cache
        //      If that fails, fall back to parsing RAW osm data
        //
        // Otherwise just parse raw osm data
        let (building_locations, building_boundaries) = if use_cache {
            match OSMRawBuildings::read_cached_osm_data(cache_filename.to_string()) {
                Ok(data) => data,
                Err(e) => {
                    error!("Loading cached OSM data failed: {}", e);
                    OSMRawBuildings::load_and_write_cache(filename, cache_filename)?
                }
            }
        } else {
            OSMRawBuildings::load_and_write_cache(filename, cache_filename)?
        };

        debug!("Loaded OSM data");

        let building_vorinnis: HashMap<TagClassifiedBuilding, Voronoi> = building_locations
            .par_iter()
            .filter_map(|(building_type, locations)| {
                info!(
                    "Building voronoi diagram for {:?} with {} buildings",
                    building_type,
                    locations.len()
                );
                return match Voronoi::new(
                    700000,
                    locations
                        .iter()
                        .map(|p| (p.center.x(), p.center.y()))
                        .collect(),
                    Scaling::yorkshire_national_grid(),
                ) {
                    Ok(voronoi) => Some((*building_type, voronoi)),
                    Err(e) => {
                        error!("{}", e);
                        None
                    }
                };
            })
            .collect();
        let data = OSMRawBuildings {
            building_locations,
            building_boundaries,
            building_voronois: building_vorinnis,
        };
        if visualise_building_boundaries {
            debug!("Starting drawing");
            for (k, p) in data.building_voronois.iter() {
                let polygons: Vec<&geo_types::Polygon<i32>> =
                    p.polygons.polygons.iter().map(|(_, p)| p).collect();
                draw_voronoi_polygons(format!("images/{:?}Vorinni.png", k), &polygons, 20000);
            }
        }
        info!("Finished building OSM data");
        for (building_type, values) in &data.building_locations {
            debug!("There are {} {:?} ", values.len(), building_type);
        }
        Ok(data)
    }

    fn read_buildings_from_osm(
        filename: String,
    ) -> Result<
        (
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
            HashMap<BuildingBoundaryID, Polygon<i32>>,
        ),
        DataLoadingError,
    > {
        use osmpbf::{Element, ElementReader};
        info!("Reading OSM data from file: {}", filename);
        let reader = ElementReader::from_path(filename)?;
        // Read the OSM data, only select buildings, and build a hashmap of building types, with a list of locations
        debug!("Built reader, now generating Raw OSM Elements");
        let (ways, nodes): (Option<Vec<RawOSMWay>>, Option<BTreeMap<i64, RawOSMNode>>) = reader
            .par_map_reduce(
                |element| {
                    match element {
                        Element::DenseNode(node) => {
                            // Extract the building type and location from the node
                            // Then if a valid building time,instantiate a new Hashmap to be merged
                            (
                                None,
                                RawOSMNode::try_from(node).ok().map(|node| {
                                    let mut map = BTreeMap::new();
                                    map.insert(node.id, node);
                                    map
                                }),
                            )
                        }
                        //Discard all other OSM elements (Like roads)
                        Element::Way(way) => {
                            let parsed = RawOSMWay {
                                id: way.id(),
                                classification: TagClassifiedBuilding::from(way.tags()),
                                node_ids: way.refs().collect(),
                            };
                            (Some(vec![parsed]), None)
                        }
                        _ => (None, None),
                    }
                },
                || (None, None),
                |(a_ways, a_nodes), (b_ways, b_nodes)| {
                    let ways = merge_iterators(a_ways, b_ways);
                    let nodes = merge_iterators(a_nodes, b_nodes);
                    (ways, nodes)
                },
            )?;
        let nodes = nodes.ok_or_else(|| DataLoadingError::Misc {
            source: "No Nodes loaded from OSM file".to_string(),
        })?;
        let ways = ways.ok_or_else(|| DataLoadingError::Misc {
            source: "No Ways loaded from OSM file".to_string(),
        })?;
        info!("Completed generation of Raw OSM Elements. Now Creating RawBuildings, from {:?} ways and {:?} nodes",ways.len(),nodes.len());
        let mut buildings: HashMap<TagClassifiedBuilding, Vec<RawBuilding>> = HashMap::new();
        let mut building_boundaries: HashMap<BuildingBoundaryID, Polygon<i32>> = HashMap::new();

        let mut unvisited_nodes: BTreeSet<i64> = nodes.keys().copied().collect();
        for way in ways {
            let mut building_classification = HashSet::new();
            building_classification.insert(way.classification);
            let mut building_polygon = Vec::with_capacity(way.node_ids.len());
            for child in way.node_ids {
                if let Some(child) = nodes.get(&child) {
                    unvisited_nodes.remove(&child.id);
                    building_polygon.push(geo_types::Coordinate::from((
                        child.location.x(),
                        child.location.y(),
                    )));
                    building_classification.insert(child.classification);
                } else {
                    warn!("Node {} doesn't exist for way {}", child, way.id);
                }
            }
            let building_shape = geo_types::Polygon::new(building_polygon.into(), vec![]);
            let building_id = BuildingBoundaryID::default();
            let mut building_exists = false;
            for classification in building_classification {
                if let Some(building) =
                RawBuilding::new(classification, &building_shape, building_id)
                {
                    let building_entry = buildings.entry(classification).or_default();
                    building_entry.push(building);
                    building_exists = true;
                } else {
                    warn!("Failed to create raw building!");
                }
            }
            if building_exists {
                building_boundaries
                    .insert(building_id, building_shape)
                    .expect(
                        format!(
                            "Building ID {:?}, has multiple entries which shouldn't be possible! ",
                            building_id
                        )
                            .as_str(),
                    );
            }
        }
        debug!(
            "Loaded {} buildings from Way OSM data",
            buildings.iter().map(|(_, b)| b.len()).sum::<usize>()
        );
        for node_id in unvisited_nodes {
            if let Some(node) = nodes.get(&node_id) {
                let building_boundary_id = BuildingBoundaryID::default();
                let building_shape = geo_types::Polygon::new(
                    vec![
                        (node.location.x(), node.location.y()),
                        (node.location.x(), node.location.y()),
                    ]
                        .into(),
                    vec![],
                );
                if let Some(building) =
                RawBuilding::new(node.classification, &building_shape, building_boundary_id)
                {
                    let building_entry = buildings.entry(node.classification).or_default();
                    building_entry.push(building);
                    building_boundaries.insert(building_boundary_id, building_shape).expect(format!("Building ID {:?}, has multiple entries which shouldn't be possible! ", building_boundary_id).as_str());
                } else {
                    warn!("Failed to create raw building!");
                }
            } else {
                warn!("Unvisited Node {} doesn't exist!", node_id);
            }
        }
        debug!(
            "Loaded {} buildings from node data",
            buildings.iter().map(|(_, b)| b.len()).sum::<usize>()
        );
        debug!(
            "Removed {} Unknown nodes.",
            buildings
                .remove(&TagClassifiedBuilding::Unknown)
                .map(|b| b.len())
                .unwrap_or(0)
        );
        // Count the number of unique buildings
        info!(
            "Finished loading with {} buildings",
            buildings.iter().map(|(_, b)| b.len()).sum::<usize>()
        );

        Ok((buildings, building_boundaries))
    }
}

#[cfg(test)]
mod tests {
    use crate::{OSM_CACHE_FILENAME, OSM_FILENAME, OSMRawBuildings};
    use crate::voronoi_generator::find_seed_bounds;

    #[test]
    pub fn check_x_y_range() {
        let census_directory = "../data/".to_string();
        let osm_buildings = OSMRawBuildings::build_osm_data(
            census_directory.to_string() + OSM_FILENAME,
            census_directory + OSM_CACHE_FILENAME,
            false,
            false,
        );
        //assert!(osm_buildings.is_ok());
        let osm_buildings = osm_buildings.unwrap();
        let points: Vec<Vec<(i32, i32)>> = osm_buildings
            .building_locations
            .iter()
            .map(|(_, b)| {
                b.iter()
                    .map(|p| (p.center.x(), p.center.y()))
                    .collect::<Vec<(i32, i32)>>()
            })
            .collect::<Vec<Vec<(i32, i32)>>>();
        let p: Vec<(i32, i32)> = points.into_iter().flatten().collect(); //.collect();
        let bounds = find_seed_bounds(&p);
        let width = bounds.1.0 - bounds.0.0;
        let height = bounds.1.1 - bounds.0.1;
        println!("Bounds: {:?}", bounds);
        println!("Height: {:?}", height);
        println!("Width: {:?}", width);
        assert!(width < height);
    }
}
