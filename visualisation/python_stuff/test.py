import matplotlib.pyplot as plt
import shapefile as shp  # Requires the pyshp package

sf = shp.Reader("../../data/census_map_areas/England_oa_2011/england_oa_2011.shp")

plt.figure()
for shape in sf.shapeRecords():
    x = [i[0] for i in shape.shape.points[:]]
    y = [i[1] for i in shape.shape.points[:]]
    plt.plot(x, y)
plt.show()
