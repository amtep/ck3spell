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
        continue
    if re.fullmatch("\s+localization_key = [A-Za-z_]+", line):
        _, loc = line.split(" = ")
        keys[key].append(loc)

locs = {}
for line in open(custom_loc_yml):
    line = line.rstrip()
    match = re.fullmatch('\s+([A-Za-z_]+):0 "([^"]*)"', line)
    if match:
        locs[match.group(1)] = match.group(2)

SKIP_KEYS = set([
  # defined in .txt but not in .yml
  "ES_ElLa_Relation",
  "ES_UxOgaresa",
  "ES_LeLaOpp",
  "ES_RleRlaOpp",
  "ES_SirvienteSirvienta",
  # defined in .txt and even used, but not in .yml
  "ES_OsAs",
])

for key in sorted(keys.keys()):
    if not keys[key]:
        print("empty key", key)
    if key in SKIP_KEYS:
        continue;
    loc_values = []
    for value in keys[key]:
        loc_values.append(locs[value])
    print("%s;%s" % (key, ";".join(loc_values)));
