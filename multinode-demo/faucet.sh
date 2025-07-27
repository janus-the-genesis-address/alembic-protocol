#!/usr/bin/env bash
#
# Starts an instance of Alembic-faucet
#
here=$(dirname "$0")

# shellcheck source=multinode-demo/common.sh
source "$here"/common.sh

[[ -f "$Alembic_CONFIG_DIR"/faucet.json ]] || {
  echo "$Alembic_CONFIG_DIR/faucet.json not found, create it by running:"
  echo
  echo "  ${here}/setup.sh"
  exit 1
}

set -x
# shellcheck disable=SC2086 # Don't want to double quote $Alembic_faucet
exec $Alembic_faucet --keypair "$Alembic_CONFIG_DIR"/faucet.json "$@"
