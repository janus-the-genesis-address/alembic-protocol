#!/usr/bin/env bash
#
# Basic empirical ABI system test - can validators on all supported versions of
# Alembic talk to each other?
#

set -e
cd "$(dirname "$0")"
Alembic_ROOT="$(cd ../..; pwd)"

logDir="$PWD"/logs
ledgerDir="$PWD"/config
rm -rf "$ledgerDir" "$logDir"
mkdir -p "$logDir"

baselineVersion=1.1.18  # <-- oldest version we remain compatible with
otherVersions=(
  beta
  edge
)

AlembicInstallDataDir=$PWD/releases
AlembicInstallGlobalOpts=(
  --data-dir "$AlembicInstallDataDir"
  --config "$AlembicInstallDataDir"/config.yml
  --no-modify-path
)

# Install all the Alembic versions
bootstrapInstall() {
  declare v=$1
  if [[ ! -h $AlembicInstallDataDir/active_release ]]; then
    sh "$Alembic_ROOT"/install/Alembic-install-init.sh "$v" "${AlembicInstallGlobalOpts[@]}"
  fi
  export PATH="$AlembicInstallDataDir/active_release/bin/:$PATH"
}

bootstrapInstall "$baselineVersion"
for v in "${otherVersions[@]}"; do
  Alembic-install-init "${AlembicInstallGlobalOpts[@]}" "$v"
  Alembic -V
done


ORIGINAL_PATH=$PATH
AlembicInstallUse() {
  declare version=$1
  echo "--- Now using Alembic $version"
  Alembic_BIN="$AlembicInstallDataDir/releases/$version/Alembic-release/bin"
  export PATH="$Alembic_BIN:$ORIGINAL_PATH"
}

killSession() {
  tmux kill-session -t abi || true
}

export RUST_BACKTRACE=1

# Start up the bootstrap validator using the baseline version
AlembicInstallUse "$baselineVersion"
echo "--- Starting $baselineVersion bootstrap validator"
trap 'killSession' INT TERM ERR EXIT
killSession
(
  set -x
  if [[ ! -x baseline-run.sh ]]; then
    curl https://raw.githubusercontent.com/Alembic-labs/Alembic/v"$baselineVersion"/run.sh -o baseline-run.sh
    chmod +x baseline-run.sh
  fi
  tmux new -s abi -d " \
    ./baseline-run.sh 2>&1 | tee $logDir/$baselineVersion.log \
  "

  SECONDS=
  while [[ ! -f config/baseline-run/init-completed ]]; do
    sleep 5
    if [[ $SECONDS -gt 60 ]]; then
      echo "Error: validator failed to start"
      exit 1
    fi
  done

  Alembic --url http://127.0.0.1:8899 show-validators
)

# Ensure all versions can see the bootstrap validator
for v in "${otherVersions[@]}"; do
  AlembicInstallUse "$v"
  echo "--- Looking for bootstrap validator on gossip"
  (
    set -x
    "$Alembic_BIN"/Alembic-gossip spy \
      --entrypoint 127.0.0.1:8001 \
      --num-nodes-exactly 1 \
      --timeout 30
  )
  echo Ok
done

# Start a validator for each version and look for it
#
# Once https://github.com/Alembic-labs/Alembic/issues/7738 is resolved, remove
# `--no-snapshot-fetch` when starting the validators
#
nodeCount=1
for v in "${otherVersions[@]}"; do
  nodeCount=$((nodeCount + 1))
  AlembicInstallUse "$v"
  # start another validator
  ledger="$ledgerDir"/ledger-"$v"
  rm -rf "$ledger"
  echo "--- Looking for $nodeCount validators on gossip"
  (
    set -x
    tmux new-window -t abi -n "$v" " \
      $Alembic_BIN/Alembic-validator \
      --ledger $ledger \
      --no-snapshot-fetch \
      --entrypoint 127.0.0.1:8001 \
      -o - 2>&1 | tee $logDir/$v.log \
    "
    "$Alembic_BIN"/Alembic-gossip spy \
      --entrypoint 127.0.0.1:8001 \
      --num-nodes-exactly $nodeCount \
      --timeout 30

    # Wait for it to make a snapshot root
    SECONDS=
    while [[ ! -d $ledger/snapshot ]]; do
      sleep 5
      if [[ $SECONDS -gt 60 ]]; then
        echo "Error: validator failed to create a snapshot"
        exit 1
      fi
    done
  )
  echo Ok
done

# Terminate all the validators
killSession

echo
echo Pass
exit 0
