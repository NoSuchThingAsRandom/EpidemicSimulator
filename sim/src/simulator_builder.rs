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
use std::fs;
use std::fs::File;
use std::rc::Rc;

use anyhow::Context;
use enum_map::EnumMap;
use geo_types::{Coordinate, Point};
use log::{debug, error, info, warn};
use num_format::ToFormattedString;
use rand::{RngCore, thread_rng};
use rand::prelude::{IteratorRandom, SliceRandom};
use rayon::prelude::{IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
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
use crate::models::citizen::{Citizen, CitizenID, Occupation, OccupationType};
use crate::models::get_density_for_occupation;
use crate::models::output_area::{OutputArea, OutputAreaID};
use crate::simulator::Timer;

pub struct SimulatorBuilder {
    census_data: CensusData,
    osm_data: OSMRawBuildings,
    pub output_areas: Vec<OutputArea>,
    /// This maps the String code of an Output Area to it's index
    pub output_area_lookup: HashMap<String, u32>,
    output_areas_polygons: PolygonContainer<String>,
    pub disease_model: DiseaseModel,
    pub citizen_output_area_lookup: Vec<OutputAreaID>,
}

/// Initialisation Methods
impl SimulatorBuilder {
    /// Generates the Output Area Structs, from the Census Data
    ///
    /// And returns the starting population count
    pub fn initialise_output_areas(&mut self) -> anyhow::Result<()> {
        // Build the initial Output Areas and Households
        for entry in &self.census_data.valid_areas {
            let output_id = OutputAreaID::from_code_and_index(entry.to_string(), self.output_areas.len() as u32);
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
            self.output_area_lookup.insert(output_id.code().clone(), output_id.index() as u32);
            let new_area = OutputArea::new(
                output_id,
                polygon.clone(),
                self.disease_model.mask_percentage,
            )
                .context("Failed to create Output Area")?;
            self.output_areas.push(new_area);
        }
        Ok(())
    }

    /// Assigns buildings to their enclosing Output Area, and Removes Output Areas that do not have any buildings
    ///
    /// Note that Schools, are not returned, because they are built from the Voronoi Diagrams
    pub fn assign_buildings_to_output_areas(
        &mut self,
    ) -> anyhow::Result<HashMap<String, HashMap<TagClassifiedBuilding, Vec<RawBuilding>>>>
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
        // Count the number of buildings generated
        let count: usize = possible_buildings_per_area
            .par_iter()
            .map(|(_, classed_building)| {
                classed_building
                    .par_iter()
                    .map(|(_, buildings)| buildings.len())
                    .sum::<usize>()
            })
            .sum();

        let mut output_areas = &mut self.output_areas;
        // TODO This is broke
        // Remove any areas without any buildings
        let to_delete: Vec<usize> = output_areas.iter().enumerate()
            .filter_map(|(index, area)| if !possible_buildings_per_area.contains_key(area.id().code()) {
                Some(index)
            } else { None }
            ).collect();
        for deletion in &to_delete {
            for index in *deletion..output_areas.len() {
                let mut area = output_areas.get_mut(index).unwrap();
                area.decrement_index();
            }
        }
        for (index, deletion) in to_delete.iter().enumerate() {
            println!("Removing: {}", deletion - index);
            self.output_areas.remove(deletion - index);
        }
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
            String,
            HashMap<TagClassifiedBuilding, Vec<RawBuilding>>,
        >,
    ) -> anyhow::Result<()> {
        let mut no_buildings = 0;
        let mut no_households = 0;
        // Generate Citizens

        // This ref self is needed, because we have a mut borrow (Output Areas) and an immutable borrow (Census Data)
        // TODO This is super hacky and I hate it
        let mut citizen_output_area_lookup = &mut self.citizen_output_area_lookup;
        let ref_output_areas = Rc::new(RefCell::new(&mut self.output_areas));
        let census_data_ref = &mut self.census_data;
        // This is the total Citizen counter
        let mut global_citizen_index = 0;
        ref_output_areas.borrow_mut().iter_mut().for_each(|output_area| {
            if let Err(e) = || -> anyhow::Result<()> {
                // Retrieve the Census Data
                let census_data_entry = census_data_ref
                    .for_output_area_code(output_area.id().code().to_string())
                    .ok_or_else(|| SimError::InitializationError {
                        message: format!(
                            "Cannot generate Citizens for Output Area {} as no Census Data exists",
                            output_area.id()
                        ),
                    })?;
                // Extract the possible buildings for this Output Area
                let possible_buildings = possible_buildings_per_area
                    .get_mut(output_area.id().code())
                    .ok_or_else(|| {
                        no_buildings += 1;
                        return SimError::InitializationError {
                            message: format!(
                                "Cannot generate Citizens for Output Area {} as no buildings exist",
                                output_area.id()
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
                                output_area.id()
                            ),
                        };
                    })?;
                let generated_count = output_area.generate_citizens_with_households(
                    global_citizen_index,
                    rng,
                    census_data_entry,
                    possible_households,
                )?;
                global_citizen_index += generated_count;
                for _citizen in &output_area.citizens {
                    citizen_output_area_lookup.push(output_area.id().clone());
                }
                assert_eq!(global_citizen_index as usize, citizen_output_area_lookup.len());
                Ok(())
            }() { error!("{:?}",e); }
        });
        info!(
            "Households and Citizen generation succeeded for {} Output Areas.",
            ref_output_areas.borrow().len()
        );
        if no_households > 0 {
            warn!("Failed to generate households for {} Output Areas, as no homes exist!",no_households);
        }
        if no_buildings > 0 {
            warn!("Failed to generate households for {} Output Areas, as no buildings exist!",no_buildings);
        }
        Ok(())
    }

    pub fn build_schools(&mut self) -> anyhow::Result<()> {
        debug!("Building Schools");
        // TODO Maybe we need to shuffle?
        // The outer index represents the age of the students, and the inner is just a list of students
        let (mut output_area_citizens, mut output_area_buildings, output_area_ids) = self.output_areas.iter_mut().map(|area| (&mut area.citizens, &mut area.buildings, &mut area.output_area_id)).fold((Vec::new(), Vec::new(), Vec::new()), |(mut accum_citizens, mut accum_buildings, mut accum_ids), (citizens, buildings, id)| {
            accum_citizens.push(citizens);
            accum_buildings.push(buildings);
            accum_ids.push(id);
            (accum_citizens, accum_buildings, accum_ids)
        });
        let (students, teachers): (Vec<Vec<&mut Citizen>>, Vec<&mut Citizen>) =
            output_area_citizens.par_iter_mut().map(|area_citizens| {
                let children = area_citizens.iter_mut().filter_map(|citizen| {
                    let age = citizen.age;
                    if age < MAX_STUDENT_AGE {
                        Some((Some((citizen.age, citizen)), None))
                    } else if Some(OccupationType::Teaching) == citizen.detailed_occupation() {
                        Some((None, Some(citizen)))
                    } else { None }
                }).fold({
                                                           let mut students = Vec::new();
                                                           for _ in 0..MAX_STUDENT_AGE {
                                                               students.push(Vec::new());
                                                           }
                                                           (students, Vec::new())
                                                       }, |(mut students, mut teachers), (student, teacher)| {
                    if let Some((age, id)) = student {
                        students[age as usize].push(id);
                    } else if let Some(id) = teacher {
                        teachers.push(id)
                    }
                    (students, teachers)
                });
                return children;
            }).reduce(|| (Vec::new(), Vec::new()), |(mut accum_students, mut accum_teachers), (item_students, item_teachers)| {
                accum_teachers.extend(item_teachers);
                for (age, students) in item_students.into_iter().enumerate() {
                    if accum_students.len() <= age {
                        accum_students.push(students);
                    } else {
                        accum_students.get_mut(age).unwrap().extend(students);
                    }
                }
                (accum_students, accum_teachers)
            });
        debug!("{} teachers retrieved",teachers.len());
        debug!("There are {} age groups, with {} students and {} teachers",students.len(),students.iter().map(|age_group|age_group.len()).sum::<usize>(),teachers.len());
        // The OSM Voronoi School Lookup
        let school_lookup = self.osm_data.voronoi().get(&TagClassifiedBuilding::School).expect("No schools exist!");
        // Function to find the the closest school, to the given citizen
        let building_locations = self.osm_data.building_locations.get(&TagClassifiedBuilding::School).ok_or_else(|| SimError::InitializationError {
            message: format!("Couldn't retrieve school buildings!")
        })?;
        debug!("There are {} raw schools",building_locations.len());

        let all_boundaries = &self.osm_data.building_boundaries;

        if crate::config::CREATE_DEBUG_DUMPS {
            let school_boundaries: Vec<(Point<i32>, Vec<Coordinate<i32>>)> = building_locations.iter().filter_map(|building| {
                let path = all_boundaries.get(&building.boundary_id())?;
                Some((building.center(), path.exterior().clone().0))
            }).collect();
            let debug_directory = crate::config::DEBUG_DUMP_DIRECTORY.to_owned() + "schools/";
            fs::create_dir_all(debug_directory.clone()).context("Failed to create debug dump directory")?;

            let filename = debug_directory.clone() + "raw_schools_locations.json";
            let file = File::create(filename.clone()).context(format!("Failed to create file: '{}'", filename.clone()))?;
            serde_json::to_writer(file, &school_boundaries).context("Failed to dump school boundaries to file!")?;
        }

        let output_area_lookup = &self.output_area_lookup;
        let building_boundaries = &self.osm_data.building_boundaries;
        let output_areas_polygons = &self.output_areas_polygons;
// Function to find the closest school to a given Citizen
        let finding_closest_school = |citizen: &Citizen, get_multiple: bool| -> Result<Vec<&RawBuilding>, SimError> {
            let area_code = citizen.household_code.output_area_code();
            let buildings = output_area_buildings.get(area_code.index()).ok_or_else(|| SimError::InitializationError { message: format!("Couldn't retrieve output area: {}", area_code) })?;

            let building = buildings.get(citizen.household_code.building_index()).ok_or_else(|| SimError::InitializationError { message: format!("Couldn't retrieve household home: {}", citizen.household_code) })?;
            Ok({
                let closest_schools_index = school_lookup.find_seeds_for_point(building.get_location())?;
                closest_schools_index.into_iter().filter_map(|index| {
                    let school = building_locations.get(index)?;
                    let area_codes = get_area_code_for_raw_building(school, output_areas_polygons, building_boundaries).expect("School building is not inside any Output areas!");
                    let output_area_id = area_codes.keys().next()?;
//.expect("School building is not inside any Output areas!");
                    if output_area_lookup.contains_key(output_area_id) {
                        Some(school)
                    } else {
                        None
                    }
                }).collect()
            })
        };


// Groups the students/teachers, by the school they are closest to
// The geo point is the key, because it is the only identifier we can use
        let mut citizens_per_raw_school = students.into_par_iter().enumerate().map(|(age, student)|
            {
                let age_grouped_students_per_school = student.into_iter().filter_map(|student| {
                    match finding_closest_school(student, false) {
                        Ok(schools) => {
                            if let Some(school) = schools.first() {
                                Some((*school, student))
                            } else {
                                warn!("No schools available");
                                None
                            }
                        }
                        Err(e) => {
                            warn!("Failed to assign school to student: {}", e);
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
        ).fold(|| HashMap::new(), |mut acc: HashMap<Point<i32>, (Vec<Vec<&mut Citizen>>, Vec<&mut Citizen>, &RawBuilding)>, (age, schools_to_flatten): (usize, HashMap<Point<i32>, (Vec<&mut Citizen>, &RawBuilding)>)| {
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
        }).reduce(|| HashMap::new(), |mut a, b| {
            for (point, (students, _teachers, building)) in b {
                let (entry, _, _) = a.entry(point).or_insert_with(|| (Vec::new(), Vec::new(), building));
                for (index, age_group) in students.into_iter().enumerate() {
                    if entry.len() < index + 1 {
                        entry.push(age_group);
                    } else {
                        let entry_group = entry.get_mut(index).unwrap();
                        entry_group.extend(age_group);
                    }
                }
            }
            a
        });
        info!("Assigned Students to {} raw schools",citizens_per_raw_school.len());


// The amount of teachers that fail to be assigned
        let mut failed_teacher_count = 0;
// Take a two pronged approach to assigning teachers
        for teacher in teachers.into_iter() {
            let mut placed = false;
            match finding_closest_school(teacher, true) {
                Ok(schools) => {
// Attempt to assign Teacher as a Teacher to the closest Available School
// Available, being that it exists, and does not have enough teachers for the number of students

// The Option is a hacky thing to get around the borrow checker, moving teacher into `citizens_per_raw_school` twice
                    let mut teacher = Some(teacher);
                    for school in &schools {
                        let school_entry = citizens_per_raw_school.get_mut(&school.center());
                        if let Some((students, teachers, _)) = school_entry {
                            let total_students = students.iter().map(|age_group| ((age_group.len() as f64 / AVERAGE_CLASS_SIZE).ceil() as usize).max(1)).sum::<usize>();
                            if teachers.len() < total_students {
                                placed = true;
                                if let Some(teacher) = teacher {
                                    teachers.push(teacher);
                                }
                                teacher = None;
                                break;
                            }
                        }
                    }
// If all Schools are full, fallback to adding to the closest school as Secondary Staff
                    if !placed {
                        for school in &schools {
                            let school_entry = citizens_per_raw_school.get_mut(&school.center());
                            if let Some((_students, teachers, _)) = school_entry {
                                if let Some(teacher) = teacher {
                                    teachers.push(teacher);
                                    placed = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to assign school to teacher: {}", e);
                }
            }
            if !placed {
                failed_teacher_count += 1;
            }
        }
        if crate::config::CREATE_DEBUG_DUMPS {
            let debug_directory = crate::config::DEBUG_DUMP_DIRECTORY.to_owned() + "schools/";
            fs::create_dir_all(debug_directory.clone()).context("Failed to create debug dump directory")?;

            let school_boundaries: Vec<(Point<i32>, Vec<Coordinate<i32>>)> = citizens_per_raw_school.iter().filter_map(|(_, (_, _, building))| {
                let path = all_boundaries.get(&building.boundary_id())?;
                Some((building.center(), path.exterior().clone().0))
            }).collect();


            let filename = debug_directory.clone() + "parsed_schools_locations.json";
            let file = File::create(filename.clone()).context(format!("Failed to create file: '{}'", filename.clone()))?;
            serde_json::to_writer(file, &school_boundaries).unwrap();


            let debug_school_data: HashMap<String, (Vec<CitizenID>, Vec<CitizenID>)> = citizens_per_raw_school.iter().map(|(school, (students, teachers, _building))| {
                return (format!("{:?}", school.x_y()), (students.iter().map(|age_group| age_group.iter().map(|student| student.id())).flatten().collect(), teachers.iter().map(|teacher| teacher.id()).collect()));
            }).collect();
            let filename = debug_directory.clone() + "citizen_schools.json";
            let file = File::create(filename.clone()).context(format!("Failed to create file: '{}'", filename.clone()))?;
            serde_json::to_writer(file, &debug_school_data).unwrap();
        }
        info!("Assigned teachers to schools");
        warn!("Failed to assign schools to {} teachers",failed_teacher_count);


        let mut class_total = 0;
        let mut teachers_total = 0;
        let mut misc_staff_total = 0;
        let mut offices_total = 0;
        let mut students_total = 0;
        let mut schools_total = 0;

        let mut schools_missing_teachers = 0;

        let mut debug_stats = HashMap::with_capacity(citizens_per_raw_school.len());


        citizens_per_raw_school.into_iter().for_each(|(_school_position, (students, teachers, building))|
            {
                students_total += students.iter().flatten().collect::<Vec<&&mut Citizen>>().len();
                teachers_total += teachers.len();
                if teachers_total == 0 {
                    schools_missing_teachers += 1;
                    return;
                }
                // Retrieve the Output Area, and build the School building
                // TODO Change to Let Else when `https://github.com/rust-lang/rust/issues/87335` is stabilised
                let possible_output_area_ids = if let Some(area) = get_area_code_for_raw_building(building, output_areas_polygons, building_boundaries) { area } else { return; };
                let output_area_code = if let Some(area) = possible_output_area_ids.keys().next() { area } else { return; };
                let index = if let Some(index) = output_area_lookup.get(output_area_code) { index } else {
                    return;
                };

                let mut buildings = if let Some(output_area) = output_area_buildings.get_mut(*index as usize) { output_area } else { return; };
                let output_area_id = (*output_area_ids.get(*index as usize).expect("No buildings exist in area")).clone();
                let building_id = BuildingID::new(output_area_id, BuildingType::School, buildings.len() as u32);

                // Pull the Citizen ID's out of the Citizens
                let student_ids = students.iter().map(|age_group|
                    age_group.iter().map(|student|
                        student.id()).collect()).collect();

                let teacher_ids = teachers.iter().map(|citizen| citizen.id()).collect();

                let (school, stats) = School::with_students_and_teachers(building_id.clone(), *building, student_ids, teacher_ids);
                debug_stats.insert(format!("({},{}", school.get_location().x(), school.get_location().y()), stats);
                class_total += school.classes().len();
                offices_total += school.offices().len();
                misc_staff_total += school.offices().iter().map(|office| office.len()).sum::<usize>();
                schools_total += 1;

                buildings.push(Box::new(school));

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
        if crate::config::CREATE_DEBUG_DUMPS {
            let debug_directory = crate::config::DEBUG_DUMP_DIRECTORY.to_owned() + "schools/";
            fs::create_dir_all(debug_directory.clone()).context("Failed to create debug dump directory")?;

            let filename = debug_directory.clone() + "school_statistics.json";
            let file = File::create(filename.clone()).context(format!("Failed to create file: '{}'", filename.clone()))?;
            serde_json::to_writer(file, &debug_stats).unwrap();
        }
        warn!("{} schools are missing teachers",schools_missing_teachers);
        info!("Generated {} schools, with {} teachers, {} students across {} classes, with avg class size {} and avg classes per school {}. With {} offices and {} misc staff",schools_total,teachers_total,students_total,class_total,(students_total/class_total),(class_total/schools_total),offices_total,misc_staff_total);
        Ok(())
    }

    /// Iterates through all Output Areas, and All Citizens in that Output Area
    ///
    /// Picks a Workplace Output Area, determined from Census Data Distribution
    ///
    /// Allocates that Citizen to the Workplace Building in that chosen Output Area
    pub fn build_workplaces(
        &mut self,
        mut possible_buildings_per_area: HashMap<String, Vec<RawBuilding>>,
    ) -> anyhow::Result<()> {
        debug!(
            "Assigning workplaces to {} output areas ",
            self.output_areas.len()
        );
        // Shuffle the buildings
        possible_buildings_per_area.par_iter_mut().for_each(|(_, buildings)| buildings.shuffle(&mut thread_rng()));

        // Group Citizens by their workplace output area
        // NOTE This is achieved by removing citizens from self.citizens, because we cannot pass references through
        // The index corresponds to the Output Area
        let mut citizens_to_allocate: Vec<Vec<CitizenID>> = vec![Vec::new(); self.output_areas.len()];
        let mut citizens_allocated_count = 0;
        // Assign workplace areas to each Citizen, per Output area
        for household_output_area in &self.output_areas {
            // Retrieve the census data for the household output area
            let household_census_data = self
                .census_data
                .for_output_area_code(household_output_area.id().code().to_string())
                .ok_or_else(|| DataLoadingError::ValueParsingError {
                    source: ParseErrorType::MissingKey {
                        context: "Cannot retrieve Census Data for output area ".to_string(),
                        key: household_output_area.id().to_string(),
                    },
                })?;

            // For each Citizen, assign a workplace area
            'citizens: for citizen_id in &household_output_area.get_residents() {
                let citizen = household_output_area.citizens.get(citizen_id.local_index()).expect("Citizen living in area doesn't exist!");
                let mut index = 0;
                if citizen.is_student() || citizen.detailed_occupation() == Some(OccupationType::Teaching) {
                    continue 'citizens;
                }
                // Loop until we find a Valid area, otherwise skip this Citizen
                let workplace_output_area_index: u32 =
                    loop {
                        let code = household_census_data.get_random_workplace_area(&mut thread_rng()).context("Failed to retrieve random workplace area")?;
                        if let Some(id) = self.output_area_lookup.get(&code) {
                            if possible_buildings_per_area.get(&code).is_some() {
                                break *id;
                            }
                        }
                        index += 1;
                        if index > 50 {
                            error!("Failed to generate code for Citizen: {}",citizen.id());
                            continue 'citizens;
                        }
                    };
                let citizens_to_add = citizens_to_allocate.get_mut(workplace_output_area_index as usize).expect(&format!("Output area {} doesn't exist", index));
                citizens_to_add.push(citizen.id());
                citizens_allocated_count += 1;
            }
        }
        debug!("Creating workplace buildings for: {:?} Citizens and {} Output Areas",citizens_allocated_count,citizens_to_allocate.len());
        debug!("{} Citizens have not been assigned a workplace area!", self.citizen_output_area_lookup.len()-citizens_allocated_count);
        // Unroll the vectors from the Struct, for Parallel Access
        let (mut output_area_citizens, mut output_area_buildings, output_area_ids) = self.output_areas.iter_mut().map(|area| (&mut area.citizens, &mut area.buildings, &mut area.output_area_id)).fold((Vec::new(), Vec::new(), Vec::new()), |(mut accum_citizens, mut accum_buildings, mut accum_ids), (citizens, buildings, id)| {
            accum_citizens.push(citizens);
            accum_buildings.push(buildings);
            accum_ids.push(id);
            (accum_citizens, accum_buildings, accum_ids)
        });
        // Create buildings for each Workplace output area
        'citizen_allocation_loop: for ((workplace_area_index, mut _citizen_ids), citizens) in citizens_to_allocate.into_iter().enumerate().zip(output_area_citizens) {
            // Retrieve the buildings or skip this area
            let workplace_output_area_buildings = match output_area_buildings.get_mut(workplace_area_index) {
                Some(workplace) => workplace,
                None => {
                    error!("Workplace output area {} doesn't exist!",workplace_area_index);
                    continue 'citizen_allocation_loop;
                }
            };
            let workplace_output_id = (*(output_area_ids.get(workplace_area_index).expect("Cannot retrieve workplace output area id"))).clone();
            let possible_buildings = match possible_buildings_per_area.get_mut(workplace_output_id.code()) {
                Some(buildings) => buildings,
                None => {
                    error!("No Workplace buildings exist for Output Area: {}",workplace_output_id);
                    continue 'citizen_allocation_loop;
                }
            };
            match SimulatorBuilder::assign_buildings_per_output_area(workplace_output_id.clone().clone(), citizens, possible_buildings, workplace_output_area_buildings.len() as u32)
            {
                Ok(buildings) => workplace_output_area_buildings.extend(buildings),
                Err(e) => {
                    error!("Failed to assign buildings for Output Area: {}, Error: {}",workplace_output_id,e);
                }
            }
        }
        Ok(())
    }

    /// Calculates which buildings should be assigned to what occupation, and scales the floor space, to ensure every Citizen can have a workplace
    ///
    /// `next_building_index` is the index to start assigning indexes to new buildings
    fn assign_buildings_per_output_area(workplace_area_code: OutputAreaID, mut citizen_ids: &mut Vec<Citizen>, possible_buildings: &mut Vec<RawBuilding>, mut next_building_index: u32) -> anyhow::Result<Vec<Box<dyn Building + Sync + Send>>> {
        if citizen_ids.len() == 0 {
            warn!("No buildings can be assigned to area {} as no workers exist: {:?}",workplace_area_code,citizen_ids);
            return Ok(Vec::new());
        }
        if possible_buildings.len() == 0 {
            warn!("No buildings can be assigned to area {} as no buildings exist: {:?}",workplace_area_code,possible_buildings);
            return Ok(Vec::new());
        }
        // TODO Need to fix what happens if not enough buildings
        if possible_buildings.len() < OccupationType::iter().len() * 2 {
            //warn!("Not enough buildings {} for {} occupations for area: {}",possible_buildings.len(), OccupationType::iter().len(),workplace_area_code);
        }


        let mut rng = thread_rng();
        // This is the amount to increase bin capacity to ensure it meets the minimum required size
        const BUILDING_PER_OCCUPATION_OVERCAPACITY: f64 = 1.1;

        // Randomise the order of the citizens, to reduce the number of Citizens sharing household and Workplace output areas
        citizen_ids.shuffle(&mut rng);

        // Group by occupation
        let mut citizen_ids_per_occupation: EnumMap<OccupationType, Vec<&mut Citizen>> = citizen_ids.into_iter().filter_map(|citizen| {
            Some((citizen.detailed_occupation()?, citizen))
        }).fold(EnumMap::default(), |mut a, b| {
            a[b.0].push(b.1);
            a
        });

        // Calculate how much space we have
        let available_space: usize = possible_buildings.iter().map(|building| building.size()).sum::<i32>() as usize;

        // Calculate how much space we need
        let mut required_space_per_occupation: EnumMap<OccupationType, usize> = citizen_ids_per_occupation.iter().map(|(occupation, citizens)| {
            let size = get_density_for_occupation(occupation);
            (occupation, size * citizens.len() as u32)
        }).fold(EnumMap::default(), |mut a, b| {
            a[b.0] += b.1 as usize;
            a
        });

        /*        // Add any missing Occupations
                OccupationType::iter().for_each(|occupation| if !required_space_per_occupation.contains_key(&occupation) {
                    required_space_per_occupation.insert(occupation, 0);
                });*/
        let required_space: usize = required_space_per_occupation.values().sum();


        // Calculate how much we need to scale buildings to meet the targets
        let scale = (((required_space as f64) / (available_space as f64)) * BUILDING_PER_OCCUPATION_OVERCAPACITY).ceil() as usize;
        //trace!("Scale for Output Area: {} is {} with {} buildings and {} Workers",workplace_area_code,scale,possible_buildings.len(),total_workers);
        // Allocate buildings using first fit

        // Occupation Type, (Current Total Floor Space, The list of buildings to be generated)
        let mut building_per_occupation: EnumMap<OccupationType, (usize, Vec<RawBuilding>)> = EnumMap::default();//from_array(*(vec![(0, Vec::with_capacity(1000));OccupationType::iter().len()].as_slice()));

        // The amount of floor space required to be added per each occupation
        let mut differences: EnumMap<OccupationType, isize> = EnumMap::default();
        required_space_per_occupation.iter().for_each(|(occupation, size)| { differences[occupation] = *size as isize; });

        // Shuffle to ensure buildings are distributed across the area
        possible_buildings.shuffle(&mut rng);
        for building in possible_buildings.into_iter() {
            let mut added = false;
            let building_size = building.size() as usize * scale;
            if building_size == 0 {
                continue;
            }

            // Add the building to the first Occupation group, that won't exceed the required size
            for (occupation, (current_size, buildings)) in &mut building_per_occupation {
                // If adding the building doesn't exceed the bin size, do it!
                if *current_size + building_size < required_space_per_occupation[occupation] {
                    *current_size += building_size;
                    buildings.push(*building);
                    differences[occupation] -= building_size as isize;
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
                        min_occupation = Some(occupation);
                    }
                }
                // Add to that building
                match min_occupation {
                    Some(occupation) => {
                        let (size, buildings) = &mut building_per_occupation[occupation];
                        *size += building_size;
                        buildings.push(*building);
                        differences[occupation] -= building_size as isize;
                    }
                    None => {
                        //error!("Failed to add building of size: {}, \nCurrent Capacities: {:?}\nRequired Sizes: {:?}",building.size()*(scale as i32),building_per_occupation.iter().map(| (occupation_type,(size,_))|(*occupation_type,*size)).collect::<HashMap<OccupationType,usize>>(),required_space_per_occupation);
                    }
                }
            }
        }
        // Ensure we have meant the minimum requirements
        // TODO This is broken
        /*        for (occupation_type, (size, _buildings)) in &building_per_occupation {
                    let required = required_space_per_occupation.get(occupation_type).expect("Occupation type is missing!");
                    if size <= required {
                        warn!( "Occupation: {:?}, has a size {} smaller than required {} for area {}",occupation_type, size, required,  workplace_area_code);
                        //\nCurrent Capacities: {:?}\nRequired Sizes: {:?}", , required, building_per_occupation.iter().map(|(occupation_type, (size, _))| (*occupation_type, *size)).collect::<HashMap<OccupationType, usize>>(), required_space_per_occupation);
                        //return Ok(HashMap::new());
                    }
                }*/


        // This is the list of full workplaces that need to be added to the parent Output Area
        let mut workplace_buildings: Vec<Box<dyn Building + Sync + Send>> = Vec::new();

        // Assign workplaces to every Citizen
        // TODO Parallelise
        for occupation in OccupationType::iter() {
            // Teaching is handled in build schools
            if occupation == OccupationType::Teaching {
                continue;
            }
            let selected_citizen_ids = &mut citizen_ids_per_occupation[occupation];
            let buildings = &mut building_per_occupation[occupation];
            match SimulatorBuilder::assign_workplaces_to_citizens_per_occupation(workplace_area_code.clone(), occupation, selected_citizen_ids, &buildings.1, next_building_index) {
                Ok(workplaces) => {
                    next_building_index += workplaces.len() as u32;
                    workplace_buildings.extend(workplaces);
                }
                Err(_e) => {
                    //error!("Failed to assign workplaces to Citizens for Occupation: {:?},\n\tError: {:?}",occupation,e);
                    continue;
                }
            }
        }
        Ok(workplace_buildings)
    }

    /// Assigns Each Citizen to one of the Given RawBuildings and transforms the RawBuildings into Workplaces
    ///
    /// Note that each Citizen should have the same Occupation
    /// `next_building_index` is the index to start assigning indexes to new buildings
    fn assign_workplaces_to_citizens_per_occupation(workplace_area_code: OutputAreaID, occupation: OccupationType, citizens: &mut Vec<&mut Citizen>, buildings: &Vec<RawBuilding>, mut next_building_index: u32) -> anyhow::Result<Vec<Box<dyn Building + Sync + Send>>> {
        let total_building_count = buildings.len();
        let total_workers = citizens.len();
        let mut workplace_buildings: Vec<Box<dyn Building + Sync + Send>> = Vec::new();
        let mut buildings = buildings.iter();

        let mut current_workplace: Workplace = Workplace::new(
            BuildingID::new(
                workplace_area_code.clone(),
                BuildingType::Workplace,
                next_building_index,
            ),
            *buildings.next().ok_or_else(|| SimError::InitializationError { message: format!("Ran out of Workplaces ({}) to assign workers ({}/{}) to in Output Area: {}", total_building_count, 0, total_workers, workplace_area_code) })?,
            occupation);
        next_building_index += 1;
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
                            .push(Box::new(current_workplace));

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
                                next_building_index,
                            ),
                            new_raw_building,
                            occupation);
                        next_building_index += 1;
                        new_workplace.add_citizen(citizen.id()).context(
                            "Cannot add Citizen to freshly generated Workplace!",
                        )?;
                        new_workplace
                    }
                };
            citizen.set_workplace_code(current_workplace.id().clone());
        }
        workplace_buildings.push(Box::new(current_workplace) as Box<dyn Building + Sync + Send>);
        Ok(workplace_buildings)
    }


    pub fn apply_initial_infections(&mut self, rng: &mut dyn RngCore) -> anyhow::Result<()> {
        for _ in 0..STARTING_INFECTED_COUNT {
            let output_area: &mut OutputArea = match self.output_areas.iter_mut().choose(rng) {
                Some(area) => area,
                None => {
                    let error = DataLoadingError::ValueParsingError {
                        source: ParseErrorType::IsEmpty {
                            message: "No Output Areas exist infor seeding the disease"
                                .to_string(),
                        },
                    };
                    error!("{:?}",error);
                    continue;
                }
            };
            let citizen: &mut Citizen = match output_area.citizens.iter_mut().choose(rng) {
                Some(citizen) => citizen,
                None => {
                    let error = DataLoadingError::ValueParsingError {
                        source: ParseErrorType::IsEmpty {
                            message: "No citizens exist in the output areas for seeding the disease"
                                .to_string(),
                        },
                    };
                    error!("{:?}",error);
                    continue;
                }
            };
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
            output_area_lookup: Default::default(),
            output_areas_polygons,
            disease_model: DiseaseModel::covid(),
            citizen_output_area_lookup: Default::default(),
        })
    }

    pub fn build(&mut self) -> anyhow::Result<()> {
        let mut timer = Timer::default();
        let mut rng = thread_rng();

        self.initialise_output_areas()
            .context("Failed to initialise output areas!")?;
        timer.code_block_finished(&format!("Initialised {} Output Areas", self.output_areas.len()))?;
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

        // Check all Citizens with a workplace are actually meant to be in a school
        self.output_areas.par_iter().for_each(|area| {
            area.citizens.iter().for_each(|citizen| {
                if citizen.household_code != citizen.workplace_code {
                    assert!(citizen.is_student() || citizen.detailed_occupation().eq(&Some(OccupationType::Teaching)), "Citizen {} should not be in a school, with job: {:?}", citizen.id(), citizen.detailed_occupation());
                }
            })
        });

        timer.code_block_finished(&format!(
            "Built schools",
        ))?;


        // TODO Currently any buildings remaining are treated as Workplaces
        let possible_workplaces: HashMap<String, Vec<RawBuilding>> =
            possible_buildings_per_area
                .drain()
                .filter_map(|(area, mut classified_buildings)| {
                    let buildings: Vec<RawBuilding> =
                        classified_buildings.drain().filter(|(class, _)| *class != TagClassifiedBuilding::School && *class != TagClassifiedBuilding::Household).flat_map(|(_, a)| a).collect();
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

        /*        // Remove any areas that do not have any workplaces
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
                });*/
        info!("Starting to build workplaces for {} areas",self.output_areas.len());
        self.build_workplaces(possible_workplaces)
            .context("Failed to build workplaces")?;
        timer.code_block_finished("Generated workplaces for {} Output Areas")?;

        let work_from_home_count: u32 = self.output_areas.par_iter().map(|area|
            area.citizens.par_iter().map(|citizen| if citizen.household_code.eq(&citizen.workplace_code) { 1 } else { 0 }).sum::<u32>()).sum();
        debug!("{} out of {} Citizens {:.1}%, are working from home.",work_from_home_count.to_formatted_string(&NUMBER_FORMATTING),self.citizen_output_area_lookup.len().to_formatted_string(&NUMBER_FORMATTING),(work_from_home_count as f64/self.citizen_output_area_lookup.len() as f64)*100.0);
        // Infect random citizens
        self.apply_initial_infections(&mut rng)
            .context("Failed to create initial infections")?;

        timer.code_block_finished("Applied initial infections")?;
        debug!(
            "Starting Statistics: There are {} total Citizens, {} Output Areas",
            self.citizen_output_area_lookup.len().to_formatted_string(&NUMBER_FORMATTING),
            self.output_areas.len().to_formatted_string(&NUMBER_FORMATTING)
        );
        Ok(())
    }
}

/// Returns a list of Output Areas that the given building is inside
///
/// If the building is in multiple Areas, it is duplicated
fn get_area_code_for_raw_building(building: &RawBuilding, output_area_lookup: &PolygonContainer<String>, building_boundaries: &HashMap<BuildingBoundaryID, geo_types::Polygon<i32>>) -> Option<HashMap<String, Vec<RawBuilding>>> {
    let boundary = building_boundaries.get(&building.boundary_id());
    if let Some(boundary) = boundary {
        if let Ok(areas) = output_area_lookup.find_polygons_containing_polygon(boundary) {
            let area_locations = areas.map(|area| area.to_string())
                .zip(std::iter::repeat(vec![*building]))
                .collect::<HashMap<String, Vec<RawBuilding>>>();
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
) -> HashMap<String, HashMap<TagClassifiedBuilding, Vec<RawBuilding>>> {
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
        String,
        HashMap<TagClassifiedBuilding, Vec<RawBuilding>>>, b: (TagClassifiedBuilding, HashMap<String, Vec<RawBuilding>>)| {
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
