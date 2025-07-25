#!/usr/bin/env bash
set -ex

[[ $(uname) = Linux ]] || exit 1
[[ $USER = root ]] || exit 1

[[ -d /home/Alembic/.ssh ]] || exit 1

if [[ ${#Alembic_PUBKEYS[@]} -eq 0 ]]; then
  echo "Warning: source Alembic-user-authorized_keys.sh first"
fi

# Alembic-user-authorized_keys.sh defines the public keys for users that should
# automatically be granted access to ALL testnets
for key in "${Alembic_PUBKEYS[@]}"; do
  echo "$key" >> /Alembic-scratch/authorized_keys
done

sudo -u Alembic bash -c "
  cat /Alembic-scratch/authorized_keys >> /home/Alembic/.ssh/authorized_keys
"
