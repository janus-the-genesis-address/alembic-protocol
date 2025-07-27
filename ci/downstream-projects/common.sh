#!/usr/bin/env bash
set -e

source ci/_
source ci/semver_bash/semver.sh
source scripts/patch-crates.sh
source scripts/read-cargo-variable.sh

Alembic_VER=$(readCargoVariable version Cargo.toml)
export Alembic_VER
export Alembic_DIR=$PWD
export CARGO="$Alembic_DIR"/cargo
export CARGO_BUILD_SBF="$Alembic_DIR"/cargo-build-sbf
export CARGO_TEST_SBF="$Alembic_DIR"/cargo-test-sbf

mkdir -p target/downstream-projects
cd target/downstream-projects
