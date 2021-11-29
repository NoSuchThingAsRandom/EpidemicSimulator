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
