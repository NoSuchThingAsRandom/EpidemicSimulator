# Format

The `data` directory contains two folders

* `map`
    - This contains subdirectories for sections of the output area shape files
* `tables`
    - This has subdirectories for each collection of Output Areas to run
    - i.e. England, Yorkshire and The Humber, York, etc
        * For each of these directories, all the required census tables are stored in csv format

i.e.\
England Shape Files:\
`data/map/England_oa_2011/england_oa_2011.shp`\
`data/map/England_oa_2011/england_oa_2011.shx`\
`data/map/England_oa_2011/england_oa_2011.dbf`\
`data/map/England_oa_2011/england_oa_2011.prj`\

England Census Tables:\
`data/tables/2092957699TYPE299/ks101ew.csv` - Population\
`data/tables/2092957699TYPE299/ks608uk.csv` - Occupation Counts\
`data/tables/2092957699TYPE299/wf01bew.csv` - Home To Workplace

Yorkshire And The Humber Census Tables:\
`data/tables/2013265923TYPE299/ks101ew.csv` - Population\
`data/tables/2013265923TYPE299/ks608uk.csv` - Occupation Counts\
`data/tables/2013265923TYPE299/wf01bew.csv` - Home To Workplace

## Areas:

2013265923TYPE299 - All Output Areas within Yorkshire and the Humber 1946157112TYPE299 - All Output Areas within York

https://www.nomisweb.co.uk/api/v01/dataset/NM_144_1.data.csv?date=latest&geography=1254162148...1254162748,1254262205...1254262240,1237321401...1237321422&rural_urban=0&cell=0&measures=20100

# OSM Data

The entire region of England is downloaded from: https://download.geofabrik.de/europe/great-britain/england.html

Size: ~1.1GB

There are 121003660 dense nodes (Points of interest)

# Table 1 - Usual resident population

https://www.nomisweb.co.uk/census/2011/ks101ew

API CODE: https://www.nomisweb.co.uk/api/v01/dataset/NM_144_1.data.csv/summary

|GEOGRAPHY_NAME|GEOGRAPHY_TYPE|RURAL_URBAN_NAME|RURAL_URBAN_TYPECODE|CELL_NAME|MEASURES_NAME|OBS_VALUE|OBS_STATUS|RECORD_OFFSET|RECORD_COUNT|
|---|---|---|---|---|---|---|---|---|---|
|E00062207|2011 output areas|Total|2000|all usual residents|Value|242|A|0|35645376|
|E00062207|2011 output areas|Total|2000|all usual residents|Percent|100.0|A|1|35645376|
|E00062207|2011 output areas|Total|2000|Males|Value|116|A|2|35645376|
|E00062207|2011 output areas|Total|2000|Males|Percent|47.9|A|3|35645376|
|E00062207|2011 output areas|Total|2000|Females|Value|126|A|4|35645376|
|E00062207|2011 output areas|Total|2000|Females|Percent|52.1|A|5|35645376|
|E00062207|2011 output areas|Total|2000|Lives in a household|Value|242|A|6|35645376|
|E00062207|2011 output areas|Total|2000|Lives in a household|Percent|100.0|A|7|35645376|
|E00062207|2011 output areas|Total|2000|Lives in a communal establishment|Value|0|A|8|35645376|
|E00062207|2011 output areas|Total|2000|Lives in a communal establishment|Percent|0.0|A|9|35645376|
|E00062207|2011 output areas|Total|2000|Schoolchild or full-time student aged 4 and over at their non term-time address|Value|7|A|10|35645376|
|E00062207|2011 output areas|Total|2000|Schoolchild or full-time student aged 4 and over at their non term-time address|Percent||Q|11|35645376|
|E00062207|2011 output areas|Total|2000|Area (Hectares)|Value|865.24|A|12|35645376|
|E00062207|2011 output areas|Total|2000|Area (Hectares)|Percent||Q|13|35645376|
|E00062207|2011 output areas|Total|2000|Density (number of persons per hectare)|Value|0.3|A|14|35645376|
|E00062207|2011 output areas|Total|2000|Density (number of persons per hectare)|Percent||Q|15|35645376|

# Employment Densities

Page 9 of Employment Densities Guide: 2nd
Edition: https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/378203/employ-den.pdf
https://www.gov.uk/government/publications/employment-densities-guide

# Population Per Occupation

# Distance travelled to work:

https://www.nomisweb.co.uk/census/2011/wd702ew

Api Code: NM_154_1

Need to figure out size of output areas But can be used to spread out workers to different areas

# Home Residence to Work Place Position - wf01bew

Counts the number of workers in output areas, against the output area they reside in Used for modelling migration of
workers
https://www.nomisweb.co.uk/census/2011/wf01bew

Api Code: NM_1228_1

# Occupation Type Counts - ks608uk

https://www.nomisweb.co.uk/census/2011/ks608uk

Api Code: NM_1518_1
