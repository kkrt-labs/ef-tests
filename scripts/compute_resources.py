import csv
import json
import os
import re


def get_resource_usage(std_out: str):
    matches = re.findall("Running test (.*)\n.*ResourcesMapping\((.*)\)", std_out)
    tests_resources = [dict(json.loads(resources), ** {"test": test_name}) for test_name, resources in matches]
    return tests_resources

def write_resources_to_csv(tests_resources: list, output_file: str = "resources.csv"):
    # Get all keys and skip "test" key
    keys = {key for resources in tests_resources for key in resources.keys() if key != "test"}
    # Re-add "test" key, in order to be sure its first
    keys = ["test"] + sorted(keys)

    with open(output_file, "w") as f:
        writer = csv.DictWriter(f, fieldnames=keys)

        writer.writeheader()
        
        # Write data to CSV
        for row in tests_resources:
            writer.writerow(row)

def main():
    std_output = os.getenv("OUTPUT")
    test_resources = get_resource_usage(std_output)
    write_resources_to_csv(test_resources, output_file="resources.csv")


if __name__ == "__main__":
    main()
