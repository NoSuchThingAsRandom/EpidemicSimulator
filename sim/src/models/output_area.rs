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

use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

use anyhow::Context;
use log::error;
use rand::distributions::{Bernoulli, Distribution};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use load_census_data::tables::population_and_density_per_output_area::PersonType;
use load_census_data::CensusDataEntry;
use osm_data::{RawBuilding, TagClassifiedBuilding};

use crate::config::MAX_STUDENT_AGE;
use crate::interventions::InterventionStatus;
use crate::models::building::{Building, BuildingID, BuildingType, Household, Workplace};
use crate::models::citizen::{Citizen, CitizenID, Occupation, OccupationType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputAreaID {
    code: String,
    index: u32,
}

impl OutputAreaID {
    pub fn from_code_and_index(code: String, index: u32) -> OutputAreaID {
        OutputAreaID { code, index }
    }
    pub fn code(&self) -> &String {
        &self.code
    }
    pub fn index(&self) -> usize {
        self.index as usize
    }
}

impl Display for OutputAreaID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID: {}", self.code)
    }
}

impl Hash for OutputAreaID {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.code.hash(state)
    }
}

impl Eq for OutputAreaID {}

impl PartialEq for OutputAreaID {
    fn eq(&self, other: &Self) -> bool {
        self.code.eq(other.code())
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
    pub citizens_eligible_for_vaccine: Option<HashSet<CitizenID>>,
    pub citizens: Vec<Citizen>,
    /// A map of households, corresponding to what area they are in (Rural, Urban, Etc)
    pub buildings: Vec<Box<dyn Building + Sync + Send>>,
    /// A polygon for drawing this output area
    pub polygon: geo_types::Polygon<i32>,
    pub total_residents: u32,
    pub interventions: InterventionStatus,

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
        polygon: geo_types::Polygon<i32>,
        mask_compliance_ratio: f64,
    ) -> anyhow::Result<OutputArea> {
        Ok(OutputArea {
            output_area_id,
            citizens_eligible_for_vaccine: None,
            citizens: Default::default(),
            buildings: Default::default(),
            polygon,
            total_residents: 0,
            interventions: Default::default(),
            mask_distribution: Bernoulli::new(mask_compliance_ratio)
                .context("Failed to initialise the mask distribution")?,
        })
    }
    /// Generates the Citizens for this Output Area, with households being the provided [`RawBuilding`]
    ///
    /// Note each [`RawBuilding`] must have a classification of [`TagClassifiedBuilding::Household`]
    ///
    /// Returns the total number of citizens that have been generated
    pub fn generate_citizens_with_households(
        &mut self,
        mut global_citizen_index: u32,
        rng: &mut dyn RngCore,
        census_data: CensusDataEntry,
        possible_buildings: Vec<RawBuilding>,
    ) -> anyhow::Result<u32> {
        self.citizens = Vec::with_capacity(census_data.total_population_size() as usize);
        let pop_count = &census_data.population_count.population_counts;

        // TODO Fix this
        let household_size = (pop_count[PersonType::All] as usize / possible_buildings.len()) + 1;
        // Should use census data instead
        let mut generated_population = 0;
        // Build households
        let mut possible_buildings = possible_buildings.iter();
        let possible_buildings_size = possible_buildings.len();
        while generated_population <= pop_count[PersonType::All] {
            if let Some(location) = possible_buildings.next() {
                assert_eq!(location.classification(), TagClassifiedBuilding::Household);
                let household_building_id = BuildingID::new(
                    self.output_area_id.clone(),
                    BuildingType::Household,
                    self.buildings.len() as u32,
                );
                let mut household =
                    Household::new(household_building_id.clone(), location.center());
                for _ in 0..household_size {
                    let raw_occupation = census_data.occupation_count.get_random_occupation(rng);
                    let age = census_data.age_population.get_random_age(rng);
                    let occupation = if age < MAX_STUDENT_AGE {
                        Occupation::Student
                    } else {
                        Occupation::Normal { occupation: OccupationType::try_from(raw_occupation).unwrap_or_else(|_| panic!("Couldn't convert Census Occupation ({:?}), to sim occupation", raw_occupation)) }
                    };
                    let citizen = Citizen::new(
                        CitizenID::from_indexes(global_citizen_index),
                        household_building_id.clone(),
                        household_building_id.clone(),
                        age,
                        occupation,
                        self.mask_distribution.sample(rng),
                        rng,
                    );
                    household
                        .add_citizen(citizen.id())
                        .context("Failed to add Citizen to Household")?;
                    self.citizens.push(citizen);
                    self.total_residents += 1;
                    generated_population += 1;
                    global_citizen_index += 1;
                }
                self.buildings.push(Box::new(household));
                if generated_population >= pop_count[PersonType::All] {
                    break;
                }
            } else {
                error!(
                    "Output Area: {} has run out of households ({}) of size {} to allocate residents: ({}/{}) to.",
                    self.output_area_id,
                possible_buildings_size,
                household_size,
                    generated_population,
                    pop_count[PersonType::All]
                );
                return Ok(self.citizens.len() as u32);
            }
        }
        Ok(self.citizens.len() as u32)
    }
    fn extract_occupants_for_building_type<T: 'static + Building>(&self) -> Vec<CitizenID> {
        let mut citizens = Vec::new();
        for building in &self.buildings {
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
    pub fn get_citizen(&self, local_index: &u32) -> Option<&Citizen> {
        self.citizens.get(*local_index as usize)
    }
    pub fn get_citizen_mut(&mut self, local_index: &u32) -> Option<&mut Citizen> {
        self.citizens.get_mut(*local_index as usize)
    }
    pub fn id(&self) -> OutputAreaID {
        self.output_area_id.clone()
    }
    pub fn decrement_index(&mut self) {
        self.output_area_id.index -= 1;
    }
}

impl Clone for OutputArea {
    fn clone(&self) -> Self {
        let mut buildings_copy: Vec<Box<dyn Building + Sync + Send>> =
            Vec::with_capacity(self.buildings.len());
        for current_building in &self.buildings {
            let current_building = current_building.as_any();
            if let Some(household) = current_building.downcast_ref::<Household>() {
                buildings_copy.push(Box::new(household.clone()));
            } else if let Some(workplace) = current_building.downcast_ref::<Workplace>() {
                buildings_copy.push(Box::new(workplace.clone()));
            } else {
                panic!("Unsupported building type, for cloning!")
            }
        }

        OutputArea {
            output_area_id: self.output_area_id.clone(),
            citizens_eligible_for_vaccine: self.citizens_eligible_for_vaccine.clone(),
            citizens: self.citizens.clone(),
            buildings: buildings_copy,
            polygon: self.polygon.clone(),
            total_residents: self.total_residents,
            interventions: self.interventions.clone(),
            mask_distribution: self.mask_distribution,
        }
    }
}
