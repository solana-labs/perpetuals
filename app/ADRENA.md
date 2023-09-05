# CLI setup

## Keys

admin: 5vAooJKJxWXVPNb13dBq1jPsuE3RTbMCfYuounMJcAvb

alice: ArJJRUcbSrqqMuTrMdQNvBsXNokQBh3VCt2EDrg2aqnc
martin: 7KQ7YgFmP4mz4p1UQnwrESZ58APxDZUoWyN5GXeXm4cz
paul: 37dThGRY77tkRrnzgbfAkZaNnCQzGdQWnLCE9UD4MuPy

usdc: 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC
eth: 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr
btc: HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM

program: CfbwNZaAL4izRqLsnxixx76uQy9GE6PBy917i57jVbia

governance program: GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw

governance realm: 65M6EkpcQ5bXfJBhkmgNT3gUTB2YtW5tmBbsDEP6Gfcj

pool name: main-pool
pool: B9HJCFyrzVStCNHnAkXRMG1wDk8XS9eyKuitmvxjn2Hf
lp token mint: EqkG2sZrCw3dApo4LmeHvhAAEo7MwwjZNU2wxK5D6gbb

Orex local wallet: CqJVUVbxJae8GfYsSooA5qzjHmoZusB1Hni7Ed1eEDeH

## Upload program

./scripts/change_program_id.sh
anchor deploy --program-name perpetuals --provider.cluster devnet --program-keypair ./target/deploy/perpetuals-keypair.json

### Give program authority to admin (to be able to init)

```
solana program set-upgrade-authority <PROGRAM_ADDRESS> --new-upgrade-authority <NEW_UPGRADE_AUTHORITY>
```

i.e

```
solana program set-upgrade-authority CfbwNZaAL4izRqLsnxixx76uQy9GE6PBy917i57jVbia --new-upgrade-authority 5vAooJKJxWXVPNb13dBq1jPsuE3RTbMCfYuounMJcAvb
```

### Give program authority back to local wallet to redeploy

```
solana program set-upgrade-authority <PROGRAM_ADDRESS> --new-upgrade-authority <NEW_UPGRADE_AUTHORITY> -k <CURRENT_AUTHORITY_KEYPAIR>
```

i.e

```
solana program set-upgrade-authority CfbwNZaAL4izRqLsnxixx76uQy9GE6PBy917i57jVbia --new-upgrade-authority CqJVUVbxJae8GfYsSooA5qzjHmoZusB1Hni7Ed1eEDeH -k 5vAooJKJxWXVPNb13dBq1jPsuE3RTbMCfYuounMJcAvb
```

## Get governance realm key

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> get-governance-realm-key --name <REALM_NAME>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json get-governance-realm-key --name AdrenaTest
```

## Setup the Cortex

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> init \
 --min-signatures 1 \
 --lm-staking-reward-token-mint <STAKING_REWARD_TOKEN_MINT> \
 --governance-realm <GOVERNANCE_REALM> \
 --core-contributor-bucket-allocation <ALLOCATION> \
 --dao-treasury-bucket-allocation <ALLOCATION> \
 --pol-bucket-allocation <ALLOCATION> \
 --ecosystem-bucket-allocation <ALLOCATION> \
 <ADMIN_PUBKEY1>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json init \
 --min-signatures 1 \
 --lm-staking-reward-token-mint 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC \
 --governance-realm 65M6EkpcQ5bXfJBhkmgNT3gUTB2YtW5tmBbsDEP6Gfcj \
 --core-contributor-bucket-allocation 100000 \
 --dao-treasury-bucket-allocation 100000 \
 --pol-bucket-allocation 100000 \
 --ecosystem-bucket-allocation 100000 \
 5vAooJKJxWXVPNb13dBq1jPsuE3RTbMCfYuounMJcAvb
```

## Create the governance realm

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> create-governance-realm \
--name <REALM_NAME> \
--min-community-weight-to-create-governance <WEIGHT>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json create-governance-realm \
--name AdrenaTest \
--min-community-weight-to-create-governance 10000
```

You may access the new realm here:

```
https://app.realms.today/dao/<REALM_KEY>?cluster=devnet
```

i.e

```
https://app.realms.today/dao/65M6EkpcQ5bXfJBhkmgNT3gUTB2YtW5tmBbsDEP6Gfcj?cluster=devnet
```

## Get Multisig / Perpetuals

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> get-multisig
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> get-perpetuals
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json get-multisig
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json get-perpetuals
```

## Setup the Pool

### Add the pool

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> add-pool <POOL_NAME>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-pool main-pool
```

### Get pool info

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> get-pool <POOL_NAME>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json get-pool main-pool
```

### Get LP token mint

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> get-lp-token-mint <POOL_NAME>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json get-lp-token-mint main-pool
```

### Init LP Staking

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> init-lp-staking <POOL_NAME> --staking-reward-token-mint <STAKING_REWARD_TOKEN_MINT>
```

i.e

```
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json init-lp-staking main-pool --staking-reward-token-mint 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC
```

## Add Custodies with Pyth oracle

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> add-custody <POOL_NAME> -t pyth [--stablecoin] <TOKEN_MINT> <TOKEN_ORACLE_ACCOUNT>
```

i.e

```
// USDC
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-custody main-pool --oracletype pyth --stablecoin 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC 5SSkXsEKQepHHAewytPVwdej4epN1nxgLVM84L4KXgy7

// ETH
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-custody main-pool --oracletype pyth 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw

// BTC
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-custody main-pool --oracletype pyth HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J

// SOL
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-custody main-pool --oracletype pyth So11111111111111111111111111111111111111112 J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix
```

## Add Liquidity to custodies

```
npx ts-node app/src/cli.ts -k <ADMIN_KEYPAIR> add-liquidity <POOL_NAME> <TOKEN_MINT> --amount-in <AMOUNT_IN> --min-amount-out <AMOUNT_OUT>
```

i.e

```
// Add USDC liquidity
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity main-pool 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC --amount-in 10000000 --min-amount-out 0

// Add ETH liquidity
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity main-pool 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr --amount-in 5000 --min-amount-out 0

// Add BTC liquidity
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity main-pool HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM --amount-in 250 --min-amount-out 0

// Add SOL liquidity
npx ts-node app/src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity main-pool So11111111111111111111111111111111111111112 --amount-in 500000000 --min-amount-out 0
```

## Get custodies

```
npx ts-node src/cli.ts -k <ADMIN_KEYPAIR> get-custodies <POOL_NAME>
```

i.e

```
npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json get-custodies main-pool
```
