IDLE_CUS = 10000
START_CUS = 70000 - IDLE_CUS

# Costs in CUs (BPF Compute Units)
adding_rounds_cus = [12673, 23173, 15199, 27102, 12907, 12661]
doubling_rounds_cus = [13078, 16767, 25817, 15379, 15070, 5567, 15070]

# Stub algorithm to sum up costs
# 0 CUs are used to skip rounds and keep the total rounds count as multiple of a certain factor
rounds_cus.append(mul_by_char_cus)
rounds_cus.extend(adding_rounds_cus)

# Calculate the optimal distribution
from optimize_distribution import find_optimal_distribution
find_optimal_distribution(rounds_cus, START_CUS, IDLE_CUS)