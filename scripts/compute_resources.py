import csv
import json
import logging
import os
import re
import subprocess
from pathlib import Path

logging.basicConfig()
logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)

RESOURCES_PATH = Path("./resources/")
VERSIONS = ["v0", "v1"]
KAKAROT_VERSION = [
    x for x in os.getenv("KAKAROT_VERSION", "none").lower().split(",") if x in VERSIONS
]


def get_resource_usage(version: str):
    try:
        result = subprocess.run(
            f"make tests-{version}-ci",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            shell=True,
            check=True,
        )
        logger.info("\n" + result.stdout)
    except subprocess.CalledProcessError as e:
        logger.error(e.stdout)
        raise RuntimeError(f"Error while running ef-tests for version {version}") from e

    # Remove ANSI escape sequences
    cleaned_output = re.sub(r"\x1b\[[0-9;]*[a-zA-Z]", "", result.stdout)
    matches = re.findall(
        rf"ef_testing::models::result: (.*) {version} passed: .?ResourcesMapping\((.*)\)",
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
        test_resources = get_resource_usage(version)
        write_resources_to_csv(
            test_resources, output_file=RESOURCES_PATH / f"resources_{version}.csv"
        )


if __name__ == "__main__":
    main()
