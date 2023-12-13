#%%
import re

with open("data.txt", "r") as f:
    data = f.read()

matches_failed = re.findall(r"thread '(.*)' panicked at ", data)
matches_failed = matches_failed[1:]

print(len(matches_failed))

print(matches_failed[:10])

matches_failed = [(m.split('::')[-2].replace("_minus_", "-"), m.split('::')[-1]) for m in matches_failed]
skip_dict = dict()
for [folder, file] in matches_failed:
    files = skip_dict.get(folder, [])
    files.append(file)
    skip_dict[folder] = files

skip = "filename:\n"
for folder in skip_dict:
    skip += f"  {folder}:\n"
    for file in skip_dict[folder]:
        skip += f"      - {file[5:]}\n"

with open("blockchain-tests-skip.yml", "w") as f:
    f.write(skip)
