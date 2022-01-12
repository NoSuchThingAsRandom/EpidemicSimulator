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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::Context;
use log::{debug, error, info, warn};
use rand::{RngCore, thread_rng};
use rand::prelude::{IteratorRandom, SliceRandom};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use load_census_data::CensusData;
use load_census_data::osm_parsing::{
    BuildingBoundaryID, OSMRawBuildings, RawBuilding, TagClassifiedBuilding,
};
use load_census_data::parsing_error::{DataLoadingError, ParseErrorType};
use load_census_data::polygon_lookup::PolygonContainer;
use load_census_data::tables::occupation_count::OccupationType;

use crate::config::STARTING_INFECTED_COUNT;
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::error::SimError;
use crate::models::building::{Building, BuildingID, BuildingType, Workplace};
use crate::models::citizen::{Citizen, CitizenID};
use crate::models::output_area::{OutputArea, OutputAreaID};
use crate::simulator::Timer;

pub struct SimulatorBuilder {
    census_data: CensusData,
    osm_data: OSMRawBuildings,
    pub output_areas: HashMap<OutputAreaID, OutputArea>,
    output_areas_polygons: PolygonContainer<String>,
    pub disease_model: DiseaseModel,
    pub citizens: HashMap<CitizenID, Citizen>,
}

