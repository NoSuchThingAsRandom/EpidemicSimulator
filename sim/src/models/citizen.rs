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

use std::fmt::{Debug, Display, Formatter};

use rand::{Rng, RngCore};
use serde::Serialize;
use uuid::Uuid;

use load_census_data::tables::occupation_count::OccupationType;

use crate::disease::{DiseaseModel, DiseaseStatus};
use crate::interventions::MaskStatus;
use crate::models::building::BuildingCode;

fn factorial(n: u8) -> f64 {
    match n {
        0 => 1.0,
        1 => 1.0,
        2 => 2.0,
        3 => 6.0,
        4 => 24.0,
        5 => 120.0,
        6 => 720.0,
        7 => 5040.0,
        8 => 40320.0,
        9 => 362880.0,
        10 => 3628800.0,
        11 => 3628800.0,
        12 => 39916800.0,
        13 => 479001600.0,
        14 => 87178291200.0,
        15 => 1307674368000.0,
        16 => 20922789888000.0,
        17 => 355687428096000.0,
        18 => 6402373705728000.0,
        19 => 121645100408832000.0,
        20 => 2432902008176640000.0,
        21 => 51090942171709440000.0,
        22 => 1124000727777607680000.0,
        23 => 25852016738884976640000.0,
        24 => 620448401733239439360000.0,
        25 => 15511210043330985984000000.0,
        26 => 403291461126605635584000000.0,
        27 => 10888869450418352160768000000.0,
        28 => 304888344611713860501504000000.0,
        29 => 8841761993739701954543616000000.0,
        30 => 265252859812191058636308480000000.0,
        31 => 8222838654177922817725562880000000.0,
        32 => 263130836933693530167218012160000000.0,
        33 => 8683317618811886495518194401280000000.0,
        34 => 295232799039604140847618609643520000000.0,
        35 => 10333147966386144929666651337523200000000.0,
        36 => 371993326789901217467999448150835200000000.0,
        37 => 13763753091226345046315979581580902400000000.0,
        38 => 523022617466601111760007224100074291200000000.0,
        39 => 20397882081197443358640281739902897356800000000.0,
        40 => 815915283247897734345611269596115894272000000000.0,
        41 => 33452526613163807108170062053440751665152000000000.0,
        42 => 1405006117752879898543142606244511569936384000000000.0,
        43 => 60415263063373835637355132068513997507264512000000000.0,
        44 => 2658271574788448768043625811014615890319638528000000000.0,
        45 => 119622220865480194561963161495657715064383733760000000000.0,
        46 => 5502622159812088949850305428800254892961651752960000000000.0,
        47 => 258623241511168180642964355153611979969197632389120000000000.0,
        48 => 12413915592536072670862289047373375038521486354677760000000000.0,
        49 => 608281864034267560872252163321295376887552831379210240000000000.0,
        50 => 30414093201713378043612608166064768844377641568960512000000000000.0,
        51 => 1551118753287382280224243016469303211063259720016986112000000000000.0,
        52 => 80658175170943878571660636856403766975289505440883277824000000000000.0,
        53 => 4274883284060025564298013753389399649690343788366813724672000000000000.0,
        54 => 230843697339241380472092742683027581083278564571807941132288000000000000.0,
        55 => 12696403353658275925965100847566516959580321051449436762275840000000000000.0,
        56 => 710998587804863451854045647463724949736497978881168458687447040000000000000.0,
        57 => 40526919504877216755680601905432322134980384796226602145184481280000000000000.0,
        58 => 2350561331282878571829474910515074683828862318181142924420699914240000000000000.0,
        59 => 138683118545689835737939019720389406345902876772687432540821294940160000000000000.0,
        60 => 8320987112741390144276341183223364380754172606361245952449277696409600000000000000.0,
        61 => 507580213877224798800856812176625227226004528988036003099405939480985600000000000000.0,
        62 => 31469973260387937525653122354950764088012280797258232192163168247821107200000000000000.0,
        63 => 1982608315404440064116146708361898137544773690227268628106279599612729753600000000000000.0,
        64 => 126886932185884164103433389335161480802865516174545192198801894375214704230400000000000000.0,
        _ => panic!("Increase the factorial count to {}", n)
    }
}

/// Calculates the binomial distribution, with one success
fn binomial(probability: f64, chances: u8) -> f64 {
    factorial(chances) as f64 / factorial(chances - 1) as f64
        * probability
        * (probability.powf(chances as f64 - 1.0))
}

