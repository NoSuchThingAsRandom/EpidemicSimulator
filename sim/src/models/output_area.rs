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
use std::fmt::{Display, Formatter};

use anyhow::Context;
use geo_types::Point;
use rand::distributions::{Bernoulli, Distribution};
use rand::RngCore;
use serde::Serialize;

use load_census_data::CensusDataEntry;
use load_census_data::osm_parsing::RawBuildingTypes;
use load_census_data::tables::population_and_density_per_output_area::{
    AreaClassification, PersonType,
};

use crate::config::HOUSEHOLD_SIZE;
use crate::models::building::{Building, BuildingID, Household, Workplace};
use crate::models::citizen::{Citizen, CitizenID};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct OutputAreaID {
    code: String,
}

impl OutputAreaID {
    pub fn from_code(code: String) -> OutputAreaID {
        OutputAreaID { code }
    }
    pub fn code(&self) -> &String {
        &self.code
    }
}

impl Display for OutputAreaID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID: {}", self.code)
    }
}

/// This is a container for a Census Output Area
///
/// Has a given code corresponding to an area of the country, and a list of households and citizens
///
/// The polygon and `draw()` function can be used for image representation
#[derive(Debug)]
pub struct OutputArea {
    /// The Census Data Output Area Code
    pub output_area_id: OutputAreaID,

    /// A map of households, corresponding to what area they are in (Rural, Urban, Etc)
    pub buildings: HashMap<BuildingID, Box<dyn Building>>,
    /// A polygon for drawing this output area
    //pub polygon: geo_types::Polygon<isize>,
    pub total_residents: u32,
    /// The distribution to use to determine whether a Citizen is wearing a mask\
    /// Is stored as a distribution to increase speed
    mask_distribution: Bernoulli,
}

impl OutputArea {
    /// Builds a new output area, for the given code, polygon for drawing and a census record of the population
    ///
    /// Builds the citizens and households for this area
    pub fn new(
        output_area_id: OutputAreaID,
        //polygon: geo_types::Polygon<isize>,
        mask_compliance_ratio: f64,
    ) -> anyhow::Result<OutputArea> {
        Ok(OutputArea {
            output_area_id,
            buildings: HashMap::default(),
            //polygon,
            total_residents: 0,
            mask_distribution: Bernoulli::new(mask_compliance_ratio)
                .context("Failed to initialise the mask distribution")?,
        })
    }
    pub fn add_building(&mut self, _location: Point<isize>, _raw_building_type: RawBuildingTypes) {
        let household_building_id = BuildingID::new(self.output_area_id.clone());
        let mut _household = Household::new(household_building_id);
        todo!()
    }
    pub fn generate_citizens(
        &mut self,
        census_data: CensusDataEntry,
        rng: &mut dyn RngCore,
    ) -> anyhow::Result<HashMap<CitizenID, Citizen>> {
        let mut citizens = HashMap::with_capacity(census_data.total_population_size() as usize);

        // TODO Fix this
        let area = AreaClassification::Total;
        let pop_count = &census_data.population_count.population_counts[area];
        //for (area, pop_count) in census_data.population_count.population_counts.iter() {
        // TODO Currently assigning 4 people per household
        // Should use census data instead
        let household_number = pop_count[PersonType::All] / HOUSEHOLD_SIZE;
        let mut generated_population = 0;

        // Build households
        for _ in 0..household_number {
            let household_building_id = BuildingID::new(self.output_area_id.clone());
            let mut household = Household::new(household_building_id.clone());
            for _ in 0..HOUSEHOLD_SIZE {
                let occupation = census_data
                    .occupation_count
                    .get_random_occupation(rng)
                    .context("Cannot generate a random occupation for new Citizen!")?;
                let citizen = Citizen::new(
                    household_building_id.clone(),
                    household_building_id.clone(),
                    occupation,
                    self.mask_distribution.sample(rng),
                    rng,
                );
                household
                    .add_citizen(citizen.id())
                    .context("Failed to add Citizen to Household")?;
                citizens.insert(citizen.id(), citizen);
                self.total_residents += 1;
                generated_population += 1;
            }
            assert!(self.buildings.insert(household_building_id, Box::new(household)).is_none(), "A collision has occurred with building ID's");
            if generated_population >= pop_count[PersonType::All] {
                break;
            }
        }
        Ok(citizens)
    }
    fn extract_occupants_for_building_type<T: 'static + Building>(&self) -> Vec<CitizenID> {
        let mut citizens = Vec::new();
        for building in self.buildings.values() {
            let building = building.as_any();
            if let Some(household) = building.downcast_ref::<T>() {
                citizens.extend(household.occupants());
            }
        }
        citizens
    }
    pub fn get_residents(&self) -> Vec<CitizenID> {
        self.extract_occupants_for_building_type::<Household>()
    }
    pub fn get_workers(&self) -> Vec<CitizenID> {
        self.extract_occupants_for_building_type::<Workplace>()
    }
}

impl Clone for OutputArea {
    fn clone(&self) -> Self {
        let mut buildings_copy: HashMap<BuildingID, Box<dyn Building>> =
            HashMap::with_capacity(self.buildings.len());
        for (code, current_building) in &self.buildings {
            let current_building = current_building.as_any();
            if let Some(household) = current_building.downcast_ref::<Household>() {
                buildings_copy.insert(code.clone(), Box::new(household.clone()));
            } else if let Some(workplace) = current_building.downcast_ref::<Workplace>() {
                buildings_copy.insert(code.clone(), Box::new(workplace.clone()));
            } else {
                panic!("Unsupported building type, for cloning!")
            }
        }

        OutputArea {
            output_area_id: self.output_area_id.clone(),
            buildings: buildings_copy,
            //polygon: self.polygon.clone(),
            total_residents: self.total_residents,
            mask_distribution: self.mask_distribution,
        }
    }
}
