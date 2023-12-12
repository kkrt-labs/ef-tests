import csv
import json
import logging
import os
import re
from pathlib import Path

logging.basicConfig()
logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)

RESOURCES_PATH = Path("./resources/")
VERSIONS = ["v0", "v1"]
KAKAROT_VERSION = [
    x for x in os.getenv("KAKAROT_VERSION", "none").lower().split(",") if x in VERSIONS
]


def get_resource_usage(path: str = "./test.out"):
    with open(path, "r") as f:
        result = f.read()
    # Remove ANSI escape sequences
    cleaned_output = re.sub(r"\x1b\[[0-9;]*[a-zA-Z]", "", result)
    matches = re.findall(
        r"ef_testing::models::result: (.*) passed: .?ResourcesMapping\((.*)\)",
        cleaned_output,
    )
    tests_resources = [
        {**json.loads(resources), "test": test_name} for test_name, resources in matches
    ]
    return tests_resources


def write_resources_to_csv(tests_resources: list, output_file: str = "resources.csv"):
    # Get all keys and skip "test" key
    keys = {
        key
        for resources in tests_resources
        for key in resources.keys()
        if key != "test"
    }
    # Re-add "test" key, in order to be sure its first
    keys = ["test"] + sorted(keys)

    with open(output_file, "w") as f:
        writer = csv.DictWriter(f, fieldnames=keys)

        writer.writeheader()

        # Write data to CSV
        for row in tests_resources:
            writer.writerow(row)


def main():
    os.makedirs(RESOURCES_PATH, exist_ok=True)

    for version in KAKAROT_VERSION:
        test_resources = get_resource_usage(f"./test_{version}.out")
        write_resources_to_csv(
            test_resources, output_file=RESOURCES_PATH / f"resources_{version}.csv"
        )


if __name__ == "__main__":
    main()
