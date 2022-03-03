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
//! This is used to convert latitude and longitude to grid coordinates (National Grid Ordnance Survey - OGB36)
//!
//! Useful site: https://www.movable-type.co.uk/scripts/latlong-os-gridref.html
#![allow(dead_code, non_snake_case)]
struct Ellipsoid {
    a: f64,
    b: f64,
    e2: f64,
    f0: f64,
    map_x_origin: f64,
    map_y_origin: f64,
    true_x_origin: f64,
    true_y_origin: f64,
}

impl Ellipsoid {
    fn airy() -> Self {
        const A: f64 = 6377563.396;
        const B: f64 = 6356256.909;
        Self {
            a: A,
            b: B,
            e2: ((A * A) - (B * B)) / (A * A),
            f0: 0.9996012717,
            map_x_origin: 400000.0,
            map_y_origin: -100000.0,
            true_x_origin: 49.0,
            true_y_origin: -2.0,
        }
    }
    fn GRS80_zone_30() -> Self {
        const A: f64 = 6378137.000;
        const B: f64 = 6356752.3141;
        // Uses UTM Zone 30
        Self {
            a: A,
            b: B,
            e2: ((A * A) - (B * B)) / (A * A),
            f0: 0.9996,
            map_x_origin: 500000.0,
            map_y_origin: 0.0,

            true_x_origin: 0.0,
            true_y_origin: -3.0,
        }
    }
}

pub fn decimal_latitude_and_longitude_to_northing_and_eastings(
    latitude: f64,
    longitude: f64,
) -> (i32, i32) {
    let (x, y, z) = lat_lon_to_cartesian(latitude, longitude, Ellipsoid::GRS80_zone_30());
    let (x, y, z) = helmert_wgs84_to_osbg36((x, y, z));
    let (lat, lon) = cartesian_to_lat_lon(x, y, z, Ellipsoid::airy());
    let (northing, easting) = lat_lon_to_eastings(lat, lon, Ellipsoid::airy());
    f64_trimmed_to_isize((easting, northing))
}

/// Trims f64 coordinates to an isize
fn f64_trimmed_to_isize(position: (f64, f64)) -> (i32, i32) {
    (position.0.round() as i32, position.1.round() as i32)
}

/// Converts an angle in seconds, to radians
fn seconds_to_radians(second: f64) -> f64 {
    second * (std::f64::consts::PI / (180.0 * 60.0 * 60.0))
}

/// Converts an angle in seconds, to radians
fn radians_to_seconds(radians: f64) -> f64 {
    radians / (std::f64::consts::PI / (180.0 * 60.0 * 60.0))
}

/// Converts a latitude and longitude in degree format, to cartesian (X,Y,Z)
///
///https://www.ordnancesurvey.co.uk/documents/resources/guide-coordinate-systems-great-britain.pdf - B.1
fn lat_lon_to_cartesian(lat: f64, lon: f64, ellipsoid: Ellipsoid) -> (f64, f64, f64) {
    let lat_radians = lat.to_radians();
    let lon_radians = lon.to_radians();
    let lat_sin = lat_radians.sin();
    let lat_cos = lat_radians.cos();
    let lon_sin = lon_radians.sin();
    let lon_cos = lon_radians.cos();
    let h = 299.8;
    let v = ellipsoid.a / ((1.0 - ellipsoid.e2 * lat_sin * lat_sin).sqrt());
    let y = (v + h) * lat_cos * lon_sin;
    let x = (v + h) * lat_cos * lon_cos;
    let z = ((1.0 - ellipsoid.e2) * v + h) * lat_sin;
    (x, y, z)
}

