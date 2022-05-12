use clap::{App, Arg};
use log::warn;

extern crate serde;

use serde::{Serialize, Deserialize};

const VERSION_NUMBER: &str = "V2.1";

fn get_cmd_arguments() -> clap::ArgMatches<'static> {
    App::new("Epidemic Simulation Using Census Data and Open Street Maps")
        .version("2.0")
        .author("Sam Ralph <sr1474@york.ac.uk")
        .about("Simulates an Epidemic Using UK Census Data")
        .usage("run \"area_code\" --mode=<mode>
            \n    The area code which to use must be specified (area)\
            \n    The directory specifying where to store data must be specified (directory)\
            \n    There are several modes available to choose from:\
            \n        download                  ->      Downloads and Verifies the data files for a simulation\
            \n        resume                    ->      Used to resume a table download, (requires the '--table' argument)\
            \n        simulate                  ->      Starts a text only logging simulation for the given area\
            \n        render                    ->      Starts a simulation with a live view of what is happening via a rendering engine\
            \n        visualise_map             ->      Creates a png of Buildings overlayed with Output Area polygons\
            \n        visualise_output_areas    ->      Shows Output Areas coloured against several measures\
            \n        visualise_buildings       ->      Shows the voronoi diagrams of buildings")
        .arg(
            Arg::with_name("area_code")
                .help("Specifies the area code to simulate")
                .takes_value(true)
                .required(true))
        .arg(
            Arg::with_name("mode")
                .help("Specifies the mode of the simulator")
                .takes_value(true)
                .required(true))
        .arg(
            Arg::with_name("data_directory")
                .short("d")
                .long("directory")
                .help("The directory that the data files are located (osm and census tables)")
                .require_equals(true)
                .takes_value(true))
        .arg(
            Arg::with_name("output_directory")
                .long("output_name")
                .help("Specifies the name of the output directory to store statistics")
                .takes_value(true)
                .require_equals(true))
        .arg(
            Arg::with_name("allow-downloads")
                .long("allow-downloads")
                .help("If enabled, census tables will be automatically downloaded."))
        .arg(
            Arg::with_name("use-cache")
                .long("use-cache")
                .help("Will attempt to use cached pre loaded data, instead of parsing tables/maps from scratch"))
        .arg(Arg::with_name("grid-size")
            .require_equals(true)
            .long("grid-size")
            .takes_value(true)
            .help("Specifies the size of the Voronoi Lookup Grids"))

        .arg(Arg::with_name("number-of-threads")
            .require_equals(true)
            .long("number-of-threads")
            .takes_value(true)
            .help("Specifiers the number of separate threads to use for processing"))
        .arg(
            Arg::with_name("table")
                .long("table")
                .help("Specifies the table name to download")
                .takes_value(true)
                .requires("area"),
        )
        .get_matches()
}

pub struct Arguments {
    pub mode: SimMode,
    pub data_directory: String,
    pub output_directory: String,
    pub area_code: String,
    pub use_cache: bool,
    pub allow_downloads: bool,
    pub grid_size: i32,
    pub number_of_threads: Option<usize>,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SimMode {
    Simulate,
    Render,
    Download,
    Resume,
    #[serde(alias = "visualise_map")]
    VisualiseMap,
    #[serde(alias = "visualise_output_areas")]
    VisualiseOutputAreas,
    #[serde(alias = "visualise_buildings")]
    VisualiseBuildings,
}

impl Arguments {
    pub fn load_from_arguments() -> Arguments {
        let mut arguments = Arguments::default();
        let matches = get_cmd_arguments();
        arguments.mode = serde_plain::from_str(matches.value_of("mode").expect("Mode for the simulator must be provided!")).expect("Unknown mode received! Use --help for a list of valid modes");

        if let Some(directory) = matches
            .value_of("data_directory") {
            //let census_directory = directory.to_owned() + "/";
            arguments.data_directory = directory.to_string();
        }

        if let Some(directory) = matches.value_of("output_directory") {
            // TODO Remove illegal characters to prevent directory traversal
            //output_directory = sanitize(name)+"/";
            arguments.output_directory = directory.to_string();
        }

        if let Some(area_code) = matches.value_of("area_code") {
            arguments.area_code = area_code.to_string()
        }
        if matches.is_present("use-cache") {
            arguments.use_cache = true;
        }
        if matches.is_present("allow-downloads") {
            arguments.allow_downloads = true;
        }
        if let Some(grid_size) = matches
            .value_of("grid-size") {
            let grid_size_parsed = grid_size.parse();
            match grid_size_parsed {
                Ok(grid_size) =>
                    arguments.grid_size = grid_size,
                Err(e) => {
                    warn!("Failed to parse grid size with value: '{}' and error {}. Using default value of: {}",grid_size,e,arguments.grid_size)
                }
            }
        }

        if let Some(number_of_threads) = matches
            .value_of("number_of_threads") {
            let number_of_threads_parsed = number_of_threads.parse();
            match number_of_threads_parsed {
                Ok(number_of_threads) =>
                    arguments.number_of_threads = Some(number_of_threads),
                Err(e) => {
                    warn!("Failed to parse number of threads with value: '{}' and error {}. Using default value.",number_of_threads,e)
                }
            }
        }
        arguments
    }
}

impl Default for Arguments {
    fn default() -> Self {
        Arguments {
            mode: SimMode::Simulate,
            data_directory: "data/".to_string(),
            output_directory: "simulator_output/".to_string() + VERSION_NUMBER,
            area_code: "1946157112TYPE299".to_string(),
            use_cache: false,
            allow_downloads: false,
            grid_size: 0,
            number_of_threads: None,
        }
    }
}