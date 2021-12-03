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

use crate::config::HOUSEHOLD_SIZE;
use crate::models::building::{Building, BuildingCode, Household, Workplace};
use crate::models::citizen::Citizen;

/// This is a container for a Census Output Area
///
/// Has a given code corresponding to an area of the country, and a list of households and citizens
///
/// The polygon and `draw()` function can be used for image representation
#[derive(Debug)]
pub struct OutputArea {
    /// The Census Data Output Area Code
    pub output_area_code: String,

    /// A map of households, corresponding to what area they are in (Rural, Urban, Etc)
    pub buildings: EnumMap<AreaClassification, HashMap<Uuid, Box<dyn Building>>>,
    /// A polygon for drawing this output area
    pub polygon: geo_types::Polygon<f64>,
    pub total_residents: u32,
}

impl OutputArea {
    /// Builds a new output area, for the given code, polygon for drawing and a census record of the population
    ///
    /// Builds the citizens and households for this area
    pub fn new(
        output_area_code: String,
        polygon: geo_types::Polygon<f64>,
    ) -> anyhow::Result<OutputArea> {
        Ok(OutputArea {
            output_area_code,
            buildings: EnumMap::default(),
            polygon,
            total_residents: 0,
        })
    }
    pub fn generate_citizens(
        &mut self,
        census_data: CensusDataEntry,
        rng: &mut dyn RngCore,
    ) -> anyhow::Result<HashMap<Uuid, Citizen>> {
        // TODO Fix this
        let mut citizens = HashMap::with_capacity(census_data.total_population_size() as usize);
        let area = AreaClassification::Total;
        let pop_count = &census_data.population_count.population_counts[area];
        //for (area, pop_count) in census_data.population_count.population_counts.iter() {
        // TODO Currently assigning 4 people per household
        // Should use census data instead
        let household_number = pop_count[PersonType::All] / HOUSEHOLD_SIZE;
        let mut generated_population = 0;
        let mut households_for_area: HashMap<Uuid, Box<dyn Building>> =
            HashMap::with_capacity(household_number as usize);

        // Build households
        for _ in 0..household_number {
            let household_building_code = BuildingCode::new(self.output_area_code.clone(), area);
            let mut household = Household::new(household_building_code.clone());
            for _ in 0..HOUSEHOLD_SIZE {
                let occupation = census_data
                    .occupation_count
                    .get_random_occupation(rng)
                    .context("Cannot generate a random occupation for new Citizen!")?;
                let citizen = Citizen::new(
                    household_building_code.clone(),
                    household_building_code.clone(),
                    occupation,
                );
                household
                    .add_citizen(citizen.id())
                    .context("Failed to add Citizen to Household")?;
                citizens.insert(citizen.id(), citizen);
                self.total_residents += 1;
                generated_population += 1;
            }
            households_for_area.insert(household_building_code.building_id(), Box::new(household));
            if generated_population >= pop_count[PersonType::All] {
                break;
            }
        }
        self.buildings[area] = households_for_area;
        Ok(citizens)
    }
    fn extract_occupants_for_building_type<T: 'static + Building>(&self) -> Vec<Uuid> {
        let mut citizens = Vec::new();
        for (_, data) in self.buildings.iter() {
            for building in data.values() {
                let building = building.as_any();
                if let Some(household) = building.downcast_ref::<T>() {
                    citizens.extend(household.occupants());
                }
            }
        }
        citizens
    }
    pub fn get_residents(&self) -> Vec<Uuid> {
        self.extract_occupants_for_building_type::<Household>()
    }
    pub fn get_workers(&self) -> Vec<Uuid> {
        self.extract_occupants_for_building_type::<Workplace>()
    }
}
