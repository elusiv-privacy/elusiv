<div align="center">
    <img src="https://github.com/elusiv-privacy/.github/blob/main/profile/elusiv-banner.png" width="100%"/>
</div>

<br/>

<div align="center">

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![CI](https://github.com/elusiv-privacy/elusiv/actions/workflows/test.yaml/badge.svg)](https://github.com/elusiv-privacy/elusiv/actions/workflows/test.yaml)
[![Codecov](https://codecov.io/gh/elusiv-privacy/elusiv/branch/master/graph/badge.svg?token=E6EBAGCE0M)](https://codecov.io/gh/elusiv-privacy/elusiv)
[![dependency check](https://github.com/elusiv-privacy/elusiv/actions/workflows/audit.yaml/badge.svg)](https://github.com/elusiv-privacy/elusiv/actions/workflows/audit.yaml)
[![vkey check](https://github.com/elusiv-privacy/elusiv/actions/workflows/vkey.yaml/badge.svg)](https://github.com/elusiv-privacy/elusiv/actions/workflows/vkey.yaml)

</div>

# Elusiv
Scaling privacy with Zero-knowledge proofs and MPC on the [Solana](https://github.com/solana-labs/solana) blockchain.

## Programs
This repository contains a collection of on-chain programs:

- the [Elusiv](./elusiv/) on-chain program,
- the [Elusiv-Warden-Network](./elusiv-warden-network/) on-chain program.

The addresses for the on-chain programs are located in (and linked at compilation from) [Id.toml](./Id.toml).

## Supported tokens
All tokens (SOL and SPL-tokens) supported by the Elusiv on-chain programs are located in (and linked at compilation from) [Token.toml](./Token.toml).
On-chain price data is provided through the [Pyth oracle network](https://pyth.network/).

## Development
Please ensure that you have [Rust](https://www.rust-lang.org/tools/install) and the [Solana tool suite](https://docs.solana.com/cli/install-solana-cli-tools) installed on your local machine.

### Building
The `build` crate allows for accessible building and testing of any program in this repository.
Build a program library with:

```
$ sh build.sh build --cluster <mainnet|devnet> --target <program-name>
```

### Testing
This library comes with comprehensive unit and integration tests for each of the provided crates.
Execute the tests with:

```
$ sh build.sh test --test-kind <unit|integration|...> --target <program-name>
```

### Using Docker
Testing can be performed in a Docker container using `./docker_test.sh`. Running this will result in the creation of an `elusiv-dev` Docker image as well as a few cache volumes. 

### Program interaction
In order to easily interact with the program from a Rust client, import a program library with the `elusiv-client` feature enabled.
This results in access to all instruction-generation functions located in each program-crate's `instructions` module.

When constructing instructions from other clients, serialize the instructions using [Borsh](https://borsh.io/).

## Contribution
We welcome contributions and pull requests.
Please check our [contribution rules](https://github.com/elusiv-privacy/elusiv/blob/master/CONTRIBUTING.md) and [code of conduct](./CODE_OF_CONDUCT.md).

## Security
To ensure that Elusiv is secure we did the following among other things:

- independent security audit of the Elusiv on-chain program with [OtterSec](./resources/OtterSec-09-22.pdf),
- independent security audit of the associated Zero-knowledge-proof circuits with [ABDK Consulting](https://github.com/elusiv-privacy/circuits/tree/master/audits),
- running the [Sec3](https://www.sec3.dev/) X-Ray tool after any changes to the on-chain code,
- open sourced the codebase together with our [security policy](./SECURITY.md).

Our goal is to make the Elusiv on-chain programs non-upgradeable as soon as possible.

## License
This project is licensed under the terms of the [GNU General Public License v3.0](./LICENSE).

## Disclaimer
This project is provided "AS IS", WITHOUT WARRANTIES OF ANY KIND, either express or implied.
This project provides complex software that utilizes an advanced and experimental smart contract runtime.

We do not guarantee that the code in this project is error-free, complete, or up-to-date.
Even with all measures taken to ensure its reliability, mistakes can still occur.
We are not liable for any damages or losses that may result from your use of this project.
Please use this project at your own risk.

We reserve the right to modify this disclaimer at any time.