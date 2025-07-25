#!/usr/bin/env bash

set -e
cd "$(dirname "$0")"
Alembic_ROOT="$(cd ../..; pwd)"

logDir="$PWD"/logs
rm -rf "$logDir"
mkdir "$logDir"

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

bootstrapInstall "edge"
Alembic-install-init --version
Alembic-install-init edge
Alembic-gossip --version
Alembic-dos --version

killall Alembic-gossip || true
Alembic-gossip spy --gossip-port 8001 > "$logDir"/gossip.log 2>&1 &
AlembicGossipPid=$!
echo "Alembic-gossip pid: $AlembicGossipPid"
sleep 5
Alembic-dos --mode gossip --data-type random --data-size 1232 &
dosPid=$!
echo "Alembic-dos pid: $dosPid"

pass=true

SECONDS=
while ((SECONDS < 600)); do
  if ! kill -0 $AlembicGossipPid; then
    echo "Alembic-gossip is no longer running after $SECONDS seconds"
    pass=false
    break
  fi
  if ! kill -0 $dosPid; then
    echo "Alembic-dos is no longer running after $SECONDS seconds"
    pass=false
    break
  fi
  sleep 1
done

kill $AlembicGossipPid || true
kill $dosPid || true
wait || true

$pass && echo Pass
