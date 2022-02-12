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
use std::default::Default;
use std::fmt::format;
use std::fs::File;
use std::rc::Rc;

use anyhow::Context;
use geo_types::Point;
use log::{debug, error, info, warn};
use num_format::ToFormattedString;
use rand::{RngCore, thread_rng};
use rand::prelude::{IteratorRandom, SliceRandom};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use strum::IntoEnumIterator;

use load_census_data::CensusData;
use load_census_data::parsing_error::{DataLoadingError, ParseErrorType};
use osm_data::{
    BuildingBoundaryID, OSMRawBuildings, RawBuilding, TagClassifiedBuilding,
};
use osm_data::polygon_lookup::PolygonContainer;

use crate::config::{MAX_STUDENT_AGE, NUMBER_FORMATTING};
use crate::config::STARTING_INFECTED_COUNT;
use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::error::SimError;
use crate::models::building::{AVERAGE_CLASS_SIZE, Building, BuildingID, BuildingType, School, Workplace};
use crate::models::citizen::{Citizen, CitizenID, OccupationType};
use crate::models::get_density_for_occupation;
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
        for entry in &self.census_data.valid_areas {
            let output_id = OutputAreaID::from_code(entry.to_string());
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
    ///
    /// Note that Schools, are not returned, because they are built from the Voronoi Diagrams
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
        let census_data_ref = &mut self.census_data;
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
        for occupation in OccupationType::iter() {
            debug!("There are {} Citizens with an occupation of: {:?}", citizens.iter().filter(|(_, citizen)| citizen.detailed_occupation() == Some(occupation)).count(), occupation);
        }
        let mut ages = vec![0; 101];
        for citizen in &citizens {
            ages[citizen.1.age as usize] += 1;
        }
        self.citizens = citizens;
        Ok(())
    }

    pub fn build_schools(&mut self) -> anyhow::Result<()> {
        debug!("Building Schools");
        // TODO Maybe we need to shuffle?
        // The outer index represents the age of the students, and the inner is just a list of students
        let mut teacher_count = 0;
        let (students, teachers): (Vec<Vec<&mut Citizen>>, Vec<&mut Citizen>) = self.citizens.iter_mut().filter_map(|(_id, citizen)| {
            let age = citizen.age;
            if age < MAX_STUDENT_AGE {
                Some((Some((citizen.age, citizen)), None))
            } else if Some(OccupationType::Teaching) == citizen.detailed_occupation() {
                teacher_count += 1;
                Some((None, Some(citizen)))
            } else { None }
        }).fold({
                    let mut data = Vec::new();
                    for _ in 0..MAX_STUDENT_AGE {
                        data.push(Vec::new());
                    }
                    (data, Vec::new())
                }, |mut acc, (student, teacher)| {
            if let Some((age, id)) = student {
                acc.0[age as usize].push(id);
            } else if let Some(id) = teacher {
                acc.1.push(id)
            }
            acc
        });
        debug!("{} teachers retrieved",teacher_count);
        debug!("There are {} age groups, with {} students and {} teachers",students.len(),students.iter().map(|age_group|age_group.len()).sum::<usize>(),teachers.len());
        // The OSM Voronoi School Lookup
        let school_lookup = self.osm_data.voronoi().get(&TagClassifiedBuilding::School).expect("No schools exist!");
        // Function to find the the closest school, to the given citizen
        let building_locations = self.osm_data.building_locations.get(&TagClassifiedBuilding::School).ok_or_else(|| SimError::InitializationError { message: format!("Couldn't retrieve school buildings!") })?;
        debug!("There are {} raw schools",building_locations.len());

        let output_areas = &mut self.output_areas;


        let finding_closest_school = |citizen: &Citizen, get_multiple: bool| -> Result<Vec<&RawBuilding>, SimError> {
            let area_code = citizen.household_code.output_area_code();
            let area = output_areas.get(&area_code).ok_or_else(|| SimError::InitializationError { message: format!("Couldn't retrieve output area: {}", area_code) })?;

            let building = area.buildings.get(&citizen.household_code).ok_or_else(|| SimError::InitializationError { message: format!("Couldn't retrieve household home: {}", citizen.household_code) })?;
            Ok(if get_multiple {
                let closest_schools_index = school_lookup.find_seeds_for_point(building.get_location())?;
                closest_schools_index.into_iter().filter_map(|index| building_locations.get(index)).collect()
            } else {
                vec![building_locations.get(school_lookup.find_seed_for_point(building.get_location())?).unwrap()]
            })
        };


        // Groups the students/teachers, by the school they are closest to
        // The geo point is the key, because it is the only identifier we can use

        let mut student_school: HashMap<String, u32> = HashMap::new();
        let mut citizens_per_raw_school = students.into_iter().enumerate().map(|(age, student)|
            {
                let age_grouped_students_per_school = student.into_iter().filter_map(|student| {
                    match finding_closest_school(student, false) {
                        Ok(schools) => {
                            if let Some(school) = schools.first() {
                                let mut entry = student_school.entry(format!("{:?}", school.center().x_y())).or_default();
                                *entry += 1;
                                Some((*school, student))
                            } else {
                                warn!("No schools available");
                                None
                            }
                        }
                        Err(e) => {
                            warn!("Failed to assign school to student: {}",e);
                            None
                        }
                    }
                }).fold(HashMap::new(), |mut acc: HashMap<geo_types::Point<i32>, (Vec<&mut Citizen>, &RawBuilding)>, (school, student): (&RawBuilding, &mut Citizen)| {
                    let (entry, _) = acc.entry(school.center()).or_insert_with(|| (Vec::new(), school));
                    entry.push(student);
                    acc
                });
                (age, age_grouped_students_per_school)
            }
        ).fold(HashMap::new(), |mut acc: HashMap<Point<i32>, (Vec<Vec<&mut Citizen>>, Vec<&mut Citizen>, &RawBuilding)>, (age, schools_to_flatten): (usize, HashMap<Point<i32>, (Vec<&mut Citizen>, &RawBuilding)>)| {
            schools_to_flatten.into_iter().for_each(|(key, (students, school))| {
                let (entry, _, _) = acc.entry(key).or_insert_with(|| (Vec::new(), Vec::new(), school));
                while entry.len() < age + 1 {
                    entry.push(Vec::new());
                }
                let age_group_len = entry.len();
                let age_group = entry.get_mut(age).unwrap_or_else(|| panic!("Cannot retrieve age vector that has been generated! Age: {}, Age Group Len: {}", age, age_group_len));
                age_group.extend(students);
            });
            acc
        });
        info!("Assigned Students to {} raw schools",citizens_per_raw_school.len());
        let mut teacher_school: HashMap<String, u32> = HashMap::new();
        let mut failed_count = 0;
        let mut school_full = 0;
        let mut school_not_found = 0;
        let mut teacher_school_possibilites = Vec::with_capacity(20000);
        for (index, teacher) in teachers.into_iter().enumerate() {
            let mut placed = false;
            match finding_closest_school(teacher, true) {
                Ok(schools) => {
                    let mut hashed_schools = HashSet::with_capacity(200);
                    schools.iter().for_each(|school| { (hashed_schools.insert(school.center())); });
                    //assert_eq!(hashed_schools.len(), schools.len());
                    teacher_school_possibilites.push(hashed_schools.len());
                    for school in schools {
                        let school_entry = citizens_per_raw_school.get_mut(&school.center());
                        if let Some((students, teachers, _)) = school_entry {
                            let total_students = students.iter().map(|age_group| ((age_group.len() as f64 / AVERAGE_CLASS_SIZE).ceil() as usize).max(1)).sum::<usize>();
                            if teachers.len() < total_students {
                                teachers.push(teacher);
                                let mut entry = teacher_school.entry(format!("{:?}", school.center().x_y())).or_default();
                                *entry += 1;
                                placed = true;
                                break;
                            } else {
                                school_full += 1;
                            }
                        } else {
                            school_not_found += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to assign school to teacher: {}",e);
                }
            }
            if index % 5000 == 0 {
                debug!("On index {} for assigning teachers. {} have failed. {} schools are full, {} schools don't exist",index,failed_count,school_full,school_not_found);
            }
            if !placed {
                failed_count += 1;
            }
        }


        let mut file = File::create("../../debug_dumps/teacher_school_possibilities.json");
        serde_json::to_writer(file.unwrap(), &teacher_school_possibilites).unwrap();
        let mut file = File::create("../../debug_dumps/student_schools.json");
        serde_json::to_writer(file.unwrap(), &student_school).unwrap();
        let mut file = File::create("../../debug_dumps/teacher_schools.json");
        serde_json::to_writer(file.unwrap(), &teacher_school).unwrap();
        let mut file = File::create("../../debug_dumps/teacher_schools.json");
        serde_json::to_writer(file.unwrap(), &teacher_school).unwrap();
        info!("Assigned teachers to schools");
        warn!("Failed to assign schools to {} teachers",failed_count);

        let building_boundaries = &self.osm_data.building_boundaries;
        let output_areas_polygons = &self.output_areas_polygons;

        let mut class_total = 0;
        let mut teachers_total = 0;
        let mut students_total = 0;
        let mut schools_total = 0;

        let mut schools_missing_teachers = 0;

        let mut debug_stats = HashMap::with_capacity(citizens_per_raw_school.len());


        citizens_per_raw_school.into_iter().for_each(|(key, (students, teachers, building))|
            {
                students_total += students.iter().flatten().collect::<Vec<&&mut Citizen>>().len();
                teachers_total += teachers.len();
                if teachers_total == 0 {
                    schools_missing_teachers += 1;
                    return;
                }
                // Retrieve the Output Area, and build the School building
                let output_area_id = get_area_code_for_raw_building(building, output_areas_polygons, building_boundaries).expect("School building is not inside any Output areas!").keys().next().expect("School building is not inside any Output areas!").clone();
                let building_id = BuildingID::new(output_area_id.clone(), BuildingType::School);

                // Pull the Citizen ID's out of the Citizens
                let student_ids = students.iter().map(|age_group|
                    age_group.iter().map(|student|
                        student.id()).collect()).collect();

                let teacher_ids = teachers.iter().map(|citizen| citizen.id()).collect();

                let (school, stats) = School::with_students_and_teachers(building_id.clone(), *building, student_ids, teacher_ids);
                debug_stats.insert(format!("({},{}", school.get_location().x(), school.get_location().y()), stats);
                class_total += school.classes().len();
                schools_total += 1;


                let output_area = output_areas.get_mut(&output_area_id).ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Retrieving output area for schools".to_string(),
                        key: output_area_id.to_string(),
                    },
                });
                match output_area {
                    Ok(output_area) => { output_area.buildings.insert(building_id.clone(), Box::new(school)); }
                    Err(e) => { error!("{}",e) }
                }

                // Assign students and teacher to school workplace
                for age_group in students {
                    for student in age_group {
                        student.workplace_code = building_id.clone();
                    }
                }
                for teacher in teachers {
                    teacher.workplace_code = building_id.clone();
                }
            }
        );


        let mut file = File::create("../../debug_dumps/pre_duplicate_removal/school_statistics.json");
        serde_json::to_writer(file.unwrap(), &debug_stats).unwrap();
        warn!("{} schools are missing teachers",schools_missing_teachers);
        info!("Generated {} schools, with {} teachers, {} students across {} classes, with avg class size {} and avg classes per school {}",schools_total,teachers_total,students_total,class_total,(students_total/class_total),(class_total/schools_total));
        panic!("Built schools!");
        Ok(())
    }

    /// Iterates through all Output Areas, and All Citizens in that Output Area
    ///
    /// Picks a Workplace Output Area, determined from Census Data Distribution
    ///
    /// Allocates that Citizen to the Workplace Building in that chosen Output Area
    pub fn build_workplaces(
        &mut self,
        mut possible_buildings_per_area: HashMap<OutputAreaID, Vec<RawBuilding>>,
    ) -> anyhow::Result<()> {
        debug!(
            "Assigning workplaces to {} output areas ",
            self.output_areas.len()
        );
        // Shuffle the buildings
        possible_buildings_per_area.par_iter_mut().for_each(|(_, buildings)| buildings.shuffle(&mut thread_rng()));

        // Group Citizens by their workplace output area
        // NOTE This is achieved by removing citizens from self.citizens, because we cannot pass references through
        let mut citizens_to_allocate: HashMap<OutputAreaID, HashMap<CitizenID, Citizen>> =
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
                        // TODO Redo this
                        let code = household_census_data.get_random_workplace_area(&mut thread_rng()).context("Failed to retrieve random workplace area")?;
                        let code = Some(OutputAreaID::from_code(code));
                        let code = if let Some(code) = code {
                            if !self.output_areas.contains_key(&code) {
                                //warn!("Output area: {} doesn't exist!",output_area_code);
                                None
                            } else if possible_buildings_per_area.get(&code).is_none() {
                                //warn!("Buildings don't exist for area: {}",output_area_code);
                                None
                            } else {
                                Some(code)
                            }
                        } else { None };
                        if let Some(code) = code {
                            break code;
                        }
                        index += 1;
                        if index > 50 {
                            error!("Failed to generate code for Citizen: {}",citizen_id);
                            continue 'citizens;
                        }
                    };
                let citizens_to_add = citizens_to_allocate.entry(workplace_output_area_code).or_default();
                citizens_to_add.insert(citizen_id, self.citizens.remove(&citizen_id).expect(format!("Citizen {} does not exist!", citizen_id).as_str()));
            }
        }
        debug!("Creating workplace buildings for: {:?} Citizens and {} Output Areas",citizens_to_allocate.iter().map(|(_,citizens)|citizens.len()).sum::<usize>(),citizens_to_allocate.len());
        debug!("{} Citizens have not been assigned a workplace area!", self.citizens.len());
        // Create buildings for each Workplace output area
        'citizen_allocation_loop: for (workplace_area_code, mut citizens) in citizens_to_allocate {
            // Retrieve the buildings or skip this area
            let possible_buildings = match possible_buildings_per_area.get_mut(&workplace_area_code) {
                Some(buildings) => buildings,
                None => {
                    error!("No Workplace buildings exist for Output Area: {}",workplace_area_code);
                    continue 'citizen_allocation_loop;
                }
            };

            match SimulatorBuilder::assign_buildings_per_output_area(workplace_area_code.clone(), citizens.values_mut().collect(), possible_buildings)
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
                        Err(e) => error!("Failed to retrieve workplace output area: {}, Error: {}",workplace_area_code,e),
                    }
                }
                Err(e) => {
                    error!("Failed to assign buildings for Output Area: {}, Error: {}",workplace_area_code,e);
                }
            }
            // Add the Citizens back to self
            citizens.into_iter().for_each(|(id, citizen)| { if self.citizens.insert(id, citizen).is_some() { panic!("Citizen {} has been duplicated", id); } });
        }
        Ok(())
    }

    /// Calculates which buildings should be assigned to what occupation, and scales the floor space, to ensure every Citizen can have a workplace
    fn assign_buildings_per_output_area(workplace_area_code: OutputAreaID, mut citizens: Vec<&mut Citizen>, possible_buildings: &mut Vec<RawBuilding>) -> anyhow::Result<HashMap<BuildingID, Box<dyn Building>>> {
        if citizens.len() == 0 {
            warn!("No buildings can be assigned to area {} as no workers exist: {:?}",workplace_area_code,citizens);
            return Ok(HashMap::new());
        }
        if possible_buildings.len() == 0 {
            warn!("No buildings can be assigned to area {} as no buildings exist: {:?}",workplace_area_code,possible_buildings);
            return Ok(HashMap::new());
        }
        // TODO Need to fix what happens if not enough buildings
        if possible_buildings.len() < OccupationType::iter().len() * 2 {
            warn!("Not enough buildings {} for {} occupations for area: {}",possible_buildings.len(), OccupationType::iter().len(),workplace_area_code);
        }


        let mut rng = thread_rng();
        // This is the amount to increase bin capacity to ensure it meets the minimum required size
        const BUILDING_PER_OCCUPATION_OVERCAPACITY: f64 = 1.1;

        // Randomise the order of the citizens, to reduce the number of Citizens sharing household and Workplace output areas
        citizens.shuffle(&mut rng);

        // Group by occupation
        let mut citizens: HashMap<OccupationType, Vec<&mut Citizen>> = citizens.into_iter().filter_map(|citizen| {
            Some((citizen.detailed_occupation()?, citizen))
        }).fold(HashMap::new(), |mut a, b| {
            let entry = a.entry(b.0).or_default();
            entry.push(b.1);
            a
        });

        // Calculate how much space we have
        let available_space: usize = possible_buildings.iter().map(|building| building.size()).sum::<i32>() as usize;

        // Calculate how much space we need
        let mut required_space_per_occupation: HashMap<OccupationType, usize> = citizens.iter().map(|(occupation, citizens)| {
            let size = get_density_for_occupation(*occupation);
            (occupation, size * citizens.len() as u32)
        }).fold(HashMap::new(), |mut a, b| {
            let entry = a.entry(*b.0).or_default();
            *entry += b.1 as usize;
            a
        });

        // Add any missing Occupations
        OccupationType::iter().for_each(|occupation| if !required_space_per_occupation.contains_key(&occupation) {
            required_space_per_occupation.insert(occupation, 0);
        });
        let required_space: usize = required_space_per_occupation.values().sum();


        // Calculate how much we need to scale buildings to meet the targets
        let scale = (((required_space as f64) / (available_space as f64)) * BUILDING_PER_OCCUPATION_OVERCAPACITY).ceil() as usize;
        //trace!("Scale for Output Area: {} is {} with {} buildings and {} Workers",workplace_area_code,scale,possible_buildings.len(),total_workers);
        // Allocate buildings using first fit

        // Occupation Type, (Current Total Floor Space, The list of buildings to be generated)
        let mut building_per_occupation: HashMap<OccupationType, (usize, Vec<RawBuilding>)> = OccupationType::iter().map(|occupation| (occupation, (0, Vec::with_capacity(1000)))).collect();
        let mut differences: HashMap<OccupationType, isize> = required_space_per_occupation.iter().map(|(occupation, size)| (*occupation, *size as isize)).collect();
        // Shuffle to ensure buildings are distributed across the area
        possible_buildings.shuffle(&mut rng);
        for building in possible_buildings.into_iter() {
            let mut added = false;
            let building_size = building.size() as usize * scale;
            if building_size == 0 {
                continue;
            }

            // Add the building to the first Occupation group, where it won't exceed the required size
            for (occupation, (current_size, buildings)) in &mut building_per_occupation {
                // If adding the building doesn't exceed the bin size, do it!
                if *current_size + building_size < *required_space_per_occupation.get(occupation).expect("Occupation type is missing!") {
                    *current_size += building_size;
                    buildings.push(*building);
                    *differences.get_mut(occupation).expect("") -= building_size as isize;
                    added = true;
                }
            }
            // If cannot be added to any buildings without overflowing the size, then add to occupation that overflows least
            if !added {
                // Find the building that overflows least
                let mut min_occupation = None;
                let mut min_diff = isize::MAX;
                for (occupation, diff) in &differences {
                    if 0 < *diff && *diff < min_diff {
                        min_diff = *diff;
                        min_occupation = Some(*occupation);
                    }
                }
                // Add to that building
                match min_occupation {
                    Some(occupation) => {
                        let (size, buildings) = building_per_occupation.get_mut(&occupation).expect("");
                        *size += building_size;
                        buildings.push(*building);
                        *differences.get_mut(&occupation).expect("") -= building_size as isize;
                    }
                    None => {
                        //error!("Failed to add building of size: {}, \nCurrent Capacities: {:?}\nRequired Sizes: {:?}",building.size()*(scale as i32),building_per_occupation.iter().map(| (occupation_type,(size,_))|(*occupation_type,*size)).collect::<HashMap<OccupationType,usize>>(),required_space_per_occupation);
                    }
                }
            }
        }
        // Ensure we have meant the minimum requirements
        for (occupation_type, (size, _buildings)) in &building_per_occupation {
            let required = required_space_per_occupation.get(occupation_type).expect("Occupation type is missing!");
            if size <= required {
                warn!( "Occupation: {:?}, has a size {} smaller than required {} for area {}",occupation_type, size, required,  workplace_area_code);
                //\nCurrent Capacities: {:?}\nRequired Sizes: {:?}", , required, building_per_occupation.iter().map(|(occupation_type, (size, _))| (*occupation_type, *size)).collect::<HashMap<OccupationType, usize>>(), required_space_per_occupation);
                //return Ok(HashMap::new());
            }
        }


        // This is the list of full workplaces that need to be added to the parent Output Area
        let mut workplace_buildings: HashMap<BuildingID, Box<dyn Building>> = HashMap::new();

        // Assign workplaces to every Citizen
        // TODO Parallelise
        let keys = citizens.keys().cloned().collect::<Vec<OccupationType>>();
        for occupation in keys {
            // Teaching is handled in build schools
            if occupation == OccupationType::Teaching {
                continue;
            }
            let citizens = citizens.get_mut(&occupation).expect(format!("Couldn't get Citizens with occupation: {:?}", occupation).as_str());
            let buildings = building_per_occupation.get_mut(&occupation).expect(format!("Couldn't get Citizens with occupation: {:?}", occupation).as_str());
            let workplaces = SimulatorBuilder::assign_workplaces_to_citizens_per_occupation(workplace_area_code.clone(), occupation, citizens, &buildings.1);


            if let Err(e) = workplaces {
                error!("Failed to assign workplaces to Citizens for Occupation: {:?},\n\tError: {:?}",occupation,e);
                continue;
            }
            let mut workplaces = workplaces.unwrap();
            workplaces.drain()
                .for_each(|(id, workplace)| {
                    if workplace_buildings.insert(id, workplace).is_some() { panic!("Two Workplaces exist with the same Building ID!"); }
                });
        }
        Ok(workplace_buildings)
    }

    /// Assigns Each Citizen to one of the Given RawBuildings and transforms the RawBuildings into Workplaces
    ///
    /// Note that each Citizen should have the same Occupation
    fn assign_workplaces_to_citizens_per_occupation(workplace_area_code: OutputAreaID, occupation: OccupationType, citizens: &mut Vec<&mut Citizen>, buildings: &Vec<RawBuilding>) -> anyhow::Result<HashMap<BuildingID, Box<dyn Building>>> {
        let total_building_count = buildings.len();
        let total_workers = citizens.len();
        let mut workplace_buildings: HashMap<BuildingID, Box<dyn Building>> = HashMap::new();
        let mut buildings = buildings.iter();

        let mut current_workplace: Workplace = Workplace::new(
            BuildingID::new(
                workplace_area_code.clone(),
                BuildingType::Workplace,
            ),
            *buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces ({}) to assign workers ({}/{}) to in Output Area: {}", total_building_count, 0, total_workers, workplace_area_code) })?,
            occupation);
        for (index, citizen) in citizens.iter_mut().enumerate() {
            assert_eq!(citizen.detailed_occupation().unwrap(), occupation, "Citizen does not have the specified occupation!");
            // 2 Cases
            // Citizen can be added:
            //      Add Citizen to it
            // Citizen cannot be added:
            //      Save the current Workplace
            //      Generate a new Workplace
            //      Add a Citizen to the new Workplace
            current_workplace =
                match current_workplace.add_citizen(citizen.id()) {
                    Ok(_) => current_workplace,
                    Err(_) => {
                        workplace_buildings
                            .insert(current_workplace.id().clone(), Box::new(current_workplace));

                        let new_raw_building = match buildings.next() {
                            Some(building) => *building,
                            None => {
                                error!("Ran out of Workplaces {} to assign workers ({}/{}) to in Output Area: {}", total_building_count, index, total_workers, workplace_area_code);
                                return Ok(workplace_buildings);
                            }
                        };

                        let mut new_workplace = Workplace::new(
                            BuildingID::new(
                                workplace_area_code.clone(),
                                BuildingType::Workplace,
                            ),
                            new_raw_building,
                            occupation);
                        new_workplace.add_citizen(citizen.id()).context(
                            "Cannot add Citizen to freshly generated Workplace!",
                        )?;
                        new_workplace
                    }
                };
            citizen.set_workplace_code(current_workplace.id().clone());
        }
        workplace_buildings.insert(current_workplace.id().clone(), Box::new(current_workplace));
        Ok(workplace_buildings)
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

        self.build_schools()
            .context("Failed to build schools")?;

        timer.code_block_finished(&format!(
            "Built schools",
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
        self.build_workplaces(possible_workplaces)
            .context("Failed to build workplaces")?;
        timer.code_block_finished("Generated workplaces for {} Output Areas")?;

        let work_from_home_count: u32 = self.citizens.par_iter().map(|(_, citizen)| if citizen.household_code.eq(&citizen.workplace_code) { 1 } else { 0 }).sum();
        debug!("{} out of {} Citizens {:.1}%, are working from home.",work_from_home_count.to_formatted_string(&NUMBER_FORMATTING),self.citizens.len().to_formatted_string(&NUMBER_FORMATTING),(work_from_home_count as f64/self.citizens.len() as f64)*100.0);
        // Infect random citizens
        self.apply_initial_infections(&mut rng)
            .context("Failed to create initial infections")?;

        timer.code_block_finished("Applied initial infections")?;
        debug!(
            "Starting Statistics: There are {} total Citizens, {} Output Areas",
            self.citizens.len().to_formatted_string(&NUMBER_FORMATTING),
            self.output_areas.len().to_formatted_string(&NUMBER_FORMATTING)
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

/// Returns a list of Output Areas that the given building is inside
///
/// If the building is in multiple Areas, it is duplicated
fn get_area_code_for_raw_building(building: &RawBuilding, output_area_lookup: &PolygonContainer<String>, building_boundaries: &HashMap<BuildingBoundaryID, geo_types::Polygon<i32>>) -> Option<HashMap<OutputAreaID, Vec<RawBuilding>>> {
    let boundary = building_boundaries.get(&building.boundary_id());
    if let Some(boundary) = boundary {
        if let Ok(areas) = output_area_lookup.find_polygons_containing_polygon(boundary) {
            let area_locations = areas.map(|area|
                OutputAreaID::from_code(area.to_string()))
                .zip(std::iter::repeat(vec![*building]))
                .collect::<HashMap<OutputAreaID, Vec<RawBuilding>>>();
            return Some(area_locations);
        }
    } else {
        warn!("Raw Building is missing Boundary with id: {:?}", building.boundary_id());
    }
    None
}

/// On csgpu2 with 20? threads took 11 seconds as oppose to 57 seconds for single threaded version
pub fn parallel_assign_buildings_to_output_areas(
    building_boundaries: &HashMap<BuildingBoundaryID, geo_types::Polygon<i32>>,
    building_locations: &HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
    output_area_lookup: &PolygonContainer<String>,
) -> HashMap<OutputAreaID, HashMap<TagClassifiedBuilding, Vec<RawBuilding>>> {
    building_locations.into_par_iter().filter_map(|(building_type, possible_building_locations)|
        {
            if TagClassifiedBuilding::School == *building_type {
                return None;
            }
            // Try find Area Codes for the given building
            let area_codes = possible_building_locations.into_par_iter().filter_map(|building| {
                get_area_code_for_raw_building(building, output_area_lookup, building_boundaries)
            });
            // Group By Area Code
            let area_codes = area_codes.reduce(HashMap::new, |mut a, b| {
                for (area, area_buildings) in b {
                    let area_entry = a.entry(area).or_default();
                    area_entry.extend(area_buildings)
                }
                a
            });
            Some((*building_type, area_codes))
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
