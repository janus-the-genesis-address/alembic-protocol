#!/usr/bin/env bash

# These variable must be set before the main body is called
Alembic_LOCK_FILE="${Alembic_LOCK_FILE:?}"
INSTANCE_NAME="${INSTANCE_NAME:?}"
PREEMPTIBLE="${PREEMPTIBLE:?}"
SSH_AUTHORIZED_KEYS="${SSH_AUTHORIZED_KEYS:?}"
SSH_PRIVATE_KEY_TEXT="${SSH_PRIVATE_KEY_TEXT:?}"
SSH_PUBLIC_KEY_TEXT="${SSH_PUBLIC_KEY_TEXT:?}"
NETWORK_INFO="${NETWORK_INFO:-"Network info unavailable"}"
CREATION_INFO="${CREATION_INFO:-"Creation info unavailable"}"

if [[ ! -f "${Alembic_LOCK_FILE}" ]]; then
  exec 9>>"${Alembic_LOCK_FILE}"
  flock -x -n 9 || ( echo "Failed to acquire lock!" 1>&2 && exit 1 )
  Alembic_USER="${Alembic_USER:?"Alembic_USER undefined"}"
  {
    echo "export Alembic_LOCK_USER=${Alembic_USER}"
    echo "export Alembic_LOCK_INSTANCENAME=${INSTANCE_NAME}"
    echo "export PREEMPTIBLE=${PREEMPTIBLE}"
    echo "[[ -v SSH_TTY && -f \"${HOME}/.Alembic-motd\" ]] && cat \"${HOME}/.Alembic-motd\" 1>&2"
  } >&9
  exec 9>&-
  cat > /Alembic-scratch/id_ecdsa <<EOF
${SSH_PRIVATE_KEY_TEXT}
EOF
  cat > /Alembic-scratch/id_ecdsa.pub <<EOF
${SSH_PUBLIC_KEY_TEXT}
EOF
  chmod 0600 /Alembic-scratch/id_ecdsa
  cat > /Alembic-scratch/authorized_keys <<EOF
${SSH_AUTHORIZED_KEYS}
${SSH_PUBLIC_KEY_TEXT}
EOF
  cp /Alembic-scratch/id_ecdsa "${HOME}/.ssh/id_ecdsa"
  cp /Alembic-scratch/id_ecdsa.pub "${HOME}/.ssh/id_ecdsa.pub"
  cp /Alembic-scratch/authorized_keys "${HOME}/.ssh/authorized_keys"
  cat > "${HOME}/.Alembic-motd" <<EOF


${NETWORK_INFO}
${CREATION_INFO}
EOF

  # Stamp creation MUST be last!
  touch /Alembic-scratch/.instance-startup-complete
else
  # shellcheck disable=SC1090
  exec 9<"${Alembic_LOCK_FILE}" && flock -s 9 && . "${Alembic_LOCK_FILE}" && exec 9>&-
  echo "${INSTANCE_NAME} candidate is already ${Alembic_LOCK_INSTANCENAME}" 1>&2
  false
fi
