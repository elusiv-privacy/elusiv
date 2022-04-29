IDLE_CUS = 20000
START_CUS = 0

# ATE loop count reversed, first element removed and -1s squared
ate_rev_normalized = [1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0]

# Costs in CUs (BPF Compute Units)
adding_rounds_cus = [90000]
doubling_rounds_cus = [70000]
ell_rounds_cus = [11677, 92056, 10550, 92091, 10147, 91988]
square_in_place_cus = 91923
mul_by_char_cus = 18000

rounds_cus = list()

# Stub algorithm to sum up costs
# 0 CUs are used to skip rounds and keep the total rounds count as multiple of a certain factor
for i, complex_round in enumerate(ate_rev_normalized):
    if i > 0:
        rounds_cus.append(square_in_place_cus)
    else:
        rounds_cus.append(0)

    rounds_cus.extend(doubling_rounds_cus)
    rounds_cus.extend(ell_rounds_cus)

    if complex_round == 1:
        rounds_cus.extend(adding_rounds_cus)
        rounds_cus.extend(ell_rounds_cus)
    else:
        rounds_cus.extend([0] * len(adding_rounds_cus))
        rounds_cus.extend([0] * len(ell_rounds_cus))

rounds_main_loop = len(rounds_cus)

rounds_cus.append(mul_by_char_cus)
rounds_cus.extend(adding_rounds_cus)
rounds_cus.extend(ell_rounds_cus)
rounds_cus.append(mul_by_char_cus)
rounds_cus.extend(adding_rounds_cus)
rounds_cus.extend(ell_rounds_cus)

# Calculate the optimal distribution
from optimize_distribution import find_optimal_distribution
find_optimal_distribution(rounds_cus, START_CUS, IDLE_CUS)
print("Main loop rounds: ", rounds_main_loop)