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
use std::collections::HashMap;
use std::default::Default;
use std::rc::Rc;

use anyhow::Context;
use log::{debug, error, info, warn};
use rand::{Rng, RngCore, thread_rng};
use rand::prelude::{IteratorRandom, SliceRandom};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};

use load_census_data::{CensusData, CensusDataEntry};
use load_census_data::osm_parsing::{
    BuildingBoundaryID, OSMRawBuildings, RawBuilding, TagClassifiedBuilding,
};
use load_census_data::parsing_error::{DataLoadingError, ParseErrorType};
use load_census_data::polygon_lookup::PolygonContainer;
use load_census_data::tables::employment_densities::EmploymentDensities;
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


    fn generate_output_are_code(&self, census_data: &CensusDataEntry, possible_buildings_per_area: &HashMap<OutputAreaID, Vec<RawBuilding>>) -> anyhow::Result<Option<OutputAreaID>> {
        let code = census_data.get_random_workplace_area(&mut thread_rng()).context("Failed to retrieve random workplace area")?;
        let output_area_code = OutputAreaID::from_code(code);

        if !self.output_areas.contains_key(&output_area_code) {
            warn!("Output area: {} doesn't exist!",output_area_code);
            return Ok(None);
        }

        let buildings = possible_buildings_per_area.get(&output_area_code);
        if buildings.is_none() {
            warn!("Buildings don't exist for area: {}",output_area_code);
            return Ok(None);
        }
        Ok(Some(output_area_code))
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
        // Shuffle the buildings
        possible_buildings_per_area.par_iter_mut().for_each(|(_, buildings)| buildings.shuffle(&mut thread_rng()));

        // Add Workplace Output Areas to Every Citizen
        let mut citizens_to_allocate: HashMap<OutputAreaID, Vec<CitizenID>> =
            HashMap::new();
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
            'citizens: for citizen_id in household_output_area.get_residents() {
                let mut index = 0;
                let workplace_output_area_code: OutputAreaID =
                    loop {
                        let code = self.generate_output_are_code(&household_census_data, &possible_buildings_per_area).context("Failed to generate random workplace area code")?;
                        if let Some(code) = code {
                            break code;
                        }
                        index += 1;
                        if index > 5 {
                            error!("Failed to generate code for Citizen: {}",citizen_id);
                            continue 'citizens;
                        }
                    };
                let citizens_to_add = citizens_to_allocate.entry(workplace_output_area_code).or_default();
                citizens_to_add.push(citizen_id);
            }
        }
        debug!("Creating workplace buildings for: {:?} Citizens and {} Output Areas",citizens_to_allocate.iter().map(|(_,citizens)|citizens.len()).sum::<usize>(),citizens_to_allocate.len());
        // Create buildings for each Workplace output area
        'citizen_allocation_loop: for (workplace_area_code, citizens) in citizens_to_allocate {
            // Retrieve the buildings or skip this area
            let possible_buildings = match possible_buildings_per_area.get_mut(&workplace_area_code) {
                Some(buildings) => buildings,
                None => {
                    error!("No Workplace buildings exist for Output Area: {}",workplace_area_code);
                    continue 'citizen_allocation_loop;
                }
            };

            match self.assign_buildings_per_output_area(workplace_area_code.clone(), citizens, possible_buildings)
            {
                Ok(buildings) => {
                    match self
                        .output_areas
                        .get_mut(&workplace_area_code)
                        .ok_or_else(|| DataLoadingError::ValueParsingError {
                            source: ParseErrorType::MissingKey {
                                context: "Retrieving output area for building workplaces ".to_string(),
                                key: workplace_area_code.to_string(),
                            },
                        }) {
                        Ok(workplace_output_area) => workplace_output_area.buildings.extend(buildings),
                        Err(e) => error!("Failed to retrieve workplace output area: {}, {}",workplace_area_code,e),
                    }
                }
                Err(e) => {
                    error!("Failed to assign buildings: {}",e);
                }
            }
        }
        Ok(())
    }

    fn assign_buildings_per_output_area(&mut self, workplace_area_code: OutputAreaID, mut citizens: Vec<CitizenID>, possible_buildings: &mut Vec<RawBuilding>) -> anyhow::Result<HashMap<BuildingID, Box<dyn Building>>> {
        let mut rng = thread_rng();
        // This is the amount to increase bin capacity to ensure it meets the minimum required size
        const building_per_occupation_overcapacity: f64 = 0.2;

        // Randomise the order of the citizens, to reduce the number of Citizens sharing household and Workplace output areas
        citizens.shuffle(&mut rng);


        // Extract the Citizen Struct
        // TODO Fix the borrow of self
        let citizens: Vec<&mut Citizen> = citizens.iter().map(|citizen_id| self.citizens.get_mut(citizen_id).ok_or_else(|| {
            DataLoadingError::ValueParsingError {
                source: ParseErrorType::MissingKey {
                    context: "Cannot retrieve Citizen to assign Workplace ".to_string(),
                    key: citizen_id.to_string(),
                },
            }
        })).collect::<Result<Vec<&mut Citizen>, DataLoadingError>>()?;


        let total_building_count = possible_buildings.len();


        // Calculate how much space we have
        let available_space: usize = possible_buildings.iter().map(|building| building.size()).sum::<i32>() as usize;

        // Calculate how much space we need
        let required_space_per_occupation: HashMap<OccupationType, usize> = citizens.iter().map(|citizen| {
            let occupation = citizen.occupation();
            let size = EmploymentDensities::get_density_for_occupation(occupation);
            (occupation, size)
        }).fold(HashMap::new(), |mut a, b| {
            let entry = a.entry(b.0).or_default();
            *entry += b.1 as usize;
            a
        });
        let required_space: usize = required_space_per_occupation.values().sum();


        // Calculate how much we need to scale buildings to meet the targets
        let scale = (((required_space as f64) / (available_space as f64)) * building_per_occupation_overcapacity).ceil() as usize;

        // Allocate buildings using first fit
        let mut building_per_occupation: HashMap<OccupationType, (usize, Vec<RawBuilding>)> = HashMap::new();

        // Shuffle to ensure buildings are distrubuted accross the area
        possible_buildings.shuffle(&mut rng);
        for building in possible_buildings.into_iter() {
            let mut added = false;
            for (occupation, (current_size, buildings)) in &mut building_per_occupation {

                // If adding the building doesn't exceed the bin size, do it!
                if *current_size + (building.size() as usize * scale) < *required_space_per_occupation.get(occupation).expect("Occupation type is missing!") {
                    *current_size += (building.size() as usize * scale);
                    buildings.push(*building);
                    added = true;
                }
            }
            // TODO Make this nicer/figure out what to do here
            if !added {
                error!("Failed to add building of size: {}, current capacities: {:?}",building.size(),building_per_occupation.iter().map(| (occupation_type,(size,_))|(occupation_type,size)).collect());
            }
        }
        // Ensure we have meant the minimum requirements OR TODO Allow some overflow to remote work?
        for (occupation_type, (size, buildings)) in &building_per_occupation {
            let required = required_space_per_occupation.get(occupation_type).expect("Occupation type is missing!");
            assert!(size > required, "Occupation: {:?}, has a size {} smaller than required {}", occupation_type, size, required);
        }

        // TODO Assign Citizens to each building per Occupation Count
        // TODO Maybe parrelise this?
        return Ok(HashMap::new());/*
        let mut possible_buildings = possible_buildings.into_iter();
        let total_workers = citizens.len();

        // This is the Workplace list to allocate citizens to
        let mut current_workplaces_to_allocate: HashMap<OccupationType, Workplace> = HashMap::new();

        // This is the list of full workplaces that need to be added to the parent Output Area
        let mut workplace_buildings: HashMap<BuildingID, Box<dyn Building>> = HashMap::new();


        for (index, citizen_id) in citizens.iter_mut().enumerate() {
            let building_index = rng.gen_range(0..possible_buildings.len());


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
            let assign_workplace: Result<Workplace, anyhow::Error> = (|| {
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
                                    *possible_buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces {} to assign workers ({}/{}) to in Output Area: {}", total_building_count, index, total_workers, workplace_area_code) })?,
                                    citizen.occupation());
                                workplace.add_citizen(citizen.id()).context(
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
                            .add_citizen(citizen.id())
                            .context("Cannot add Citizen to new workplace!")?;
                        workplace
                    }
                };
                Ok(workplace)
            })();
            match assign_workplace {
                Ok(workplace) => {
                    citizen.set_workplace_code(workplace.id().clone());
                    // Add the unfilled Workplace back to the allocator
                    current_workplaces_to_allocate.insert(citizen.occupation(), workplace);
                }
                Err(e) => {
                    error!("Failed to assign workplace: {}",e);
                }
            }
        }
        // Add any leftover Workplaces to the Output Area
        current_workplaces_to_allocate
            .drain()
            .for_each(|(_, workplace)| {
                workplace_buildings.insert(workplace.id().clone(), Box::new(workplace));
            });
        Ok(workplace_buildings)*/
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
        debug!(
            "There are {} areas with workplace buildings",
            possible_workplaces.len()
        );
        for (id, _) in &self.output_areas {
            println!("{} has {} buildings ", &id, possible_workplaces.get(&id).map(|f| f.len()).unwrap_or(0));
        }

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
pub fn parallel_assign_buildings_to_output_areas(
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
