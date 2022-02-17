import re

lines = open("optimization/cu-log.txt").read().splitlines()

compute_unit_lines = []
for line in lines:
    compute_unit_lines.append(int(line))

if len(compute_unit_lines) % 2 != 0:
    del compute_unit_lines[-1]

sum = 0
diffs = []
for i in range(len(compute_unit_lines) // 2):
    diff = compute_unit_lines[i * 2] - compute_unit_lines[i * 2 + 1]
    diffs.append(diff)
    print(i, "c: ", diff)
    sum += diff

print("Average:", sum // (len(compute_unit_lines) // 2))
print("Min:", min(diffs))
print("Max:", max(diffs))
