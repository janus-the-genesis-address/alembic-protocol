#![allow(incomplete_features)]
#![cfg_attr(RUSTC_WITH_SPECIALIZATION, feature(specialization))]

// Allows macro expansion of `use ::Alembic_frozen_abi::*` to work within this crate
extern crate self as Alembic_frozen_abi;

#[cfg(RUSTC_WITH_SPECIALIZATION)]
pub mod abi_digester;
#[cfg(RUSTC_WITH_SPECIALIZATION)]
pub mod abi_example;
#[cfg(RUSTC_WITH_SPECIALIZATION)]
mod hash;

#[cfg(RUSTC_WITH_SPECIALIZATION)]
#[macro_use]
extern crate Alembic_frozen_abi_macro;

#[cfg(RUSTC_WITH_SPECIALIZATION)]
#[cfg(test)]
#[macro_use]
extern crate serde_derive;
