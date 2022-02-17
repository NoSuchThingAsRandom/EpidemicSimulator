index = 0
areas = set()
with open("log.log") as logs:
    for line in logs:
        line: str = str(line)
        line: [str] = line.split(">")
        if len(line) > 1:
            line = line[1]
            id = line.split("Cannot find output area ID: ")
            if len(id) > 1:
                id = id[1]
            else:
                continue
            print(id)
            id = line.split(",")
            if len(id) > 1:
                id = id[0]
                areas.add(id)
            else:
                continue
        else:
            continue
        index += 1
        if index == 50:
            break

print("Areas:\n\n\n\n")
print(areas)
