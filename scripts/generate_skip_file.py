import argparse
import re
from collections import defaultdict


def format_into_identifier(s: str, is_pyspec: bool = False) -> str:
    if is_pyspec:
        test_name = s.split("/")[-1].split("::")[-1]
        test_name = (
            test_name.replace("test_", "")
            .replace("(", "_lpar_")
            .replace(")", "_rpar_")
            .replace("[", "__")
            .replace("]", "")
            .replace("-", "_minus_")
            .replace(" ", "_")
            .replace(".", "_")
            .split(",")
        )
        test_name = "_".join(part.strip() for part in test_name)
        return test_name
    else:
        return (
            s.replace("-", "_minus_")
            .replace("+", "_plus_")
            .replace("^", "_xor_")
        )


def extract_runresource_failures(input_file):
    failing_tests = []
    last_reverted = None

    with open(input_file, "r") as file:
        for line in file:
            if "reverted:" in line:
                test_name_line = line
                is_pyspec = ".py" in test_name_line
                test_name_raw = test_name_line.split("reverted:")[0].split("::")[-1].strip()
                test_name = format_into_identifier(test_name_raw, is_pyspec)
                last_reverted = test_name
            # If we find a line that says "RunResources has no remaining steps." after the last reverted test,
            # we know that the test failed due to a RunResources error
            elif (
                "RunResources has no remaining steps." in line
                and last_reverted is not None
            ):
                failing_tests.append(last_reverted)
                last_reverted = None

    return failing_tests


def parse_and_write_to_yaml(input_file, output_file):
    with open(input_file, "r") as f:
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

    runresources_errors = extract_runresource_failures(input_file)

    skip_dict = defaultdict(list)
    for [folder, file] in matches_failed:
        skip_dict[folder].append(file)

    skip = "testname:\n"
    for folder in skip_dict:
        skip += f"  {folder}:\n"
        skip_dict[folder] = sorted(skip_dict[folder])
        for file in skip_dict[folder]:
            if file[5:] in runresources_errors:
                skip += f"      - {file[5:]}  #RunResources error\n"
            else:
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
