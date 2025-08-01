#!/usr/bin/env bash

here=$(dirname "$0")
# shellcheck source=multinode-demo/common.sh
source "$here"/common.sh

set -e

rm -rf "$Alembic_CONFIG_DIR"/latest-testnet-snapshot
mkdir -p "$Alembic_CONFIG_DIR"/latest-testnet-snapshot
(
  cd "$Alembic_CONFIG_DIR"/latest-testnet-snapshot || exit 1
  set -x
  wget http://api.testnet.genesisaddress.ai/genesis.tar.bz2
  wget --trust-server-names http://testnet.genesisaddress.ai/snapshot.tar.bz2
)

snapshot=$(ls "$Alembic_CONFIG_DIR"/latest-testnet-snapshot/snapshot-[0-9]*-*.tar.zst)
if [[ -z $snapshot ]]; then
  echo Error: Unable to find latest snapshot
  exit 1
fi

if [[ ! $snapshot =~ snapshot-([0-9]*)-.*.tar.zst ]]; then
  echo Error: Unable to determine snapshot slot for "$snapshot"
  exit 1
fi

snapshot_slot="${BASH_REMATCH[1]}"

rm -rf "$Alembic_CONFIG_DIR"/bootstrap-validator
mkdir -p "$Alembic_CONFIG_DIR"/bootstrap-validator


# Create genesis ledger
if [[ -r $FAUCET_KEYPAIR ]]; then
  cp -f "$FAUCET_KEYPAIR" "$Alembic_CONFIG_DIR"/faucet.json
else
  $Alembic_keygen new --no-passphrase -fso "$Alembic_CONFIG_DIR"/faucet.json
fi

if [[ -f $BOOTSTRAP_VALIDATOR_IDENTITY_KEYPAIR ]]; then
  cp -f "$BOOTSTRAP_VALIDATOR_IDENTITY_KEYPAIR" "$Alembic_CONFIG_DIR"/bootstrap-validator/identity.json
else
  $Alembic_keygen new --no-passphrase -so "$Alembic_CONFIG_DIR"/bootstrap-validator/identity.json
fi

$Alembic_keygen new --no-passphrase -so "$Alembic_CONFIG_DIR"/bootstrap-validator/vote-account.json
$Alembic_keygen new --no-passphrase -so "$Alembic_CONFIG_DIR"/bootstrap-validator/stake-account.json

$Alembic_ledger_tool create-snapshot \
  --ledger "$Alembic_CONFIG_DIR"/latest-testnet-snapshot \
  --faucet-pubkey "$Alembic_CONFIG_DIR"/faucet.json \
  --faucet-lamports 500000000000000000 \
  --bootstrap-validator "$Alembic_CONFIG_DIR"/bootstrap-validator/identity.json \
                        "$Alembic_CONFIG_DIR"/bootstrap-validator/vote-account.json \
                        "$Alembic_CONFIG_DIR"/bootstrap-validator/stake-account.json \
  --hashes-per-tick sleep \
  "$snapshot_slot" "$Alembic_CONFIG_DIR"/bootstrap-validator

$Alembic_ledger_tool modify-genesis \
  --ledger "$Alembic_CONFIG_DIR"/latest-testnet-snapshot \
  --hashes-per-tick sleep \
  "$Alembic_CONFIG_DIR"/bootstrap-validator
