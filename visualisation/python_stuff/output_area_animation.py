import json
import math
import time

import matplotlib.animation as anm
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import shapefile as shp  # Requires the pyshp package
from matplotlib import cm
from matplotlib.collections import PatchCollection
from matplotlib.patches import Polygon

MAX_INFECTED_VALUE = 2000
MIN_INFECTED_VALUE = 0
LOG_BASE = 2


def build_output_area_df() -> pd.DataFrame:
    with open("../../recordings/v1.0.0-test.json") as file:
        output_area_data = json.load(file)
    print(output_area_data.keys())

    output_areas = set(output_area_data["OutputArea"].keys())
    output_area_df = None
    for area in output_area_data["OutputArea"]:
        records = []
        for record in output_area_data["OutputArea"][area]:
            record["code"] = area
            records.append(record)
        if output_area_df is None:
            output_area_df = pd.DataFrame(records)
        else:
            output_area_df = pd.concat([output_area_df, pd.DataFrame(records)])

    print(output_area_df["code"].value_counts())
    return output_area_df


def build_polygons():
    sf = shp.Reader("../../data/census_map_areas/England_oa_2011/england_oa_2011.shp")

    output_area_polygons = {}
    areas = set(output_area_df["code"].unique())
    print("Reading data")
    for shape in sf.shapeRecords():
        code = shape.record.as_dict(date_strings=True)["code"]
        if code in areas:
            points = shape.shape.points[:]
            output_area_polygons[code] = Polygon(points, closed=True)
    print("Competed loop")
    return output_area_polygons


def build_patch(value, time_step=None):
    if time_step is not None:
        time_step = int(time_step)
    print("Building graph for ", value, " at index: ", str(time_step))
    patches = []
    poly_colors = []
    for (code, poly) in output_area_polygons.items():
        if time_step is None:
            colour_ranking = math.log(
                output_area_df.loc[output_area_df["code"] == code][value].max() / MAX_INFECTED_VALUE, LOG_BASE)
        else:
            colour_ranking = math.log((1 / LOG_BASE) +
                                      output_area_df.loc[(output_area_df["time_step"] == time_step) & (
                                              output_area_df["code"] == code)][
                                          value] / MAX_INFECTED_VALUE, LOG_BASE)
        patches.append(poly)
        poly_colors.append(colour_ranking)
    collection = PatchCollection(patches, edgecolors="black")
    return collection, poly_colors


def get_colours(value, time_step=None):
    if time_step is not None:
        time_step = int(time_step)
    poly_colors = []
    min_val = 0
    for (code, poly) in output_area_polygons.items():
        if time_step is None:
            val = (
                    output_area_df.loc[output_area_df["code"] == code][
                        value].max() / MAX_INFECTED_VALUE)
            min_val = min(val, min_val)
            colour_ranking = math.log((1 / LOG_BASE) + val, LOG_BASE)
        else:
            colour_ranking = math.log((1 / LOG_BASE) + (
                    output_area_df.loc[(output_area_df["time_step"] == time_step) & (
                            output_area_df["code"] == code)][
                        value] / MAX_INFECTED_VALUE), LOG_BASE)
        poly_colors.append(colour_ranking)
    print(min_val)
    return poly_colors


def plot(value: str, collection: PatchCollection, poly_colours: [float], ax=None, time_step=None):
    max = output_area_df[value].max()
    min = output_area_df[value].max()
    mpl
    cmap = cm.get_cmap("magma")  # .reversed()
    if ax is None:
        ax = plt.gca()
    # fig,ax=plt.subplots()
    ax.add_collection(collection)
    ax.autoscale()
    colors = cmap(poly_colours)
    collection.set_facecolor(colors)
    # ax.colorbar(collection,label="Infected cases")
    ax.set_title("Hour")
    collection.set_clim([min, max])

    if time_step is None:
        ax.set_title(str("Maximum " + str(value) + " cases per Output Area"))
    else:
        ax.set_title(str(str(value) + " cases at time step" + str(time_step) + " per Output Area"))
    # plt.show()


def animate(frame):
    print("Building frame: ", str(frame))
    hour = selected_hours[frame]
    poly_colours = get_colours("infected", hour)
    colors = cmap(poly_colours)
    patches.set_facecolor(colors)
    return patches


if __name__ == "__main__":
    total_time = time.time()
    func_time = time.time()
    output_area_df = build_output_area_df()
    print("Loaded sim data in: ", time.time() - func_time)
    func_time = time.time()

    output_area_polygons = build_polygons()
    print("Loaded poly data in: ", time.time() - func_time)
    func_time = time.time()
    colours = get_colours("infected")
    print(min(colours))
    print(max(colours))
    print(colours)
    exit()
    frames = 100
    selected_hours = np.linspace(output_area_df.time_step.min(), output_area_df.time_step.max(), num=frames)
    plt.axis("off")
    fig, ax = plt.subplots(1, 1, figsize=(10, 10))

    patches, colour = build_patch("infected")
    # ax.colorbar(patches, )
    print("Built patches in: ", time.time() - func_time)
    func_time = time.time()

    cmap = cm.get_cmap("magma")
    ax.add_collection(patches)
    ax.autoscale()
    print("Starting render...")

    anim = anm.FuncAnimation(fig, animate, frames=frames, interval=1000, blit=False)
    print("Displaying: ", time.time() - func_time)
    plt.show()
    # anim.save("test.gif", writer="imagemagick")
    print("Saved in: ", time.time() - func_time)
    print("Finished in: ", time.time() - total_time)
    # output_area_heatmap("infected")