/// Initialisation Methods
impl SimulatorBuilder {
    /// Generates the Output Area Structs, from the Census Data
    ///
    /// And returns the starting population count
    pub fn initialise_output_areas(&mut self) -> anyhow::Result<()> {
        // Build the initial Output Areas and Households
        for entry in self.census_data.values() {
            let output_id = OutputAreaID::from_code(entry.output_area_code.to_string());
            // TODO Remove polygons from grid
            let polygon = self
                .output_areas_polygons
                .polygons
                .get(output_id.code())
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Building output areas map".to_string(),
                        key: output_id.to_string(),
                    },
                })
                .context(format!("Loading polygon shape for area: {}", output_id))?;
            let new_area = OutputArea::new(
                output_id,
                polygon.clone(),
                self.disease_model.mask_percentage,
            )
                .context("Failed to create Output Area")?;
            self.output_areas
                .insert(new_area.output_area_id.clone(), new_area);
        }
        Ok(())
    }

    /// Assigns buildings to their enclosing Output Area, and Removes Output Areas that do not have any buildings
    pub fn assign_buildings_to_output_areas(
        &mut self,
    ) -> anyhow::Result<HashMap<OutputAreaID, HashMap<TagClassifiedBuilding, Vec<RawBuilding>>>>
    {
        debug!(
            "Attempting to allocating {} possible buildings to {} Output Areas",
            self.osm_data
                .building_locations
                .iter()
                .map(|(_k, v)| v.len())
                .sum::<usize>(),
            self.output_areas.len()
        );
        // Assign possible buildings to output areas
        let possible_buildings_per_area = parallel_assign_buildings_to_output_areas(
            &self.osm_data.building_boundaries,
            &self.osm_data.building_locations,
            &self.output_areas_polygons,
        );
        let count: usize = possible_buildings_per_area
            .par_iter()
            .map(|(_, classed_building)| {
                classed_building
                    .par_iter()
                    .map(|(_, buildings)| buildings.len())
                    .sum::<usize>()
            })
            .sum();

        self.output_areas
            .retain(|code, _area| possible_buildings_per_area.contains_key(code));
        debug!(
            "{} Buildings have been assigned. {} Output Areas remaining (with buildings)",
            count,
            self.output_areas.len()
        );
        Ok(possible_buildings_per_area)
    }

    /// Generates the Citizens for each Output Area
    pub fn generate_citizens(
        &mut self,
        rng: &mut dyn RngCore,
        possible_buildings_per_area: &mut HashMap<
            OutputAreaID,
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
        >,
    ) -> anyhow::Result<()> {
        let mut citizens = HashMap::new();
        let mut no_buildings = 0;
        let mut no_households = 0;
        // Generate Citizens

        // This ref self is needed, because we have a mut borrow (Output Areas) and an immutable borrow (Census Data)
        // TODO This is super hacky and I hate it
        let ref_output_areas = Rc::new(RefCell::new(&mut self.output_areas));
        let census_data_ref = &self.census_data;
        ref_output_areas.borrow_mut().retain(|output_area_id, output_area| {
            let generate_citizen_closure = || -> anyhow::Result<()> {
                // Retrieve the Census Data
                let census_data_entry = census_data_ref
                    .for_output_area_code(output_area_id.code().to_string())
                    .ok_or_else(|| SimError::InitializationError {
                        message: format!(
                            "Cannot generate Citizens for Output Area {} as Census Data exists",
                            output_area_id
                        ),
                    })?;
                // Extract the possible buildings for this Output Area
                let possible_buildings = possible_buildings_per_area
                    .get_mut(output_area_id)
                    .ok_or_else(|| {
                        no_buildings += 1;
                        return SimError::InitializationError {
                            message: format!(
                                "Cannot generate Citizens for Output Area {} as no buildings exist",
                                output_area_id
                            ),
                        };
                    })?;
                // Retrieve the Households for this Output Area
                let possible_households = possible_buildings
                    .remove(&TagClassifiedBuilding::Household)
                    .ok_or_else(|| {
                        no_households += 1;
                        return SimError::InitializationError {
                            message: format!(
                                "Cannot generate Citizens for Output Area {} as no households exist",
                                output_area_id
                            ),
                        };
                    })?;
                citizens.extend(output_area.generate_citizens_with_households(
                    rng,
                    census_data_entry,
                    possible_households,
                )?);
                Ok(())
            }();
            generate_citizen_closure.is_ok()
        });
        error!(
            "Households and Citizen generation succeeded for {} Output Areas.",
            ref_output_areas.borrow().len()
        );
        self.citizens = citizens;
        Ok(())
    }

    /// Iterates through all Output Areas, and All Citizens in that Output Area
    ///
    /// Picks a Workplace Output Area, determined from Census Data Distribution
    ///
    /// Allocates that Citizen to the Workplace Building in that chosen Output Area
    pub fn build_workplaces(
        &mut self,
        rng: &mut dyn RngCore,
        mut possible_buildings_per_area: HashMap<OutputAreaID, Vec<RawBuilding>>,
    ) -> anyhow::Result<()> {
        debug!(
            "Assigning workplaces to {} output areas ",
            self.output_areas.len()
        );
        // Add Workplace Output Areas to Every Citizen
        let mut citizens_to_allocate: HashMap<OutputAreaID, (Vec<CitizenID>, Vec<RawBuilding>)> =
            HashMap::new();
        let mut failed_output_areas = Vec::new();


        let mut dead_output_areas = 0;
        let mut no_buildings_per_output_area = 0;
        // Assign workplace areas to each Citizen, per Output area
        for (household_output_area_code, household_output_area) in &self.output_areas {
            // Retrieve the census data for the household output area
            let household_census_data = self
                .census_data
                .for_output_area_code(household_output_area_code.code().to_string())
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Cannot retrieve Census Data for output area ".to_string(),
                        key: household_output_area_code.to_string(),
                    },
                })?;


            // For each Citizen, assign a workplace area
            for citizen_id in household_output_area.get_residents() {
                // Generate a workplace Output Area, and ensure it exists!
                let mut attempt_index = 0;
                let mut workplace_output_area_code = OutputAreaID::from_code("".to_string());

                // TODO This generation is broken!
                while !(possible_buildings_per_area.contains_key(&workplace_output_area_code)
                    && self.output_areas.contains_key(&workplace_output_area_code))
                {
                    workplace_output_area_code = OutputAreaID::from_code(
                        household_census_data
                            .get_random_workplace_area(rng)
                            .context("Failed to select a random workplace")?,
                    );
                    if !possible_buildings_per_area.contains_key(&workplace_output_area_code) {
                        no_buildings_per_output_area += 1;
                    }
                    if !self.output_areas.contains_key(&workplace_output_area_code) {
                        dead_output_areas += 1;
                    }
                    attempt_index += 1;
                    if attempt_index == 10 {
                        workplace_output_area_code = OutputAreaID::from_code("".to_string());
                        break;
                    }
                }
                if workplace_output_area_code == OutputAreaID::from_code("".to_string()) {
                    //error!("Failed to find workplace area for household area {:?} ",household_output_area_code);
                    failed_output_areas.push(household_output_area_code.clone());
                    break;
                }
                // Initialise the workplace area if it doesn't exist
                if !citizens_to_allocate.contains_key(&workplace_output_area_code) {
                    let possible_workplaces = possible_buildings_per_area
                        .remove(&workplace_output_area_code)
                        .ok_or_else(|| SimError::InitializationError {
                            message: format!(
                                "Cannot generate Citizens for Output Area {} as no buildings exist",
                                workplace_output_area_code
                            ),
                        })?;
                    citizens_to_allocate.insert(
                        workplace_output_area_code.clone(),
                        (Vec::new(), possible_workplaces),
                    );
                }
                citizens_to_allocate
                    .get_mut(&workplace_output_area_code)
                    .ok_or_else(|| DataLoadingError::ValueParsingError {
                        source: ParseErrorType::MissingKey {
                            context: "Cannot retrieve Output Area to add Citizens to  ".to_string(),
                            key: workplace_output_area_code.to_string(),
                        },
                    })?
                    .0
                    .push(citizen_id);
            }
        }
        error!(
            "Failed to find workplace buildings for {} output areas. {} areas don't exist, and {} areas don't have workplaces",
            failed_output_areas.len(),dead_output_areas,no_buildings_per_output_area
        );
        debug!("Creating workplace buildings");
        // Create buildings for each Workplace output area
        for (workplace_area_code, mut to_allocate) in citizens_to_allocate {
            // Randomise the order of the citizens, to reduce the number of Citizens sharing household and Workplace output areas
            to_allocate.0.shuffle(rng);
            // TODO Check buildings are shuffled
            let mut possible_buildings = to_allocate.1.iter();
            let total_building_count = possible_buildings.len();
            let total_workers = to_allocate.0.len();

            // This is the Workplace list to allocate citizens to
            let mut current_workplaces_to_allocate: HashMap<OccupationType, Workplace> =
                HashMap::new();

            // This is the list of full workplaces that need to be added to the parent Output Area
            let mut workplace_buildings: HashMap<BuildingID, Box<dyn Building>> = HashMap::new();
            for (index, citizen_id) in to_allocate.0.iter().enumerate() {
                let citizen = self.citizens.get_mut(citizen_id).ok_or_else(|| {
                    DataLoadingError::ValueParsingError {
                        source: ParseErrorType::MissingKey {
                            context: "Cannot retrieve Citizen to assign Workplace ".to_string(),
                            key: citizen_id.to_string(),
                        },
                    }
                })?;

                // 3 Cases
                // Work place exists and Citizen can be added:
                //      Add Citizen to it
                // Work place exists and Citizen cannot be added:
                //      Save the current Workplace
                //      Generate a new Workplace
                //      Add a Citizen to the new Workplace
                // Work place doesn't exist
                //      Generate a new Workplace
                //      Add a Citizen to the new Workplace
                // Else
                let workplace = current_workplaces_to_allocate.remove(&citizen.occupation());
                let workplace = match workplace {
                    Some(mut workplace) => {
                        match workplace.add_citizen(*citizen_id) {
                            Ok(_) => workplace,
                            Err(_) => {
                                workplace_buildings
                                    .insert(workplace.id().clone(), Box::new(workplace));
                                // TODO Have better distribution of AreaClassification?
                                let mut workplace = Workplace::new(
                                    BuildingID::new(
                                        workplace_area_code.clone(),
                                        BuildingType::Workplace,
                                    ),
                                    *possible_buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces{} to assign workers{}/{} to in Output Area: {}", total_building_count, index, total_workers, workplace_area_code) })?,
                                    citizen.occupation());
                                workplace.add_citizen(*citizen_id).context(
                                    "Cannot add Citizen to freshly generated Workplace!",
                                )?;
                                workplace
                            }
                        }
                    }
                    None => {
                        let mut workplace = Workplace::new(
                            BuildingID::new(
                                workplace_area_code.clone(),
                                BuildingType::Workplace,
                            ),
                            *possible_buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces{} to assign workers{}/{} to in Output Area: {}", total_building_count, index, total_workers, workplace_area_code) })?,
                            citizen.occupation(),
                        );
                        workplace
                            .add_citizen(*citizen_id)
                            .context("Cannot add Citizen to new workplace!")?;
                        workplace
                    }
                };
                citizen.set_workplace_code(workplace.id().clone());
                // Add the unfilled Workplace back to the allocator
                current_workplaces_to_allocate.insert(citizen.occupation(), workplace);
            }
            let workplace_output_area = self
                .output_areas
                .get_mut(&workplace_area_code)
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Retrieving output area for building workplaces ".to_string(),
                        key: workplace_area_code.to_string(),
                    },
                })?;
            // Add any leftover Workplaces to the Output Area
            current_workplaces_to_allocate
                .drain()
                .for_each(|(_, workplace)| {
                    workplace_buildings.insert(workplace.id().clone(), Box::new(workplace));
                });
            workplace_output_area.buildings.extend(workplace_buildings);
        }
        Ok(())
    }

    pub fn apply_initial_infections(&mut self, rng: &mut dyn RngCore) -> anyhow::Result<()> {
        for _ in 0..STARTING_INFECTED_COUNT {
            let citizen = self
                .citizens
                .values_mut()
                .choose(rng)
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::IsEmpty {
                        message: "No citizens exist in the output areas for seeding the disease"
                            .to_string(),
                    },
                })
                .context("Initialisation of disease!")?;
            citizen.disease_status = DiseaseStatus::Infected(0);
        }
        Ok(())
    }

    pub fn new(
        census_data: CensusData,
        osm_data: OSMRawBuildings,
        output_areas_polygons: PolygonContainer<String>,
    ) -> anyhow::Result<SimulatorBuilder> {
        Ok(SimulatorBuilder {
            census_data,
            osm_data,
            output_areas: Default::default(),
            output_areas_polygons,
            disease_model: DiseaseModel::covid(),
            citizens: Default::default(),
        })
    }
    pub fn build(&mut self) -> anyhow::Result<()> {
        let mut timer = Timer::default();
        let mut rng = thread_rng();

        self.initialise_output_areas()
            .context("Failed to initialise output areas!")?;
        timer.code_block_finished("Initialised Output Areas")?;
        let mut possible_buildings_per_area = self
            .assign_buildings_to_output_areas()
            .context("Failed to assign buildings to output areas")?;
        timer.code_block_finished("Assigned Possible Buildings to Output Areas")?;
        self.generate_citizens(&mut rng, &mut possible_buildings_per_area)
            .context("Failed to generate Citizens")?;

        timer.code_block_finished(&format!(
            "Generated Citizens and residences for {} output areas",
            self.output_areas.len()
        ))?;
        // TODO Currently any buildings remaining are treated as Workplaces
        let possible_workplaces: HashMap<OutputAreaID, Vec<RawBuilding>> =
            possible_buildings_per_area
                .drain()
                .filter_map(|(area, mut classified_buildings)| {
                    let buildings: Vec<RawBuilding> =
                        classified_buildings.drain().flat_map(|(_, a)| a).collect();
                    if buildings.is_empty() {
                        return None;
                    }
                    Some((area, buildings))
                })
                .collect();
        let a = possible_workplaces
            .keys()
            .cloned()
            .collect::<HashSet<OutputAreaID>>();
        let b = self
            .output_areas
            .keys()
            .cloned()
            .collect::<HashSet<OutputAreaID>>();
        let c: HashSet<&OutputAreaID> = a.intersection(&b).collect();
        debug!(
            "There are {} areas with workplace buildings",
            possible_workplaces.len()
        );
        debug!("Union of workplace and output area:{} ", c.len());

        // Remove any areas that do not have any workplaces
        let output_area_ref = Rc::new(RefCell::new(&mut self.output_areas));
        let citizens_ref = &mut self.citizens;
        output_area_ref.borrow_mut().retain(|code, data| {
            if !possible_workplaces.contains_key(code) {
                data.get_residents().iter().for_each(|id| {
                    if citizens_ref.remove(id).is_none() {
                        error!("Failed to remove citizen: {}", id);
                    }
                });

                false
            } else {
                true
            }
        });
        info!("Starting to build workplaces for {} areas",self.output_areas.len());
        self.build_workplaces(&mut rng, possible_workplaces)
            .context("Failed to build workplaces")?;
        timer.code_block_finished("Generated workplaces for {} Output Areas")?;

        let work_from_home_count: u32 = self.citizens.par_iter().map(|(_, citizen)| if citizen.household_code.eq(&citizen.workplace_code) { 1 } else { 0 }).sum();
        debug!("{} out of {} Citizens, are working from home.",work_from_home_count,self.citizens.len());
        // Infect random citizens
        self.apply_initial_infections(&mut rng)
            .context("Failed to create initial infections")?;

        timer.code_block_finished("Initialization completed wi")?;
        debug!(
            "Starting Statistics: There are {} total Citizens, {} Output Areas",
            self.citizens.len(),
            self.output_areas.len()
        );
        assert_eq!(
            self.citizens.len() as u32,
            self.output_areas
                .iter()
                .map(|area| area.1.total_residents)
                .sum::<u32>()
        );
        Ok(())
    }
}