/// This is used to represent a single Citizen in the simulation
#[derive(Debug, Serialize, Clone)]
pub struct Citizen {
    /// A unique identifier for this Citizen
    id: Uuid,
    /// The building they reside at (home)
    pub household_code: BuildingCode,
    /// The place they work at
    pub workplace_code: BuildingCode,
    occupation: OccupationType,
    /// The hour which they go to work
    start_working_hour: u32,
    /// The hour which they leave to work
    end_working_hour: u32,
    /// The building the Citizen is currently at
    pub current_position: BuildingCode,
    /// Disease Status
    pub disease_status: DiseaseStatus,
    /// Whether this Citizen wears a mask
    pub is_mask_compliant: bool,
}

impl Citizen {
    /// Generates a new Citizen with a random ID
    pub fn new(
        household_code: BuildingCode,
        workplace_code: BuildingCode,
        occupation_type: OccupationType,
        is_mask_compliant: bool,
    ) -> Citizen {
        Citizen {
            id: Uuid::new_v4(),
            household_code: household_code.clone(),
            workplace_code,
            occupation: occupation_type,
            start_working_hour: 9,
            end_working_hour: 17,
            current_position: household_code,
            disease_status: DiseaseStatus::Susceptible,
            is_mask_compliant,
        }
    }
    /// Returns the ID of this Citizen
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn execute_time_step(
        &mut self,
        current_hour: u32,
        disease: &DiseaseModel,
        lockdown_enabled: bool,
    ) {
        self.disease_status = DiseaseStatus::execute_time_step(&self.disease_status, disease);
        if !lockdown_enabled {
            match current_hour % 24 {
                hour if hour == self.start_working_hour => {
                    self.current_position = self.workplace_code.clone();
                }
                hour if hour == self.end_working_hour => {
                    self.current_position = self.household_code.clone();
                }
                _ => {}
            }
        }
    }
    /// Registers a new exposure to this citizen
    ///
    /// # Paramaters
    /// exposure_total: The amount of the exposures that occured in this time step
    pub fn expose(
        &mut self,
        exposure_total: usize,
        disease_model: &DiseaseModel,
        mask_status: &MaskStatus,
        rng: &mut dyn RngCore,
    ) -> bool {
        let mask_status = if self.is_mask_compliant {
            &MaskStatus::None(0)
        } else {
            mask_status
        };
        let exposure_chance = binomial(
            disease_model.get_exposure_chance(
                self.disease_status == DiseaseStatus::Vaccinated,
                mask_status,
            ),
            exposure_total as u8,
        );

        if self.disease_status == DiseaseStatus::Susceptible && rng.gen::<f64>() < exposure_chance {
            self.disease_status = DiseaseStatus::Exposed(0);
            return true;
        }
        false
    }
    pub fn set_workplace_code(&mut self, workplace_code: BuildingCode) {
        self.workplace_code = workplace_code;
    }
    pub fn occupation(&self) -> OccupationType {
        self.occupation
    }

    pub fn is_susceptible(&self) -> bool {
        self.disease_status == DiseaseStatus::Susceptible
    }
}

impl Display for Citizen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Citizen {} Has Disease Status {}, Is Currently Located At {}, Resides at {}, Works at {}", self.id, self.disease_status, self.current_position, self.household_code, self.workplace_code)
    }
}

/// The type of employment a Citizen can have
#[derive(Debug)]
pub enum WorkType {
    Normal,
    Essential,
    Unemployed,
    Student,
    NA,
}
/*
#[cfg(test)]
mod tests {
    use load_census_data::tables::occupation_count::OccupationType;
    use load_census_data::tables::population_and_density_per_output_area::AreaClassification;
    use crate::disease::DiseaseModel;
    use crate::models::building::BuildingCode;
    use crate::models::citizen::Citizen;

    pub fn check_citizen_schedule() {
        let work = BuildingCode::new("A".to_string(), AreaClassification::UrbanCity);
        let home = BuildingCode::new("B".to_string(), AreaClassification::UrbanCity);
        let mut citizen = Citizen::new(home, work, OccupationType::Sales);
        let disease = DiseaseModel::covid();
        for hour in 0..100 {
            citizen.execute_time_step(hour, &disease);
            if citizen.w
        }
    }
}*/
