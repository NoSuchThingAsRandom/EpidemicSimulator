/*
 * Epidemic Simulation Using Census Data (ESUCD)
 * Copyright (c)  2021. Sam Ralph
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

//! Module used for download Census Tables from the NOMIS API
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Instant;

use async_recursion::async_recursion;
use lazy_static::lazy_static;
use log::{debug, error, info, trace};
use serde_json::Value;

use crate::CensusTableNames;
use crate::CensusTableNames::ResidentialAreaVsWorkplaceArea;
use crate::parsing_error::CensusError;

lazy_static! {
    pub static ref NOMIS_API_KEY: String =
        std::env::var("NOMIS_API_KEY").expect("Failed to load NOMIS UUID");
}
pub const YORK_OUTPUT_AREA_CODE: &str = "1254162148...1254162748,1254262205...1254262240";
pub const YORK_AND_HUMBER_OUTPUT_AREA_CODE: &str = "1254132824...1254136983,1254148629...1254155319,1254159242...1254162748,1254233375...1254235353,1254258198...1254258221,1254258325...1254258337,1254260875...1254261010,1254261711...1254261745,1254261853...1254261918,1254262125...1254262240,1254262341...1254262398,1254262498...1254262532,1254262620...1254262658,1254262776...1254262816,1254262922...1254262925,1254263031...1254263052,1254263300...1254263321,1254264241...1254264419,1254264646...1254264670,1254265272...1254265286,1254266348...1254266359,1254266824...1254266863,1254267006...1254267043,1254267588...1254267709";
//"1254132824...1254267709";
//https://www.nomisweb.co.uk/api/v01/NM_144_1/summary?geography=2092957699TYPE299&recordlimit=20000&uid=
//https://www.nomisweb.co.uk/api/v01/dataset/NM_144_1.data.csv?date=latest&geography=1254162148...1254162748,1254262205...1254262240&rural_urban=0&cell=0&measures=20100";
const ENGLAND_OUTPUT_AREAS_CODE: &str = "2092957699TYPE299";
const YORKSHIRE_AND_HUMBER_OUTPUT_AREA: &str = "2013265923TYPE299";
const MAX_RETRY_COUNT: u8 = 3;
const POPULATION_TABLE_CODE: &str = "NM_144_1";
pub const NOMIS_API: &str = "https://www.nomisweb.co.uk/api/v01/";
pub const PAGE_SIZE: usize = 1000000;

/// This is a struct to download census tables from the NOMIS api
#[derive(Default)]
pub struct DataFetcher {
    client: reqwest::Client,
}

impl DataFetcher {
    /// Retrieves a list of all the census 20211 tables
    pub async fn get_list_of_census_2011_dataset_names(&self) -> Result<Value, CensusError> {
        let api: String = format!(
            "{}dataset/def.sdmx.json?search=c2011*&uid=0xca845fec90a78b8554b075b32294605f543d9c48",
            NOMIS_API
        );
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
    pub async fn get_table(
        &self,
        id: String,
        number_of_records: usize,
        page_size: usize,
    ) -> Result<String, CensusError> {
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
            /*            to_send.push_str("&select=");
            to_send.push_str(SELECTED_COLUMNS);*/
            info!("Making request to: {}", to_send);
            let request = self.client.get(to_send).send().await?;
            debug!("Got response: {:?}", request);
            let new_data = request.text().await?;
            data.push_str(new_data.as_str());
            info!("Completed request {} in {:?}", index, start_time.elapsed());
        }
        Ok(data)
    }
    pub async fn download_and_save_table(
        &self,
        filename: &str,
        request: String,
        number_of_records: Option<usize>,
        resume_from_record: Option<usize>,
    ) -> Result<(), CensusError> {
        info!("Using base request: {}", request);
        let dir_path = filename.split('/').last().unwrap();
        let dir_path = filename.to_string().replace(dir_path, "");
        std::fs::create_dir_all(dir_path)?;

        let total_time = Instant::now();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(resume_from_record.is_some())
            .open(filename)?;
        let mut processed_row_count = 0;
        if let Some(number_of_records) = number_of_records {
            for index in resume_from_record
                .map(|value| value / PAGE_SIZE)
                .unwrap_or(0)
                ..(number_of_records as f64 / PAGE_SIZE as f64).ceil() as usize
            {
                let current_time = Instant::now();
                let data = self.execute_request(index, request.clone(), 0).await?;
                if let Some(data) = data {
                    file.write_all(data.as_bytes())?;
                } else {
                    break;
                }
                debug!(
                    "Completed request {} in {:?}",
                    index,
                    current_time.elapsed()
                );
            }
        } else {
            let mut index = resume_from_record
                .map(|value| value / PAGE_SIZE)
                .unwrap_or(0);
            let mut data = Some(String::new());
            let mut row_count: Option<usize> = None;
            while data.is_some() {
                let current_time = Instant::now();
                if let Some(data) = data {
                    // Get number of rows
                    if row_count.is_none() {
                        let mut rows = data.split('\n');
                        rows.next();
                        if let Some(row) = rows.next() {
                            let split = row.split(',');
                            if let Some(count) = split.last() {
                                if let Ok(count) = count.parse() {
                                    row_count = Some(count);
                                }
                            }
                        }
                    }
                    file.write_all(data.as_bytes())?;
                }
                data = self.execute_request(index, request.clone(), 0).await?;
                processed_row_count += PAGE_SIZE;
                index += 1;
                let est_time = row_count.map(|row_count| {
                    ((row_count - processed_row_count) / PAGE_SIZE as usize) as u64
                        * current_time.elapsed().as_secs()
                });
                let percentage =
                    row_count.map(|total_row_count| processed_row_count * 100 / total_row_count);
                info!("Completed request {} in {:?}, current row count {}/{:?}={:?}% Estimated Time: {:?} seconds", index, current_time.elapsed(),processed_row_count,row_count,percentage,est_time);
            }
        }
        file.flush()?;
        info!(
            "Finished downloading table with {} rows in {:?}",
            processed_row_count,
            total_time.elapsed()
        );
        Ok(())
    }
    /// Execute the request, returning Data if it exists
    #[async_recursion]
    async fn execute_request(
        &self,
        index: usize,
        base_request: String,
        retry_count: u8,
    ) -> Result<Option<String>, CensusError> {
        let mut current_request = base_request.clone();
        current_request.push_str("&RecordOffset=");
        current_request.push_str((index * PAGE_SIZE).to_string().as_str());
        if index != 0 {
            current_request.push_str("&ExcludeColumnHeadings=true");
        }
        trace!("Making request to: {}", current_request);

        match self.client.get(current_request).send().await {
            Err(e) => {
                error!("Request failed: {}", e);
                return if retry_count < MAX_RETRY_COUNT {
                    info!("Retrying web request...");
                    self.execute_request(index, base_request, retry_count + 1)
                        .await
                } else {
                    Err(CensusError::from(e))
                };
            }
            Ok(request) => {
                trace!("Got response: {:?}", request);
                let data = request.text().await?;
                if data.is_empty() {
                    info!("No more records, exiting");
                    return Ok(None);
                }
                Ok(Some(data))
            }
        }
    }
}

