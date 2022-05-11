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
#![allow(dead_code)]

extern crate log;
extern crate pretty_env_logger;

pub mod config;
mod disease;
mod error;
mod interventions;
pub mod models;
pub mod simulator;
pub mod simulator_builder;
mod statistics;

use serde::{Serialize, Deserialize};

#[derive(Copy,Clone,Debug, Deserialize, Serialize)]
pub enum DayOfWeek {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Default for DayOfWeek {
    fn default() -> Self {
        DayOfWeek::Monday
    }
}

impl DayOfWeek {
    /// Returns the next day of the week
    pub fn next_day(self) -> Self {
        match self {
            DayOfWeek::Monday => { DayOfWeek::Tuesday }
            DayOfWeek::Tuesday => { DayOfWeek::Wednesday }
            DayOfWeek::Wednesday => { DayOfWeek::Thursday }
            DayOfWeek::Thursday => { DayOfWeek::Friday }
            DayOfWeek::Friday => { DayOfWeek::Saturday }
            DayOfWeek::Saturday => { DayOfWeek::Sunday }
            DayOfWeek::Sunday => { DayOfWeek::Monday }
        }
    }
    /// Returns True if the day is a weekend
    pub fn is_weekend(&self) -> bool {
        match self {
            DayOfWeek::Saturday |
            DayOfWeek::Sunday => true,
            _ => false
        }
    }
}