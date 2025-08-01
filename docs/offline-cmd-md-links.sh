#!/usr/bin/env bash

CLI_USAGE_RELPATH="../cli/usage.md"

SED_OMIT_NONMATCHING=$'\nt\nd'
SED_CMD="s:^#### Alembic-(.*):* [\`\\1\`](${CLI_USAGE_RELPATH}#Alembic-\\1):${SED_OMIT_NONMATCHING}"

OFFLINE_CMDS=$(grep -E '#### Alembic-|--signer ' src/cli/usage.md | grep -B1 -- --signer | sed -Ee "$SED_CMD")

# Omit deprecated
grep -vE '\b(pay)\b' <<<"$OFFLINE_CMDS"
