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

use std::fmt::{Debug, Display, Formatter};

use serde::Serialize;
use uuid::Uuid;

use crate::config::BUS_CAPACITY;
use crate::error::Error;
use crate::models::citizen::CitizenID;
use crate::models::output_area::OutputAreaID;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct PublicTransportID {
    source: OutputAreaID,
    destination: OutputAreaID,
    id: Uuid,
}

impl PublicTransportID {
    pub fn new(source: OutputAreaID, destination: OutputAreaID) -> PublicTransportID {
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
    pub exposure_count: usize,
}

impl PublicTransport {
    pub fn new(source: OutputAreaID, destination: OutputAreaID) -> PublicTransport {
        PublicTransport {
            id: PublicTransportID::new(source, destination),
            capacity: BUS_CAPACITY,
            citizens: Default::default(),
            exposure_count: 0,
        }
    }
    pub fn add_citizen(&mut self, citizen_id: CitizenID) -> Result<(), Error> {
        if self.citizens.len() < self.capacity as usize {
            self.citizens.push(citizen_id);
            Ok(())
        } else {
            Err(Error::Simulation {
                message: "Cannot add Citizen, as Public Transport is at capacity".to_string(),
            })
        }
    }

    pub fn id(&self) -> &PublicTransportID {
        &self.id
    }

    pub fn occupants(&self) -> &Vec<CitizenID> {
        &self.citizens
    }
}

impl Display for PublicTransport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Public Transport {}, Capacity: {}, Occupancy: {}",
            self.id,
            self.capacity,
            self.citizens.len()
        )
    }
}

impl Debug for PublicTransport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Public Transport {}, Capacity: {}, Citizens: {:?}",
            self.id, self.capacity, self.citizens
        )
    }
}
