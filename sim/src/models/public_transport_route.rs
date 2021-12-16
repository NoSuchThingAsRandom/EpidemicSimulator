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

use std::any::Any;
use std::fmt::{Debug, Display, Formatter};

use serde::Serialize;
use uuid::Uuid;

use crate::config::BUS_CAPACITY;
use crate::error::Error;
use crate::models::building::{Building, BuildingID};
use crate::models::citizen::CitizenID;
use crate::models::ID;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct PublicTransportID {
    source: String,
    destination: String,
    id: Uuid,
}

impl PublicTransportID {
    pub fn new(source: String, destination: String) -> PublicTransportID {
        PublicTransportID {
            source,
            destination,
            id: Default::default(),
        }
    }
}

impl Display for PublicTransportID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Transporting from: {} to {}, unique ID: {}",
            self.source, self.destination, self.id
        )
    }
}

/// This is a container representing a public transport object like a bus or train
///
/// It implements `Building` as a hacky solution
#[derive(Clone)]
pub struct PublicTransport {
    id: PublicTransportID,
    capacity: u32,
    citizens: Vec<CitizenID>,
}

impl PublicTransport {
    pub fn new(source: String, destination: String) -> PublicTransport {
        PublicTransport {
            id: PublicTransportID::new(source, destination),
            capacity: BUS_CAPACITY,
            citizens: Default::default(),
        }
    }
}

impl Display for PublicTransport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Debug for PublicTransport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Building for PublicTransport {
    fn add_citizen(&mut self, citizen_id: CitizenID) -> Result<(), Error> {
        todo!()
    }

    fn id(&self) -> &BuildingID {
        todo!()
    }

    fn occupants(&self) -> &Vec<CitizenID> {
        todo!()
    }

    fn as_any(&self) -> &dyn Any {
        todo!()
    }
}
