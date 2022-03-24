IDLE_CUS = 10000
START_CUS = 70000 - IDLE_CUS

# Costs in CUs (BPF Compute Units)
mul_cus = 34000
save_cus = 36000
inputs_count = 4
mul_rounds = 256

rounds_cus = []

# Stub algorithm to sum up costs
for i in range(inputs_count):
    for round in range(mul_rounds): rounds_cus.append(mul_cus)
    rounds_cus.append(save_cus)

# Calculate the optimal distribution
from optimize_distribution import find_optimal_distribution
find_optimal_distribution(rounds_cus, START_CUS, IDLE_CUS)