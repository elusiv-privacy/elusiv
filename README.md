[![CI](https://github.com/elusivcash/elusiv/actions/workflows/test.yaml/badge.svg)](https://github.com/elusivcash/elusiv/actions/workflows/test.yaml) [![codecov](https://codecov.io/gh/elusivcash/elusiv/branch/master/graph/badge.svg?token=E6EBAGCE0M)](https://codecov.io/gh/elusivcash/elusiv) [![Security audit](https://github.com/elusivcash/elusiv/actions/workflows/audit.yaml/badge.svg)](https://github.com/elusivcash/elusiv/actions/workflows/audit.yaml)

# elusiv
### Devnet usage
#### First time
- build program with `npm run build`
- deploy contract for first time with `npm run deploy` (generates new keypair)

#### Redeploy changes
- redeploy program to same programID: `npm run redeploy`
- deploy new storage account: `npm run setup`
