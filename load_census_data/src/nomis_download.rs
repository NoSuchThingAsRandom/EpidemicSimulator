//! Module used for download Census Tables from the NOMIS API
use std::fs::File;
use std::io::Write;
use std::time::Instant;

use log::{debug, info};
use serde_json::Value;

use crate::parsing_error::CensusError;
use crate::tables::population_and_density_per_output_area::SELECTED_COLUMNS;

const YORK_OUTPUT_AREA_CODE: &str = "1254162148...1254162748,1254262205...1254262240";
//https://www.nomisweb.co.uk/api/v01/dataset/NM_144_1.data.csv?date=latest&geography=1254162148...1254162748,1254262205...1254262240&rural_urban=0&cell=0&measures=20100";
const ENGLAND_OUTPUT_AREAS_CODE: &str = "2092957699TYPE299";
const POPULATION_TABLE_CODE: &str = "NM_144_1";
const NOMIS_API: &str = "https://www.nomisweb.co.uk/api/v01/";


/// This is a struct to download census tables from the NOMIS api
pub struct DataFetcher {
    client: reqwest::Client,
}

impl Default for DataFetcher {
    fn default() -> Self {
        DataFetcher { client: reqwest::Client::default() }
    }
}


impl DataFetcher {
    /// Retrieves a list of all the census 20211 tables
    pub async fn get_list_of_census_2011_dataset_names(&self) -> Result<Value, CensusError> {
        let api: String = format!("{}dataset/def.sdmx.json?search=c2011*&uid=0xca845fec90a78b8554b075b32294605f543d9c48", NOMIS_API);
        println!("Making request to: {}", api);
        let request = self.client.get(api).send().await?;
        println!("Got response: {:?}", request);
        let data = request.text().await?;
        let json: Value = serde_json::from_str(&data)?;
        Ok(json)
    }

    /// Retrieves all the Output Area geography codes
    pub async fn get_geography_code(&self, id: String) -> Result<(), CensusError> {
        let mut path = String::from("https://www.nomisweb.co.uk/api/v01/dataset/");
        path.push_str(&id);
        path.push_str("/geography.def.sdmx.json");
        println!("Making request to: {}", path);
        let request = self.client.get(path).send().await?;
        println!("Got response: {:?}", request);
        let data = request.text().await?;
        println!("{}", data);
        Ok(())
    }
    /// Downloads a census table from the NOMIS api
    pub async fn get_table(&self, id: String, number_of_records: usize, page_size: usize) -> Result<String, CensusError> {
        let mut path = String::from(NOMIS_API);
        path.push_str(&id);
        path.push_str(".data.csv");
        path.push_str("?geography=");
        path.push_str(YORK_OUTPUT_AREA_CODE);
        path.push_str("&recordlimit=");
        path.push_str(page_size.to_string().as_str());
        path.push_str("&uid=0xca845fec90a78b8554b075b32294605f543d9c48");
        let mut data = String::new();
        let start_time = Instant::now();
        for index in 0..(number_of_records as f64 / page_size as f64).ceil() as usize {
            let mut to_send = path.clone();
            to_send.push_str("&RecordOffset=");
            to_send.push_str((index * page_size).to_string().as_str());
            if index != 0 {
                to_send.push_str("&ExcludeColumnHeadings=true");
            }
            to_send.push_str("&select=");
            to_send.push_str(SELECTED_COLUMNS);
            info!("Making request to: {}", to_send);
            let request = self.client.get(to_send).send().await?;
            debug!("Got response: {:?}", request);
            let new_data = request.text().await?;
            data.push_str(new_data.as_str());
            info!("Completed request {} in {:?}",index,start_time.elapsed());
        }
        Ok(data)
    }
    pub async fn download_and_save_table(&self, filename: String, request: String, number_of_records: usize, page_size: usize) -> Result<(), CensusError> {
        let start_time = Instant::now();
        let mut file = std::fs::File::create(filename)?;
        for index in 0..(number_of_records as f64 / page_size as f64).ceil() as usize {
            let mut to_send = request.clone();
            to_send.push_str("&RecordOffset=");
            to_send.push_str((index * page_size).to_string().as_str());
            if index != 0 {
                to_send.push_str("&ExcludeColumnHeadings=true");
            }
            to_send.push_str("&select=");
            to_send.push_str(SELECTED_COLUMNS);
            info!("Making request to: {}", to_send);
            let request = self.client.get(to_send).send().await?;
            debug!("Got response: {:?}", request);
            let new_data = request.text().await?;
            if new_data.is_empty() {
                info!("No more records, exiting");
                break;
            }
            file.write_all(new_data.as_bytes())?;
            info!("Completed request {} in {:?}",index,start_time.elapsed());
        }
        file.flush()?;
        Ok(())
    }
}

pub fn table_144_york_output_areas(page_size: usize) -> String {
    let mut path = String::from(NOMIS_API);
    path.push_str(&POPULATION_TABLE_CODE);
    path.push_str(".data.csv");
    path.push_str("?geography=");
    path.push_str(YORK_OUTPUT_AREA_CODE);
    path.push_str("&recordlimit=");
    path.push_str(page_size.to_string().as_str());
    path.push_str("&uid=0xca845fec90a78b8554b075b32294605f543d9c48");
    path
}
