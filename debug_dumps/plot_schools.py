import json

import matplotlib.pyplot as plt
import numpy.random
import pandas as pd
from matplotlib.collections import PatchCollection
from matplotlib.patches import Polygon


def plot_school_locations():
    colours = ["red", "blue"]
    filenames = ["teacher_schools.json", "student_schools.json"]
    fig, axes = plt.subplots(2)
    fig.set_size_inches(25, 25)
    # ax = plt.axes(projection="3d")
    box_size = 200

    z_heights = {}
    keys = [set(), set()]

    for index, name in enumerate(filenames):
        file = open(name)
        data = json.load(file)
        points = []
        print(len(data))
        axes[index].bar(data.keys(), data.values())
        axes[index].set_title(name)
        for coord, value in data.items():
            x, y = eval(coord)
            keys[index].add(coord)
            points.append((x, y, value))
        print(name)
        z_bottom = []
        for point in points:
            x, y, z = point
            if index == 0:
                z = z * 1

            if (x, y) in z_heights:
                offset = z_heights[(x, y)]
                z_bottom.append(offset)
                z_heights[(x, y)] += z
            else:
                z_bottom.append(0)
                z_heights[(x, y)] = z
        x, y, z = zip(*points)
        # ax.bar3d(x, y, z_bottom, box_size, box_size, z, shade=True, label=name, color=colours[index])

    # ax.legend(loc="upper left")
    # teacher_data = map(lambda entry: , teacher_data.items())
    plt.show()
    shared = keys[0].intersection(keys[1])
    print(len(shared))
    print(shared)


def plot_teacher_possibilites():
    file = open("pre_duplicate_removal/teacher_school_possiblities.json")
    data = json.load(file)
    plt.boxplot(data)
    plt.scatter(0.9 + (numpy.random.random(len(data)) / 5), data)
    plt.show()


def build_output_areas() -> PatchCollection:
    with open("../recordings/v1.0.0-test.json") as file:
        output_area_polygons = json.load(file)
    print(output_area_polygons.keys())

    output_areas = set(output_area_polygons["OutputArea"].keys())
    output_area_df = None
    for area in output_area_polygons["OutputArea"]:
        records = []
        for record in output_area_polygons["OutputArea"][area]:
            record["code"] = area
            records.append(record)
        if output_area_df is None:
            output_area_df = pd.DataFrame(records)
        else:
            output_area_df = pd.concat([output_area_df, pd.DataFrame(records)])

    # output_area_df.append()
    # output_areas
    output_area_df["code"].value_counts()

    import shapefile as shp
    sf = shp.Reader("../data/census_map_areas/England_oa_2011/england_oa_2011.shp")

    output_area_polygons = {}
    for shape in sf.shapeRecords():
        code = shape.record.as_dict(date_strings=True)["code"]
        if code in output_areas:
            points = shape.shape.points[:]
            output_area_polygons[code] = points  #

    #
    patches = []
    # Draw Background of Output Areas
    for (code, poly) in output_area_polygons.items():
        patches.append(Polygon(poly, closed=True))
    output_collection = PatchCollection(patches, edgecolors="black", facecolors="red")
    return output_collection


def plot_school_outlines():
    output_collection = build_output_areas()
    fig, axes = plt.subplots()
    fig.set_dpi(400)
    fig.set_size_inches(20, 20)
    axes.add_collection(output_collection)

    file = open("locations/schools/raw_schools_locations.json")
    data = json.load(file)
    school_patches = []
    for school, outline in data:
        points = []
        for p in outline:
            # p=eval(p)
            points.append((p["x"], p["y"]))
        school_patches.append(Polygon(points, closed=True))
    collection = PatchCollection(school_patches, edgecolors="black", facecolors="dodgerblue")
    axes.add_collection(collection)
    fig.show()
    plt.autoscale()
    plt.show()


# plot_teacher_possibilites()

def plot_school_positions():
    output_collection = build_output_areas()
    fig, axes = plt.subplots()
    fig.set_dpi(400)
    fig.set_size_inches(20, 20)
    axes.add_collection(output_collection)

    file = open("missing_schools.json")
    data = json.load(file)
    xs = []
    ys = []
    for school in data:
        print(school)
        x, y = eval(school)
        xs.append(x)
        ys.append(y)
    plt.scatter(xs, ys)
    plt.show()


# plot_school_outlines()
# plot_school_positions()
plot_school_locations()
