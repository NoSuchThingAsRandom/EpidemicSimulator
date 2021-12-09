import json

import more_itertools as mit


def find_ranges(iterable):
    """Yield range of consecutive numbers."""
    for group in mit.consecutive_groups(iterable):
        group = list(group)
        if len(group) == 1:
            yield group[0]
        else:
            yield group[0], group[-1]


if __name__ == "__main__":
    data = open("yorkshire_and_humber_codelists.json")
    data = json.load(data)
    data = data["structure"]
    data = data["codelists"]
    data = data["codelist"]
    data = data[0]
    data = data["code"]
    codes = set()
    for code in data:
        code = code["value"]
        # code=code["annotation"]
        # for an in code:
        # if an["annotationtitle"]=="GeogCode":
        f = (int(code))
        codes.add(f)
    # print(codes)
    print(len(codes))
    print(min(codes))
    print(max(codes))
    print((list(find_ranges(codes))))
    # print([list(group) for group in mit.consecutive_groups(codes)])
    # print(list(gb))
    # all_groups = ([i[1] for i in g] for _, g in gb)
    # test=[x[0] for x in gb]
    # print(list(test))
    # print(list(all_groups))
    # print(len((all_groups)))
    # print(min(codes))
    # print(max(codes))
    # code=code["annotations"]
    # print(data)#{"structure"}{"codelists"}{"codelist"}{"code"})
