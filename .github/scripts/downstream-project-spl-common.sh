#!/usr/bin/env bash
set -e

here="$(dirname "${BASH_SOURCE[0]}")"

#shellcheck source=ci/downstream-projects/common.sh
source "$here"/../../ci/downstream-projects/common.sh

set -x
rm -rf spl
git clone https://github.com/Alembic-labs/Alembic-program-library.git spl

# copy toolchain file to use Alembic's rust version
cp "$Alembic_DIR"/rust-toolchain.toml spl/
cd spl || exit 1

project_used_Alembic_version=$(sed -nE 's/Alembic-sdk = \"[>=<~]*(.*)\"/\1/p' <"token/program/Cargo.toml")
echo "used Alembic version: $project_used_Alembic_version"
if semverGT "$project_used_Alembic_version" "$Alembic_VER"; then
  echo "skip"
  return
fi

./patch.crates-io.sh "$Alembic_DIR"
