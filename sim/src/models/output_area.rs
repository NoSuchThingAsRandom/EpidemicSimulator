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

use std::collections::HashMap;

use anyhow::Context;
use enum_map::EnumMap;
use rand::RngCore;
use uuid::Uuid;

use load_census_data::CensusDataEntry;
use load_census_data::tables::population_and_density_per_output_area::{
    AreaClassification, PersonType,
};

use crate::models::building::{Building, BuildingCode, Household};
use crate::models::citizen::Citizen;

/// This is a container for a Census Output Area
///
/// Has a given code corresponding to an area of the country, and a list of households and citizens
///
/// The polygon and `draw()` function can be used for image representation
#[derive(Debug)]
pub struct OutputArea {
    /// The Census Data Output Area Code
    pub code: String,
    /// The list of citizens who have a "home" in this area
    pub citizens: HashMap<Uuid, Citizen>,
    /// How big the area is in Hectares
    pub area_size: f32,
    /// How many people per hectare? TODO Check this
    pub density: f32,
    /// A map of households, corresponding to what area they are in (Rural, Urban, Etc)
    pub buildings: EnumMap<AreaClassification, HashMap<Uuid, Box<dyn Building>>>,
    /// A polygon for drawing this output area
    pub polygon: geo_types::Polygon<f64>,
}

impl OutputArea {
    /// Builds a new output area, for the given code, polygon for drawing and a census record of the population
    ///
    /// Builds the citizens and households for this area
    pub fn new(
        output_area_code: String,
        polygon: geo_types::Polygon<f64>,
        census_data: CensusDataEntry,
        rng: &mut dyn RngCore,
    ) -> anyhow::Result<OutputArea> {
        // TODO Fix this
        let mut buildings = EnumMap::default();
        let mut citizens = HashMap::with_capacity(census_data.total_population_size() as usize);

        for (area, pop_count) in census_data.population_count.population_counts.iter() {
            // TODO Currently assigning 4 people per household
            // Should use census data instead
            let household_size = 4;
            let household_number = pop_count[PersonType::All] / household_size;
            let mut generated_population = 0;
            let mut households_for_area: HashMap<Uuid, Box<dyn Building>> = HashMap::with_capacity(household_number as usize);

            // Build households
            for _ in 0..household_number {
                let household_building_code = BuildingCode::new(output_area_code.clone(), area);
                let mut household = Household::new(household_building_code.clone());
                for _ in 0..household_size {
                    let occupation = census_data.occupation_count.get_random_occupation(rng).context("Cannot generate a random occupation for new Citizen!")?;
                    let citizen = Citizen::new(household_building_code.clone(), household_building_code.clone(), occupation);
                    household.add_citizen(citizen.id()).context("Failed to add Citizen to Household")?;
                    citizens.insert(citizen.id(), citizen);
                    generated_population += 1;
                }
                households_for_area.insert(household_building_code.building_id(), Box::new(household));
                if generated_population >= pop_count[PersonType::All] {
                    break;
                }
            }
            buildings[area] = households_for_area;
        }

        Ok(OutputArea {
            code: output_area_code,
            citizens,
            area_size: census_data.population_count.area_size,
            density: census_data.population_count.density,
            buildings,
            polygon,
        })
    }
}
