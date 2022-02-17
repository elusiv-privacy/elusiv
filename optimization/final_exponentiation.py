from enum import Enum

IDLE_CUS = 34000
START_CUS = 0

# Costs in CUs (BPF Compute Units)
class Arm(Enum):
    One = 0,
    Inverse = 1,
    Mul = 2,
    Frobenius = 3,
    CyclotomicSquare = 4,
    ExpByNegX = 5, 

conjugate_swap_cus = 10000
inverse_cus = [30000, 32000, 11000, 22000, 21000, 3600, 65000, 25000, 85000]
mul_cus = [20400, 25000, 20400, 25000, 46000]
frobenius_cus = [18000, 18000, 18000]
cyclotomic_square_cus = 50000
cyclotomic_exp_round = [1, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, -1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, -1, 0, -1, 0, -1, 0, 1, 0, 1, 0, 0, -1, 0, 1, 0, 1, 0, -1, 0, 0, 1, 0, 1, 0, 0, 0, 1]
cyclotomic_exp_round.reverse()
exp_cus = [1300]
for i, cus in enumerate(cyclotomic_exp_round):
    if i > 0: exp_cus.append(cyclotomic_square_cus)
    else: exp_cus.append(0)

    if cus != 0: exp_cus.extend(mul_cus)
    else: exp_cus.extend([0, 0, 0, 0, 0])
exp_cus.append(1000)

arms = [
    Arm.One,
    Arm.Inverse,
    Arm.One,
    Arm.Mul,
    Arm.One,
    Arm.Frobenius,
    Arm.Mul,
    Arm.One,
    Arm.ExpByNegX,
    Arm.CyclotomicSquare,
    Arm.CyclotomicSquare,
    Arm.Mul,
    Arm.One,
    Arm.ExpByNegX,
    Arm.CyclotomicSquare,
    Arm.ExpByNegX,
    Arm.One,
    Arm.Mul,
    Arm.One,
    Arm.Mul,
    Arm.One,
    Arm.Mul,
    Arm.One,
    Arm.Mul,
    Arm.One,
    Arm.Mul,
    Arm.One,
    Arm.Frobenius,
    Arm.Mul,
    Arm.One,
    Arm.Frobenius,
    Arm.Mul,
    Arm.One,
    Arm.Mul,
    Arm.Frobenius,
    Arm.One,
    Arm.Mul,
    Arm.One,
]

rounds_cus = list()

# Stub algorithm to sum up costs
for arm in arms:
    match arm:
        case Arm.One:
            rounds_cus.append(conjugate_swap_cus)
        case Arm.Inverse:
            rounds_cus.extend(inverse_cus)
        case Arm.Mul:
            rounds_cus.extend(mul_cus)
        case Arm.Frobenius:
            rounds_cus.extend(frobenius_cus)
        case Arm.CyclotomicSquare:
            rounds_cus.append(cyclotomic_square_cus)
        case Arm.ExpByNegX:
            rounds_cus.extend(exp_cus)

# Calculate the optimal distribution
from optimize_distribution import find_optimal_distribution
find_optimal_distribution(rounds_cus, START_CUS, IDLE_CUS)