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
Scaling privacy with compliance in mind for the [Solana](https://github.com/solana-labs/solana) blockchain.

## Building
Use `cargo-build-bpf` [release version 1.9.28](https://github.com/solana-labs/solana/releases/tag/v1.9.28) with `bash bin/elusiv`.
The resulting dynamic library is located in _/dist_.

## Testing
- Integration tests: `bash bin/test integration`
- Unit tests: `bash bin/test unit`
- All tests: `bash bin/test`

## Program interaction
### Rust instruction API
In order to easily interact with the program from a Rust client, import Elusiv with the `instruction-abi` feature activated.
This will enable access to the functions `ElusivInstruction::do_this_instruction(..)`.

### Instruction serialization
When calling the instructions from other clients, serialize the `ElusivInstruction` enum using [Borsh](https://docs.rs/borsh/latest/borsh/).
The Elusiv interface also allows for arbitrary instruction data after the required fields.