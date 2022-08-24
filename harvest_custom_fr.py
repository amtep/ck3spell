import re
import sys

custom_loc_txt = sys.argv[1];
custom_loc_yml = sys.argv[2];

keys = {}
key = None
for line in open(custom_loc_txt):
    line = re.sub("#.*", "", line).rstrip()
    if re.fullmatch("[A-Za-z_]+ = {", line):
        key, _ = line.split(" = ")
        keys[key] = []
        parent = None
        continue
    if re.fullmatch("\s+parent = [A-Za-z_]+", line):
        _, parent = line.split(" = ")
    if re.fullmatch("\s+suffix = [A-Za-z_]+", line):
        _, loc = line.split(" = ")
        if parent == "FR_gender_fake":
           keys[key].append("CustomLoc_FR_female_" + loc)
           keys[key].append("CustomLoc_FR_male_" + loc)

locs = {}
for line in open(custom_loc_yml):
    line = line.rstrip()
    match = re.fullmatch('\s+([A-Za-z_]+):0 "([^"]*)"', line)
    if match:
        locs[match.group(1)] = match.group(2)

for key in sorted(keys.keys()):
    if not keys[key]:
        continue
    loc_values = []
    for value in keys[key]:
        loc_values.append(locs[value])
    print("%s;%s" % (key, ";".join(loc_values)));
