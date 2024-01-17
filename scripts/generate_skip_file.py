import argparse
import re
from collections import defaultdict


def parse_and_write_to_yaml(input_file, output_file):
    with open(input_file, "r") as f:
        data = f.read()

    matches_failed = [
        (
            m.split("::")[-2]
            .replace("_minus_", "-")
            .replace("_plus_", "+")
            .replace("_xor_", "^"),
            m.split("::")[-1]
            .replace("_minus_", "-")
            .replace("_plus_", "+")
            .replace("_xor_", "^"),
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

    skip = "testname:\n"
    for folder in skip_dict:
        skip += f"  {folder}:\n"
        skip_dict[folder] = sorted(skip_dict[folder])
        for file in skip_dict[folder]:
            skip += f"      - {file[5:]}\n"

    with open(output_file, "w") as f:
        f.write(skip)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Generate skip file from ef tests logs"
    )
    parser.add_argument("input_file", help="Input file path")
    parser.add_argument("output_file", help="Output file path")

    args = parser.parse_args()

    input_file = args.input_file
    output_file = args.output_file

    parse_and_write_to_yaml(input_file, output_file)
