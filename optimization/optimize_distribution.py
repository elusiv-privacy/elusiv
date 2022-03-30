MAX_CUS = 1000000
SECURITY_PADDING = 2000

# Calculate the optimal distribution
def find_optimal_distribution(rounds_cus, start_cus, idle_cus):
    max = MAX_CUS - SECURITY_PADDING - idle_cus
    rounds = 0
    iterations = list()
    iteration_rounds = 0
    iteration_cus = start_cus

    while rounds < len(rounds_cus):
        next_cost = rounds_cus[rounds]

        if iteration_cus + next_cost <= max:
            rounds += 1
            iteration_rounds += 1
            iteration_cus += next_cost
        else:
            iterations.append(iteration_rounds)
            iteration_rounds = 0
            iteration_cus = 0
    iterations.append(iteration_rounds)

    # Output
    print("Iterations: ", iterations)
    print("Count: ", len(iterations))
    print("Total rounds: ", len(rounds_cus))
    print("Remaining CUs: ", MAX_CUS - iteration_cus)