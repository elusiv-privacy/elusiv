# Elusiv Interpreter
In order to handle Solana's compute budget we split comutation power intensive computations into (partial) sub-computations.
This means a computation is represented by states s_i ∈ [n]_0 with s_0 being the initial state and s_n the result.
For easier und more secure implementation of these partial computations, we have our own interpreter, implemented using proc macros.

## Usage

## How it works