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

use crate::tables::occupation_count::OccupationType;

/// This stores the area in square metres per full time employee
///
/// Page 9 of Employment Densities Guide: 2nd Edition: https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/378203/employ-den.pdf
/// https://www.gov.uk/government/publications/employment-densities-guide
pub struct EmploymentDensities {}

impl EmploymentDensities {
    pub const INDUSTRIAL_GENERAL: u32 = 36;
    pub const INDUSTRIAL_LIGHT_INDUSTRY_BUSINESS_PARK: u32 = 47;
    pub const WAREHOUSE_DISTRIBUTION_GENERAL: u32 = 70;
    pub const WAREHOUSE_DISTRIBUTION_WAREHOUSING: u32 = 80;
    pub const OFFICE_GENERAL_OFFICE: u32 = 12;
    pub const OFFICE_CALL_CENTRES: u32 = 8;
    pub const OFFICE_IT_DATA_CENTRES: u32 = 47;
    pub const OFFICE_BUSINESS_PARK: u32 = 10;
    pub const OFFICE_SERVICED_OFFICE: u32 = 10;
    pub const RETAIL_HIGH_STREET: u32 = 19;
    pub const RETAIL_FOOD_SUPERSTORES: u32 = 17;
    pub const RETAIL_OTHER_SUPERSTORES_RETAIL_WAREHOUSES: u32 = 90;
    pub const RETAIL_FINANCIAL_PROFESSIONAL_SERVICES: u32 = 16;
    pub const RETAIL_RESTAURANTS_CAFES: u32 = 18;
    pub fn get_size_for_occupation(occupation: OccupationType) -> u32 {
        match occupation {
            OccupationType::Managers => { Self::OFFICE_GENERAL_OFFICE }
            OccupationType::Professional => { Self::OFFICE_GENERAL_OFFICE }
            OccupationType::Technical => { Self::OFFICE_SERVICED_OFFICE }
            OccupationType::Administrative => { Self::OFFICE_GENERAL_OFFICE }
            OccupationType::SkilledTrades => { Self::INDUSTRIAL_GENERAL }
            OccupationType::Caring => { Self::INDUSTRIAL_LIGHT_INDUSTRY_BUSINESS_PARK }
            OccupationType::Sales => { Self::RETAIL_HIGH_STREET }
            OccupationType::MachineOperatives => { Self::INDUSTRIAL_GENERAL }
            OccupationType::Teaching => { Self::RETAIL_HIGH_STREET }
        }
    }
}

