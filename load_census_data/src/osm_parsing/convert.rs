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
//! This is used to convert latitude and longitude to grid coordinates (National Grid Ordnance Survey - OGB36)
pub fn decimal_latitude_and_longitude_to_coordinates(latitude: f64, longitude: f64) -> (f64, f64) {
    wgs84_to_osbg36(lat_lon_to_cartesian(latitude, longitude))
}

pub fn main() {
    //let york_point = (460815.52, 452001.17);
    //let lat_lon = (53.960000, -1.078530);
    //let lat_lon = (53.36, -1.39);
    //let lat_lon=(52.39,-1.43);
    //println!("{:?}", 1);
    //let f=lat_lon_to_cartesian(53.36431653,1.39519920);
    // Degrees: 53 36 43.1653 N, 001 39 51.9920 W =  53.61199,-1.664442
    // Degrees: 52 39 27.2531 N, 001 43 04.5177 E = 52.65757,1.717922
    //let f=lat_lon_to_cartesian();
    let lat = 53.61199;
    let lon = -1.664442;
    println!("Lat: {}, Lon: {}", lat, lon);
    println!("---------------");
    let (x, y, z) = lat_lon_to_cartesian(lat, lon);
    println!("Cartesian: ({}, {}, {})", x, y, z);
    println!("---------------");
    println!("{:?}", wgs84_to_osbg36((x, y, z)));
}

pub fn degrees_to_decimal(_coord: String) -> f64 {
    //https://support.goldensoftware.com/hc/en-us/articles/228362688-Convert-Degrees-Minutes-Seconds-To-Decimal-Degrees-in-Strater
    todo!()
}

/// Converts an angle in seconds, to radians
fn seconds_to_radians(second: f64) -> f64 {
    second * (std::f64::consts::PI / (180.0 * 60.0 * 60.0))
}

/// Converts a latitude and longitude in degree format, to cartesian (X,Y,Z)
///
///https://www.ordnancesurvey.co.uk/documents/resources/guide-coordinate-systems-great-britain.pdf - B.1
fn lat_lon_to_cartesian(lat: f64, lon: f64) -> (f64, f64, f64) {
    let lat_radians = lat.to_radians();
    let lon_radians = lon.to_radians();
    let lat_sin = lat_radians.sin();
    let lat_cos = lat_radians.cos();
    let lon_sin = lon_radians.sin();
    let lon_cos = lon_radians.cos();
    let a: f64 = 6378137.000;
    let b: f64 = 6356752.3141;
    let e2: f64 = (a.powf(2.0) - b.powf(2.0)) / a.powf(2.0);
    let h = 299.8;
    let v = a / ((1.0 - e2 * lat_sin * lat_sin).sqrt());
    let x = (v + h) * lat_cos * lon_cos;
    let y = (v + h) * lat_cos * lon_sin;
    let z = ((1.0 - e2) * v + h) * lat_sin;
    (x, y, z)
}

/// These values are converted from secs to radians
///
///
const S: f64 = 20.4894 * 0.000001;
const RX: f64 = -0.0000007282;
// seconds_to_radians(-0.1502);
const RY: f64 = -0.000001197;
// seconds_to_radians(-0.2470);
const RZ: f64 = -0.000004083;
//seconds_to_radians(-0.8421);
/// The transform matrix
const T: [[f64; 1]; 3] = [[-446.448], [125.157], [-542.060]];
/// The rotation and scale matrix (in radians)
const R: [[f64; 3]; 3] = [[1.0 + S, (-RZ), RY], [RZ, 1.0 + S, -RX], [-RY, RX, 1.0 + S]];

const NORTH_OFFSET: f64 = -100000.0;
const EAST_OFFSET: f64 = 400000.0;

/// Converts latitude and longitude  (decimal format) to National Grid  Coordinates (used in the Output Areas)
///
///https://www.ordnancesurvey.co.uk/documents/resources/guide-coordinate-systems-great-britain.pdf - C.1
fn wgs84_to_osbg36(point: (f64, f64, f64)) -> (f64, f64) {
    let p = ndarray::arr2(&[[point.0], [point.1], [point.2]]);
    let r = ndarray::arr2(&R);
    let output = ndarray::arr2(&T) + r.dot(&p);
    let output = (output[[0, 0]], output[[1, 0]]);
    (output.0 + NORTH_OFFSET, output.1 + EAST_OFFSET)
}
