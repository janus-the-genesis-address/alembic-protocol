#!/usr/bin/env bash

set -x
! tmux list-sessions || tmux kill-session
declare sudo=
if sudo true; then
  sudo="sudo -n"
fi

echo "pwd: $(pwd)"
for pid in Alembic/*.pid; do
  pgid=$(ps opgid= "$(cat "$pid")" | tr -d '[:space:]')
  if [[ -n $pgid ]]; then
    $sudo kill -- -"$pgid"
  fi
done
if [[ -f Alembic/netem.cfg ]]; then
  Alembic/scripts/netem.sh delete < Alembic/netem.cfg
  rm -f Alembic/netem.cfg
fi
Alembic/scripts/net-shaper.sh cleanup
for pattern in validator.sh boostrap-leader.sh Alembic- remote- iftop validator client node; do
  echo "killing $pattern"
  pkill -f $pattern
done