/// Converts a cartesian (X,Y,Z) coordinate to latitude and longitude in degree format
///
///https://www.ordnancesurvey.co.uk/documents/resources/guide-coordinate-systems-great-britain.pdf - B.2
fn cartesian_to_lat_lon(x: f64, y: f64, z: f64, ellipsoid: Ellipsoid) -> (f64, f64) {
    let lon = (y / x).atan();
    let p = ((x * x) + (y * y)).sqrt();
    let mut lat = (z / (p * (1.0 - ellipsoid.e2))).atan();
    let mut lat_diff = 10.0;
    while lat_diff > 10.0_f64.powf(-15.0) {
        let v = ellipsoid.a / (1.0 - (ellipsoid.e2 * lat.sin() * lat.sin())).sqrt();
        let new_lat = ((z + (ellipsoid.e2 * v * (lat.sin()))) / p).atan();
        lat_diff = (new_lat - lat).abs();
        lat = new_lat;
    }
    (lat.to_degrees(), lon.to_degrees())
}

/// Converts a latitude and longitude in degree format to Northings and Eastings
///
/// https://www.ordnancesurvey.co.uk/documents/resources/guide-coordinate-systems-great-britain.pdf - C.1
fn lat_lon_to_eastings(lat: f64, lon: f64, ellipsoid: Ellipsoid) -> (f64, f64) {
    let lat_origin: f64 = ellipsoid.true_x_origin.to_radians();
    let lon_origin: f64 = ellipsoid.true_y_origin.to_radians();

    let lat_diff = lat.to_radians() - lat_origin;
    let lat_total = lat.to_radians() + lat_origin;

    let lon_diff = lon.to_radians() - lon_origin;
    let lon_diff2 = lon_diff * lon_diff;
    let lon_diff3 = lon_diff2 * lon_diff;
    let lon_diff4 = lon_diff3 * lon_diff;
    let lon_diff5 = lon_diff4 * lon_diff;
    let lon_diff6 = lon_diff5 * lon_diff;

    let lat_radians = lat.to_radians();
    let lat_sin = lat_radians.sin();
    let lat_cos = lat_radians.cos();
    let lat_tan = lat_radians.tan();

    let lat_cos3 = lat_cos * lat_cos * lat_cos;
    let lat_cos5 = lat_cos3 * lat_cos * lat_cos;

    let lat_tan2 = lat_tan * lat_tan;
    let lat_tan4 = lat_tan2 * lat_tan2;

    let n = (ellipsoid.a - ellipsoid.b) / (ellipsoid.a + ellipsoid.b);
    let n2 = n * n;
    let n3 = n2 * n;

    let V = ellipsoid.a * ellipsoid.f0 * ((1.0 - ellipsoid.e2 * lat_sin * lat_sin).powf(-0.5));
    let p = ellipsoid.a
        * ellipsoid.f0
        * (1.0 - ellipsoid.e2)
        * ((1.0 - ellipsoid.e2 * lat_sin * lat_sin).powf(-1.5));
    let N2 = (V / p) - 1.0;

    let ma = (1.0 + n + (1.25 * n2) + (1.25 * n3)) * (lat_diff);
    let mb = (3.0 * n + 3.0 * n2 + (21.0 / 8.0) * n3) * (lat_diff.sin()) * (lat_total.cos());

    let mc = (((15.0 / 8.0) * n2) + ((15.0 / 8.0) * n3))
        * ((2.0 * lat_diff).sin())
        * ((2.0 * lat_total).cos());
    let md = (35.0 / 24.0) * n3 * ((3.0 * lat_diff).sin()) * ((3.0 * lat_total).cos());
    let m = ellipsoid.b * ellipsoid.f0 * (ma - mb + mc - md);

    let i: f64 = m + ellipsoid.map_y_origin;
    let ii = (V / 2.0) * lat_sin * lat_cos;
    let iii = (V / 24.0) * lat_sin * lat_cos3 * (5.0 - (lat_tan2) + 9.0 * N2);
    let iiia = (V / 720.0) * lat_sin * lat_cos5 * (61.0 - (58.0 * lat_tan2) + (lat_tan4));
    let iv = V * lat_cos;
    let v = (V / 6.0) * lat_cos3 * ((V / p) - (lat_tan2));
    let vi = (V / 120.0)
        * lat_cos5
        * (5.0 - (18.0 * lat_tan2) + (lat_tan4) + (14.0 * N2 * N2) - (58.0 * lat_tan2 * N2 * N2));

    let northing = i + (ii * lon_diff2) + (iii * lon_diff4) + (iiia * lon_diff6);
    let easting = ellipsoid.map_x_origin + (iv * lon_diff) + (v * lon_diff3) + (vi * lon_diff5);
    (northing, easting)
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

const AIRY_NORTH_OFFSET: f64 = -100000.0;
const AIRY_EAST_OFFSET: f64 = 400000.0;

/// Converts latitude and longitude  (decimal format) to National Grid  Coordinates (used in the Output Areas)
///
///https://www.ordnancesurvey.co.uk/documents/resources/guide-coordinate-systems-great-britain.pdf - C.1
fn helmert_wgs84_to_osbg36(point: (f64, f64, f64)) -> (f64, f64, f64) {
    let p = ndarray::arr2(&[[point.0], [point.1], [point.2]]);
    let r = ndarray::arr2(&R);
    let output = ndarray::arr2(&T) + r.dot(&p);
    let output = (output[[0, 0]], output[[1, 0]], output[[2, 0]]);
    (output.0, output.1, output.2)
}

#[cfg(test)]
mod tests {
    use crate::convert::{
        cartesian_to_lat_lon, decimal_latitude_and_longitude_to_northing_and_eastings,
        Ellipsoid, helmert_wgs84_to_osbg36, lat_lon_to_cartesian, lat_lon_to_eastings,
    };

    #[test]
    pub fn test_wgs84_to_osbg36() {
        let point = (3790644.900, -110149.210, 5111482.970);
        let new_point = helmert_wgs84_to_osbg36(point);
        let target = (3790269.549, -110038.064, 5111050.261);
        assert!(new_point.0 - target.0 < 0.1);
        assert!(new_point.1 - target.1 < 0.1);
        assert!(new_point.2 - target.2 < 0.1);
    }

    #[test]
    fn test_grs80_lat_lon_to_cartesian() {
        let desired_accuracy = 0.05;
        let lat = 53.61199; // 53 36 43.1653 N
        let lon = -1.664442; // 001 39 51.9920 W
        let (x, y, z) = lat_lon_to_cartesian(lat, lon, Ellipsoid::GRS80_zone_30());
        let expected_x = 3790644.90;
        let diff_x = (x - expected_x).abs();
        assert!(
            diff_x < desired_accuracy,
            "X Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            x,
            expected_x,
            diff_x
        );

        let expected_y = -110149.21;
        let diff_y = (y - expected_y).abs();
        assert!(
            diff_y < desired_accuracy,
            "Y Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            y,
            expected_y,
            diff_y
        );

        let expected_z = 5111482.97;
        let z_diff = (z - expected_z).abs();
        assert!(
            z_diff < desired_accuracy,
            "Z Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            z,
            expected_z,
            z_diff
        );
    }

    #[test]
    fn test_grs80_cartesian_to_lat_lon() {
        let desired_accuracy = 0.05;
        let x = 3790644.900;
        let y = -110149.210;
        let z = 5111482.970;
        let (lat, lon) = cartesian_to_lat_lon(x, y, z, Ellipsoid::GRS80_zone_30());
        let expected_lat = 53.61199; // 53 36 43.1653 N

        let diff_lat = (lat - expected_lat).abs();
        assert!(
            diff_lat < desired_accuracy,
            "Lat Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            lat,
            expected_lat,
            diff_lat
        );

        let expected_lon = -1.664442; // 001 39 51.9920 W
        let diff_lon = (lon - expected_lon).abs();
        assert!(
            diff_lon < desired_accuracy,
            "Lon Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            lon,
            expected_lon,
            diff_lon
        );
    }

    #[test]
    fn test_airy_cartesian_to_lat_lon() {
        let desired_accuracy = 0.05;
        let x = 3790269.549;
        let y = -110038.064;
        let z = 5111050.261;
        let (lat, lon) = cartesian_to_lat_lon(x, y, z, Ellipsoid::airy());
        let expected_lat = 53.611749; // 53 36 42.2972 N

        let diff_lat = (lat - expected_lat).abs();
        assert!(
            diff_lat < desired_accuracy,
            "Lat Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            lat,
            expected_lat,
            diff_lat
        );

        let expected_lon = -1.662928; // 001 39 46.5416 W
        let diff_lon = (lon - expected_lon).abs();
        assert!(
            diff_lon < desired_accuracy,
            "Lon Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            lon,
            expected_lon,
            diff_lon
        );
    }

    #[test]
    fn test_lat_lon_to_eastings() {
        let desired_accuracy = 0.05;
        let lat = 52.65757; // 52 39 27.2531 N
        let lon = 1.717922; // 001 43 04.5177 E
        let (northing, easting) = lat_lon_to_eastings(lat, lon, Ellipsoid::airy());
        let expected_northing = 313177.270;
        let diff_northing = (northing - expected_northing).abs();
        assert!(
            diff_northing < desired_accuracy,
            "Northing Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            northing,
            expected_northing,
            diff_northing
        );

        let expected_easting = 651409.903;
        let diff_easting = (easting - expected_easting).abs();
        assert!(
            diff_easting < desired_accuracy,
            "Easting Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            easting,
            expected_easting,
            diff_easting
        );
    }

    #[test]
    fn test_conversion() {
        let desired_accuracy = 0.05;
        let lat = 53.61199; // 53 36 43.1653 N
        let lon = -1.664442; // 001 39 51.9920 W
        println!("Starting Lat/Lon: {}, {}", lat, lon);
        let (x, y, z) = lat_lon_to_cartesian(lat, lon, Ellipsoid::GRS80_zone_30());
        println!("Cartesian: {}, {}, {}", x, y, z);
        let (x, y, z) = helmert_wgs84_to_osbg36((x, y, z));
        println!("Helmert Transformation(Cartesian): {}, {}, {}", x, y, z);
        let (lat, lon) = cartesian_to_lat_lon(x, y, z, Ellipsoid::airy());
        println!("Helmert Transformation(Lat/Lon): {}, {}", lat, lon);
        let (northing, easting) = lat_lon_to_eastings(lat, lon, Ellipsoid::airy());
        println!("Northings/Eastings: {}, {}", northing, easting);

        let expected_northing = 412878.741;
        let diff_northing = (northing - expected_northing).abs();
        assert!(
            diff_northing < desired_accuracy,
            "Northing Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            northing,
            expected_northing,
            diff_northing
        );

        let expected_easting = 422297.792;
        let diff_easting = (easting - expected_easting).abs();
        assert!(
            diff_easting < desired_accuracy,
            "Easting Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            easting,
            expected_easting,
            diff_easting
        );
    }

    #[test]
    fn test_decimal_latitude_and_longitude_to_northing_and_eastings() {
        let desired_accuracy = 0;
        let lat = 53.61199; // 53 36 43.1653 N
        let lon = -1.664442; // 001 39 51.9920 W
        println!("Starting Lat/Lon: {}, {}", lat, lon);
        let (easting, northing) = decimal_latitude_and_longitude_to_northing_and_eastings(lat, lon);
        println!("Northings/Eastings: {}, {}", northing, easting);
        let expected_northing = 412878;
        let diff_northing = (northing - expected_northing);
        assert_eq!(
            diff_northing, desired_accuracy,
            "Northing Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            northing, expected_northing, diff_northing
        );

        let expected_easting = 422297;
        let diff_easting = (easting - expected_easting);
        assert_eq!(
            diff_easting, desired_accuracy,
            "Easting Coordinate is incorrect, actual: {}, expected: {}, difference: {}",
            easting, expected_easting, diff_easting
        );
    }
}
