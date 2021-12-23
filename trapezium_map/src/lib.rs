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

// https://www.ti.inf.ethz.ch/ew/lehre/CG12/lecture/Chapter%205.pdf
// https://www.ti.inf.ethz.ch/ew/lehre/CG12/lecture/Chapter%209.pdf
// http://homepages.math.uic.edu/~jan/mcs481/pointlocation.pdf
// https://www2.cs.sfu.ca/~binay/813.2011/Trapezoidation.pdf
// https://stackoverflow.com/questions/1901139/closest-point-to-a-given-point
// https://www.cs.umd.edu/class/spring2020/cmsc754/Lects/lect08-trap-map.pdf
// http://cgm.cs.mcgill.ca/~athens/cs507/Projects/2002/JukkaKaartinen/
pub struct TrapeziumMap<T: geo_types::CoordNum> {
    //pub segments: Vec<geo_types::Line<T>>,
    pub bounding_box: geo_types::Rect<T>,
}

impl<T: geo_types::CoordNum> TrapeziumMap<T> {
    pub fn new(bounding_box: geo_types::Rect<T>) -> TrapeziumMap<T> {
        TrapeziumMap { bounding_box }
    }
    pub fn construct(&mut self, segments: Vec<geo_types::Line<T>>) {
        for _seg in segments {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
