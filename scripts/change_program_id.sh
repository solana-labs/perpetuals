#!/bin/sh
CURRENT_PUBKEY=`solana-keygen pubkey ./target/deploy/perpetuals-keypair.json`

solana-keygen new -o ./target/deploy/perpetuals-keypair.json --force --no-bip39-passphrase

NEW_PUBKEY=`solana-keygen pubkey ./target/deploy/perpetuals-keypair.json`

# Replace
sed -i.bak "s/$CURRENT_PUBKEY/$NEW_PUBKEY/g" ./Anchor.toml
sed -i.bak "s/$CURRENT_PUBKEY/$NEW_PUBKEY/g" ./programs/perpetuals/src/lib.rs
sed -i.bak "s/$CURRENT_PUBKEY/$NEW_PUBKEY/g" ./target/idl/perpetuals.json