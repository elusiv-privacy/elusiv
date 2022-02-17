import re

lines = open("optimization/cu-log.txt").read().splitlines()

compute_unit_lines = []
for line in lines:
    if line[0].isdigit(): compute_unit_lines.append(int(line))

if len(compute_unit_lines) % 2 != 0:
    del compute_unit_lines[-1]

sum = 0
diffs = []
for i in range(len(compute_unit_lines) // 2):
    diff = compute_unit_lines[i * 2] - compute_unit_lines[i * 2 + 1]
    diffs.append(diff)
    print(i, "c: ", diff)
    sum += diff

print("(CUs: Max:", max(diffs), "Avg:", sum // (len(compute_unit_lines) // 2), "Min:", min(diffs), ")")
