#!/usr/bin/env bash
set -ex

cd "$(dirname "$0")"

# shellcheck source=net/scripts/Alembic-user-authorized_keys.sh
source Alembic-user-authorized_keys.sh

# Alembic-user-authorized_keys.sh defines the public keys for users that should
# automatically be granted access to ALL datacenter nodes.
for i in "${!Alembic_USERS[@]}"; do
  echo "environment=\"Alembic_USER=${Alembic_USERS[i]}\" ${Alembic_PUBKEYS[i]}"
done

