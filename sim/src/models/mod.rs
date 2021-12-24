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

#![allow(dead_code)]


use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

use geo_types::{LineString, Point, Polygon};
use log::{debug, error, info};
use serde::Serialize;

use load_census_data::osm_parsing::GRID_SIZE;
use load_census_data::parsing_error::DataLoadingError;
use load_census_data::voronoi_generator::Scaling;

use crate::config::get_memory_usage;
use crate::error::Error;
use crate::models::building::BuildingID;
use crate::models::output_area::OutputAreaID;
use crate::models::public_transport_route::PublicTransportID;

pub mod building;
pub mod citizen;
pub mod output_area;
pub mod public_transport_route;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize)]
pub enum ID {
    Building(BuildingID),
    OutputArea(OutputAreaID),
    PublicTransport(PublicTransportID),
}

impl Display for ID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ID::Building(id) => {
                write!(f, "{}", id)
            }
            ID::OutputArea(id) => {
                write!(f, "{}", id)
            }
            ID::PublicTransport(id) => {
                write!(f, "{}", id)
            }
        }
    }
}