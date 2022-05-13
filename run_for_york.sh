#
# Epidemic Simulation Using Census Data (ESUCD)
# Copyright (c)  2022. Sam Ralph
#
# This file is part of ESUCD.
#
# ESUCD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, version 3 of the License.
#
# ESUCD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with ESUCD.  If not, see <https://www.gnu.org/licenses/>.
#
#

export RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
export RAYON_NUM_THREADS=40
export RUST_BACKTRACE=full
version="v1.7"
machine="workstation"
area="1946157112TYPE299"
full_path="statistics_results/"$machine"/"$version"/"$area"/"
for index in {1..1}
do
  output=$full_path$index"/"
  mkdir -p $output
  echo "Saving results to: '"$output"'"
  cargo run --release -- $area --directory=data --grid-size=250000 --use-cache --simulate --output_name=$output 2>&1 | tee $output"log.log"
done

#2013265923TYPE299
#1946157112TYPE299
