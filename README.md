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

## Testing
- Use `cargo-build-bpf` [_release version 1.9.28_](https://github.com/solana-labs/solana/releases/tag/v1.9.28)
- Integration tests: `bash bin/test integration`
- Unit tests: `bash bin/test unit`
- All tests: `bash bin/test`