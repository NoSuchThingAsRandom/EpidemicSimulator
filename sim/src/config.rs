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

use load_census_data::tables::CensusTableNames;

pub const STARTING_INFECTED_COUNT: u32 = 10;

pub fn get_census_table_filename<'a>(table: CensusTableNames) -> &'a str {
    match table {
        CensusTableNames::PopulationDensity => "data/tables/york_population_144.csv",
        CensusTableNames::OccupationCount => "data/tables/york_occupation_count_ks608uk.csv",
        CensusTableNames::OutputAreaMap => {
            "data/census_map_areas/England_oa_2011/england_oa_2011.shp"
        }
    }
}
