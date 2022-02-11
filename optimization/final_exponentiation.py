IDLE_CUS = 30000

# Costs in CUs (BPF Compute Units)
conjugate_swap_cus = 1000
inverse_cus = [28000, 30000, 11000, 22000, 21000, 3500, 80000, 25000]
mul_cus = [20000, 20000, 20000, 20000, 20000]
frobenius_cus = [18000, 18000, 18000]
cyclotomic_square_cus = 50000
cyclotomic_exp_round = [1, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, -1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, -1, 0, -1, 0, -1, 0, 1, 0, 1, 0, 0, -1, 0, 1, 0, 1, 0, -1, 0, 0, 1, 0, 1, 0, 0, 0, 1]
exp_cus = [11000]
for i, cus in enumerate(cyclotomic_exp_round):
    if i > 0: exp_cus.append(cyclotomic_square_cus)
    if cus != 0: exp_cus.extend(mul_cus)
exp_cus.append(11000)

rounds_cus = list()

# Stub algorithm to sum up costs
# 1
rounds_cus.append(conjugate_swap_cus)
# inverse
rounds_cus.extend(inverse_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# frobenius
rounds_cus.extend(frobenius_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# exp
rounds_cus.extend(exp_cus)
# cyclotomic
rounds_cus.append(cyclotomic_square_cus)
# cyclotomic
rounds_cus.append(cyclotomic_square_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# exp
rounds_cus.extend(exp_cus)
# cyclotomic
rounds_cus.append(cyclotomic_square_cus)
# exp
rounds_cus.extend(exp_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# frobenius
rounds_cus.extend(frobenius_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# frobenius
rounds_cus.extend(frobenius_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# frobenius
rounds_cus.extend(frobenius_cus)
# 1
rounds_cus.append(conjugate_swap_cus)
# mul
rounds_cus.extend(mul_cus)
# 1
rounds_cus.append(conjugate_swap_cus)

# Calculate the optimal distribution
from optimize_distribution import find_optimal_distribution
find_optimal_distribution(rounds_cus, IDLE_CUS)