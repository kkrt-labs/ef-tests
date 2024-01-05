import re
from collections import defaultdict

with open("data.txt", "r") as f:
    data = f.read()

matches_failed = [
    (
        m.split("::")[-2]
        .replace("_minus_", "-")
        .replace("_plus_", "+")
        .replace("_xor_", "^"),
        m.split("::")[-1],
    )
    for m in re.findall(r"thread '(.*)' panicked at", data)
]
summary = next(
    re.finditer(
        r"test result: (?P<result>\w+). (?P<passed>\d+) passed; (?P<failed>\d+) failed; (?P<ignored>\d+) ignored",
        data,
    )
)

if len(matches_failed) != int(summary["failed"]):
    raise ValueError("Failed to parse file")

skip_dict = defaultdict(list)
for [folder, file] in matches_failed:
    skip_dict[folder].append(file)

skip = "filename:\n"
for folder in skip_dict:
    skip += f"  {folder}:\n"
    for file in skip_dict[folder]:
        skip += f"      - {file[5:]}\n"

with open("blockchain-tests-skip.yml", "w") as f:
    f.write(skip)
