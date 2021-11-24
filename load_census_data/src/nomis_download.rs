//! Module used for download Census Tables from the NOMIS API
use std::time::Instant;

use log::{debug, info};
use serde_json::Value;

use crate::parsing_error::CensusError;
use crate::population_and_density_per_output_area::SELECTED_COLUMNS;

const ENGLAND_OUTPUT_AREAS_CODE: &str = "2092957699TYPE299";
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
        path.push_str(ENGLAND_OUTPUT_AREAS_CODE);
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
}
