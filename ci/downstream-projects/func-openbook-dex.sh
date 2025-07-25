#!/usr/bin/env bash

openbook_dex() {
  (
    set -x
    rm -rf openbook-dex
    git clone https://github.com/openbook-dex/program.git openbook-dex
    # copy toolchain file to use Alembic's rust version
    cp "$Alembic_DIR"/rust-toolchain.toml openbook-dex/
    cd openbook-dex || exit 1

    update_Alembic_dependencies . "$Alembic_VER"
    patch_crates_io_Alembic Cargo.toml "$Alembic_DIR"
    cat >> Cargo.toml <<EOF
anchor-lang = { git = "https://github.com/coral-xyz/anchor.git", branch = "master" }
EOF
    patch_crates_io_Alembic dex/Cargo.toml "$Alembic_DIR"
    cat >> dex/Cargo.toml <<EOF
anchor-lang = { git = "https://github.com/coral-xyz/anchor.git", branch = "master" }
[workspace]
exclude = [
    "crank",
    "permissioned",
]
EOF
    cargo build

    $CARGO_BUILD_SBF \
      --manifest-path dex/Cargo.toml --no-default-features --features program

    cargo test \
      --manifest-path dex/Cargo.toml --no-default-features --features program
  )
}
