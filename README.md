<div align="center">
    <img src="https://github.com/elusiv-privacy/.github/blob/main/profile/elusiv.svg" width="150"/>
</div>

<br/>

<div align="center">

[![CI](https://github.com/elusiv-privacy/elusiv/actions/workflows/test.yaml/badge.svg)](https://github.com/elusiv-privacy/elusiv/actions/workflows/test.yaml)
[![Codecov](https://codecov.io/gh/elusiv-privacy/elusiv/branch/master/graph/badge.svg?token=E6EBAGCE0M)](https://codecov.io/gh/elusiv-privacy/elusiv)
[![Dependency check](https://github.com/elusiv-privacy/elusiv/actions/workflows/audit.yaml/badge.svg)](https://github.com/elusiv-privacy/elusiv/actions/workflows/audit.yaml)

</div>

# Elusiv
Scaling privacy with compliance for the [Solana](https://github.com/solana-labs/solana) blockchain.

## Building and testing
`sh build.sh <build|(test (--unit|--integration|--tarpaulin))> <elusiv|elusiv-warden-network>`

## Supported tokens
All supported tokens can be found in [Token.toml](https://github.com/elusiv-privacy/elusiv/blob/master/elusiv/Token.toml).
On-chain price data is provided by [Pyth](https://pyth.network/).

## Program interaction
### Rust instruction API
In order to easily interact with the program from a Rust client, import Elusiv with the `elusiv-client` feature enabled.
This results in access to the functions `ElusivInstruction::do_this_instruction(..)`.

### Instruction serialization
When calling the instructions from other clients, serialize the `ElusivInstruction` enum using [Borsh](https://docs.rs/borsh/latest/borsh/).
The Elusiv interface also allows for arbitrary instruction data succeeding the required fields.