import matplotlib.pyplot as plt
import regex

regex = regex.compile(r"(?<=(([a-zA-Z\s]*: )))(\d.\d{3})")
fig, axes = plt.subplots(3, 2)  # , wspace=1, hspace=1)
fig.set_size_inches(24, 24)
for index, name in enumerate(["log.log", "log_improved_perf.log", "log_multithreaded_exposure_gen.log"]):
    generate_time, apply_time, intervention_time = [], [], []
    with open("york_performance_logs/" + name) as logs:
        for line in logs:
            result = regex.split(line)
            if len(result) > 1 and "Generate Exposures" in line:
                generate_time.append(float(result[3]))
                apply_time.append(float(result[7]))
                intervention_time.append(float(result[11]))

    total_time = list(map(sum, zip(*[generate_time, apply_time, intervention_time])))
    time_step = range(len(generate_time))
    time_per_step = {"Exposure Generation Time (seconds)": generate_time,
                     "Exposure Application Time (seconds)": apply_time,
                     "Intervention Time (seconds)": intervention_time}
    axes[index][0].stackplot(time_step, time_per_step.values(), labels=time_per_step.keys())
    axes[index][0].set_title(name)
    axes[index][0].legend()
    axes[0][1].plot(generate_time, label=name)
    axes[1][1].plot(apply_time, label=name)
    axes[2][1].plot(intervention_time, label=name)
axes[0][1].legend()
axes[1][1].legend()
axes[2][1].legend()

axes[0][1].set_title("Generate time")
axes[1][1].set_title("Apply Time")
axes[2][1].set_title("Intervention Time")
# plt.plot(range(len(generate_time)),generate_time,label="Exposure Generation Time (seconds)")
# plt.plot(range(len(apply_time)),apply_time,label="Exposure Application Time (seconds)")
# plt.plot(range(len(intervention_time)),intervention_time,label="Intervention Time (seconds)")
# plt.plot(range(len(total_time)),total_time,label="Total Time (seconds)")
plt.legend()
plt.show()
# print("Areas:\n\n\n\n")
# print(areas)
