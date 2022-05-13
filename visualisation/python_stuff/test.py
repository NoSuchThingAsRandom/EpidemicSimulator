#  Epidemic Simulation Using Census Data (ESUCD)
#  Copyright (c)  2022. Sam Ralph
#
#  This file is part of ESUCD.
#
#  ESUCD is free software: you can redistribute it and/or modify
#  it under the terms of the GNU General Public License as published by
#  the Free Software Foundation, version 3 of the License.
#
#  ESUCD is distributed in the hope that it will be useful,
#  but WITHOUT ANY WARRANTY; without even the implied warranty of
#  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#  GNU General Public License for more details.
#
#  You should have received a copy of the GNU General Public License
#  along with ESUCD.  If not, see <https://www.gnu.org/licenses/>.
#
#
#  This file is part of ESUCD.
#
#  ESUCD is free software: you can redistribute it and/or modify
#  it under the terms of the GNU General Public License as published by
#  the Free Software Foundation, version 3 of the License.
#
#  ESUCD is distributed in the hope that it will be useful,
#  but WITHOUT ANY WARRANTY; without even the implied warranty of
#  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#  GNU General Public License for more details.
#
#  You should have received a copy of the GNU General Public License
#  along with ESUCD.  If not, see <https://www.gnu.org/licenses/>.
#

import pandas

pd = pandas.read_csv(
    "../../data/tables/1254162148...1254162748,1254262205...1254262240/ks608uk_occupation_count_NM_1518_1.csv")
print(pd.head())
