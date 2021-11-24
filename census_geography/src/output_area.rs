use enum_map::EnumMap;

use load_census_data::population_and_density_per_output_area::PopulationRecord as PopRecord;
use load_census_data::table_144_enum_values::{AreaClassification, PersonType};

use crate::BuildingCode;
use crate::citizen::{Citizen, WorkType};
use crate::household::Household;

/// This is a container for a Census Output Area
///
/// Has a given code corresponding to an area of the country, and a list of households and citizens
///
/// The polygon and `draw()` function can be used for image representation
pub struct OutputArea {
    /// The Census Data Output Area Code
    pub code: String,
    /// The list of citizens who have a "home" in this area
    pub citizens: Vec<Citizen>,
    /// How big the area is in Hectares
    pub area_size: f32,
    /// How many people per hectare? TODO Check this
    pub density: f32,
    /// A map of households, corresponding to what area they are in (Rural, Urban, Etc)
    pub households: EnumMap<AreaClassification, Vec<Household>>,
    /// A polygon for drawing this output area
    pub polygon: geo_types::Polygon<f64>,
}

impl OutputArea {
    /// Builds a new output area, for the given code, polygon for drawing and a census record of the population
    pub fn new(code: String, polygon: geo_types::Polygon<f64>, census_data: &PopRecord) -> OutputArea {
        // TODO Fix this
        let mut household_classification = EnumMap::default();
        let mut citizens = Vec::with_capacity(census_data.population_size as usize);
        for (area, pop_count) in census_data.population_counts.iter() {
            // TODO Currently assigning 4 people per household
            // Should use census data instead
            let household_size = 4;
            let household_number = pop_count[PersonType::All] / household_size;
            let mut generated_population = 0;
            let mut households = Vec::with_capacity(household_number as usize);
            for _ in 0..household_number {
                let area_code = BuildingCode::new(code.clone(), area);
                let mut household = Household::new(area_code.clone());
                for _ in 0..household_size {
                    // TODO Add workplaces to citizens
                    let citizen = Citizen::new(area_code.clone(), area_code.clone(), WorkType::NA);
                    household.add_citizen(citizen.id());
                    citizens.push(citizen);
                    generated_population += 1;
                }
                households.push(household);
                if generated_population >= pop_count[PersonType::All] {
                    break;
                }
            }
            household_classification[area] = households;
        }
        OutputArea {
            code,
            citizens,
            area_size: census_data.area_size,
            density: census_data.density,
            households: household_classification,
            polygon,
        }
    }
}