pub fn table_144_york_output_areas(page_size: usize) -> String {
    let mut path = String::from(NOMIS_API);
    path.push_str(POPULATION_TABLE_CODE);
    path.push_str(".data.csv");
    path.push_str("?geography=");
    path.push_str(YORK_OUTPUT_AREA_CODE);
    path.push_str("&recordlimit=");
    path.push_str(page_size.to_string().as_str());
    path.push_str("&uid=0xca845fec90a78b8554b075b32294605f543d9c48");
    path
}

pub fn build_table_request_string(table: CensusTableNames, area_code: String) -> String {
    let mut path = String::from(NOMIS_API);
    path.push_str("dataset/");
    path.push_str(table.get_api_code());
    path.push_str(".data.csv");

    if let ResidentialAreaVsWorkplaceArea = table {
        path.push_str("?currently_residing_in=");
        path.push_str(&area_code);
        path.push_str("&place_of_work=");
        path.push_str(YORK_OUTPUT_AREA_CODE);
    } else {
        path.push_str("?geography=");
        path.push_str(&area_code);
    }
    path.push_str("&ExcludeZeroValues=true");
    path.push_str("&recordlimit=");
    path.push_str(PAGE_SIZE.to_string().as_str());
    if let Some(columns) = table.get_required_columns() {
        path.push_str("&select=");
        path.push_str(columns);
    }
    path.push_str("&uid=");
    path += &NOMIS_API_KEY;
    path
}