/// On csgpu2 with 20? threads took 11 seconds as oppose to 57 seconds for single threaded version
fn parallel_assign_buildings_to_output_areas(
    building_boundaries: &HashMap<BuildingBoundaryID, geo_types::Polygon<i32>>,
    building_locations: &HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
    output_area_lookup: &PolygonContainer<String>,
) -> HashMap<OutputAreaID, HashMap<TagClassifiedBuilding, Vec<RawBuilding>>> {
    building_locations.into_par_iter().map(|(building_type, possible_building_locations)|
        {
            // Try find Area Codes for the given building
            let area_codes = possible_building_locations.into_par_iter().filter_map(|building| {
                let boundary = building_boundaries.get(&building.boundary_id());
                if let Some(boundary) = boundary {
                    if let Ok(areas) = output_area_lookup.find_polygons_containing_polygon(boundary) {
                        let f = areas.iter().map(|area|
                            OutputAreaID::from_code(area.to_string()))
                            .zip(std::iter::repeat(vec![*building]))
                            .collect::<HashMap<OutputAreaID, Vec<RawBuilding>>>();
                        return Some(f);
                    }
                } else {
                    warn!("Raw Building is missing Boundary with id: {:?}",building.boundary_id());
                }
                None
            });
            // Group By Area Code
            let area_codes = area_codes.reduce(HashMap::new, |mut a, b| {
                for (area, area_buildings) in b {
                    let area_entry = a.entry(area).or_default();
                    area_entry.extend(area_buildings)
                }
                a
            });
            (*building_type, area_codes)
        }).fold(HashMap::new, |mut a: HashMap<
        OutputAreaID,
        HashMap<TagClassifiedBuilding, Vec<RawBuilding>>>, b: (TagClassifiedBuilding, HashMap<OutputAreaID, Vec<RawBuilding>>)| {
        for (area_code, buildings) in b.1 {
            let area_entry = a.entry(area_code).or_default();
            let class_entry = area_entry.entry(b.0).or_default();
            class_entry.extend(buildings);
        }
        a
    }).
        // Reduce to a single hashmap
        reduce(HashMap::new, |mut a, b| {
            for (area, classed_buildings) in b {
                let area_entry = a.entry(area).or_default();
                for (class, buildings) in classed_buildings {
                    let class_entry = area_entry.entry(class).or_default();
                    class_entry.extend(buildings);
                }
            }
            a
        })
}
