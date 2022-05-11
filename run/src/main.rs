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


use anyhow::Context;
use log::{info};
use rayon;

use crate::arguments::Arguments;
use crate::execute_modes::execute_arguments;

use crate::load_data::load_data;
use crate::load_data::load_data_and_init_sim;

mod load_data;
mod visualise;
mod arguments;
mod execute_modes;

//use visualisation::citizen_connections::{connected_groups, draw_graph};
//use visualisation::image_export::DrawingRecord;
#[allow(dead_code)]
fn get_bool_env(env_name: &str) -> anyhow::Result<bool> {
    std::env::var(env_name)
        .context(format!("Missing env variable '{}'", env_name))?
        .parse()
        .context(format!("'{}' is not a bool!", env_name))
}

#[allow(dead_code)]
fn get_string_env(env_name: &str) -> anyhow::Result<String> {
    std::env::var(env_name).context(format!("Missing env variable '{}'", env_name))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().expect("Failed to load dot env");
    pretty_env_logger::init_timed();


    let arguments = Arguments::load_from_arguments();
    if let Some(threads) = arguments.number_of_threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("Failed to build Rayon thread pool!");
    } else {
        rayon::ThreadPoolBuilder::new()
            .build_global()
            .expect("Failed to build Rayon thread pool!");
    }


    info!(
        "Using area: {}, Utilizing Cache: {}, Allowing downloads: {}",
        arguments.area_code, arguments.use_cache, arguments.allow_downloads
    );
    execute_arguments(arguments).await?;
    Ok(())
}