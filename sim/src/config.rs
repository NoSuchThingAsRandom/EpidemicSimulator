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

pub const STARTING_INFECTED_COUNT: u32 = 10;
/// The amount of floor space in m^2 per Workplace building
pub const WORKPLACE_BUILDING_SIZE: u16 = 1000;
pub const HOUSEHOLD_SIZE: u16 = 4;

/// How often to print debug statements
pub const DEBUG_ITERATION_PRINT: usize = 10;

pub const PUBLIC_TRANSPORT_PERCENTAGE: f64 = 0.2;
pub const BUS_CAPACITY: u32 = 20;

pub fn get_memory_usage() -> anyhow::Result<String> {
    Ok(format!(
        "{:.3} GB",
        (procinfo::pid::statm_self()?.size * page_size::get() / 1024 / 1024) as f64 / 1024.0
    ))
}



