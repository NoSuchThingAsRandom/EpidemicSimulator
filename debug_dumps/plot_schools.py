import json

import matplotlib.pyplot as plt
import numpy.random


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

# plot_teacher_possibilites()
