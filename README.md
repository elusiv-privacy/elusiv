[![CI](https://github.com/elusivcash/elusiv/actions/workflows/test.yaml/badge.svg)](https://github.com/elusivcash/elusiv/actions/workflows/test.yaml) [![codecov](https://codecov.io/gh/elusivcash/elusiv/branch/master/graph/badge.svg?token=E6EBAGCE0M)](https://codecov.io/gh/elusivcash/elusiv)

# elusiv
### Testnet usage
#### First time
- start testnet with `npm run cluster` (and open new terminal tab)
- build program with `npm run build`
- deploy contract for first time with `npm run deploy` (generates new keypair)
- setup the storage accounts with `npm run setup`

#### Restart
- only start the cluster and ui

#### Redeploy changes
- redeploy program to same programID: `npm run redeploy`
- deploy new storage account: `npm run setup`
