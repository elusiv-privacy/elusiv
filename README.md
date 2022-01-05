# elusiv
### Testnet usage
#### First time
- clone repository
- start testnet with `npm run cluster` (and open new terminal tab)
- build program with `npm run build`
- deploy contract for first time with `npm run deploy` (generates new keypair)
- setup the storage accounts with `npm run setup`

#### Restart
- only start the cluster and ui

#### Redeploy changes
- redeploy program to same programID: `npm run redeploy`
- deploy new storage account: `npm run setup`
