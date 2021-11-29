use std::collections::HashMap;
use std::fmt::Debug;

use serde::de::DeserializeOwned;

use crate::parsing_error::CensusError;

pub mod population_and_density_per_output_area;

/// This is used to load in a CSV file, and each row corresponds to one struct
pub trait PreProcessingTable: Debug + DeserializeOwned + Sized {}

/// This represents a transformed `PreProcessingTable` struct per output area
/// This is a container for the entire processed CSV
///
/// Should contain a hashmap of OutputArea Codes to TableEntries
pub trait TableEntry: Debug + Sized {
    /// Returns the entire processed CSV per output area
    fn generate(data: Vec<impl PreProcessingTable + 'static>) -> Result<HashMap<String, Self>, CensusError>;
}
