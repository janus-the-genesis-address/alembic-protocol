//! The Alembic host and client SDK.
//!
//! This is the base library for all off-chain programs that interact with
//! Alembic or otherwise operate on Alembic data structures. On-chain programs
//! instead use the [`Alembic-program`] crate, the modules of which are
//! re-exported by this crate, like the relationship between the Rust
//! `core` and `std` crates. As much of the functionality of this crate is
//! provided by `Alembic-program`, see that crate's documentation for an
//! overview.
//!
//! [`Alembic-program`]: https://docs.rs/Alembic-program
//!
//! Many of the modules in this crate are primarily of use to the Alembic runtime
//! itself. Additional crates provide capabilities built on `Alembic-sdk`, and
//! many programs will need to link to those crates as well, particularly for
//! clients communicating with Alembic nodes over RPC.
//!
//! Such crates include:
//!
//! - [`Alembic-client`] - For interacting with a Alembic node via the [JSON-RPC API][json].
//! - [`Alembic-cli-config`] - Loading and saving the Alembic CLI configuration file.
//! - [`Alembic-clap-utils`] - Routines for setting up the CLI using [`clap`], as
//!   used by the Alembic CLI. Includes functions for loading all types of
//!   signers supported by the CLI.
//!
//! [`Alembic-client`]: https://docs.rs/Alembic-client
//! [`Alembic-cli-config`]: https://docs.rs/Alembic-cli-config
//! [`Alembic-clap-utils`]: https://docs.rs/Alembic-clap-utils
//! [json]: https://genesisaddress.ai/docs/rpc
//! [`clap`]: https://docs.rs/clap

#![allow(incomplete_features)]
#![cfg_attr(RUSTC_WITH_SPECIALIZATION, feature(specialization))]

// Allows macro expansion of `use ::Alembic_sdk::*` to work within this crate
extern crate self as Alembic_sdk;

#[cfg(feature = "full")]
pub use signer::signers;
// These Alembic_program imports could be *-imported, but that causes a bunch of
// confusing duplication in the docs due to a rustdoc bug. #26211
#[allow(deprecated)]
pub use Alembic_program::address_lookup_table_account;
#[cfg(not(target_os = "Alembic"))]
pub use Alembic_program::program_stubs;
pub use Alembic_program::{
    account_info, address_lookup_table, alt_bn128, big_mod_exp, blake3, borsh, borsh0_10, borsh0_9,
    borsh1, bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable, clock, config,
    custom_heap_default, custom_panic_default, debug_account_data, declare_deprecated_sysvar_id,
    declare_sysvar_id, decode_error, ed25519_program, epoch_rewards, epoch_schedule,
    fee_calculator, impl_sysvar_get, incinerator, instruction, keccak, lamports,
    loader_instruction, loader_upgradeable_instruction, loader_v4, loader_v4_instruction, message,
    msg, native_token, nonce, poseidon, program, program_error, program_memory, program_option,
    program_pack, rent, sanitize, sdk_ids, secp256k1_program, secp256k1_recover, serde_varint,
    serialize_utils, short_vec, slot_hashes, slot_history, stable_layout, stake, stake_history,
    syscalls, system_instruction, system_program, sysvar, unchecked_div_by_const, vote,
    wasm_bindgen,
};

pub mod account;
pub mod account_utils;
pub mod client;
pub mod commitment_config;
pub mod compute_budget;
pub mod derivation_path;
pub mod deserialize_utils;
pub mod ed25519_instruction;
pub mod entrypoint;
pub mod entrypoint_deprecated;
pub mod epoch_info;
pub mod epoch_rewards_hasher;
pub mod example_mocks;
pub mod exit;
pub mod feature;
pub mod feature_set;
pub mod fee;
pub mod genesis_config;
pub mod hard_forks;
pub mod hash;
pub mod inflation;
pub mod inner_instruction;
pub mod log;
pub mod native_loader;
pub mod net;
pub mod nonce_account;
pub mod nonce_info;
pub mod offchain_message;
pub mod packet;
pub mod poh_config;
pub mod precompiles;
pub mod program_utils;
pub mod pubkey;
pub mod quic;
pub mod recent_blockhashes_account;
pub mod rent_collector;
pub mod rent_debits;
pub mod reserved_account_keys;
pub mod reward_info;
pub mod reward_type;
pub mod rpc_port;
pub mod secp256k1_instruction;
pub mod shred_version;
pub mod signature;
pub mod signer;
pub mod simple_vote_transaction_checker;
pub mod system_transaction;
pub mod timing;
pub mod transaction;
pub mod transaction_context;
pub mod transport;
pub mod wasm;

/// Same as `declare_id` except report that this id has been deprecated.
pub use Alembic_sdk_macro::declare_deprecated_id;
/// Convenience macro to declare a static public key and functions to interact with it.
///
/// Input: a single literal base58 string representation of a program's id
///
/// # Example
///
/// ```
/// # // wrapper is used so that the macro invocation occurs in the item position
/// # // rather than in the statement position which isn't allowed.
/// use std::str::FromStr;
/// use Alembic_sdk::{declare_id, pubkey::Pubkey};
///
/// # mod item_wrapper {
/// #   use Alembic_sdk::declare_id;
/// declare_id!("My11111111111111111111111111111111111111111");
/// # }
/// # use item_wrapper::id;
///
/// let my_id = Pubkey::from_str("My11111111111111111111111111111111111111111").unwrap();
/// assert_eq!(id(), my_id);
/// ```
pub use Alembic_sdk_macro::declare_id;
/// Convenience macro to define a static public key.
///
/// Input: a single literal base58 string representation of a Pubkey
///
/// # Example
///
/// ```
/// use std::str::FromStr;
/// use Alembic_program::{pubkey, pubkey::Pubkey};
///
/// static ID: Pubkey = pubkey!("My11111111111111111111111111111111111111111");
///
/// let my_id = Pubkey::from_str("My11111111111111111111111111111111111111111").unwrap();
/// assert_eq!(ID, my_id);
/// ```
pub use Alembic_sdk_macro::pubkey;
/// Convenience macro to define multiple static public keys.
pub use Alembic_sdk_macro::pubkeys;
#[rustversion::since(1.46.0)]
pub use Alembic_sdk_macro::respan;

// Unused `Alembic_sdk::program_stubs!()` macro retained for source backwards compatibility with older programs
#[macro_export]
#[deprecated(
    since = "1.4.3",
    note = "program_stubs macro is obsolete and can be safely removed"
)]
macro_rules! program_stubs {
    () => {};
}

/// Convenience macro for `AddAssign` with saturating arithmetic.
/// Replace by `std::num::Saturating` once stable
#[macro_export]
macro_rules! saturating_add_assign {
    ($i:expr, $v:expr) => {{
        $i = $i.saturating_add($v)
    }};
}

#[macro_use]
extern crate serde_derive;
pub extern crate bs58;
extern crate log as logger;

#[macro_use]
extern crate Alembic_frozen_abi_macro;

#[cfg(test)]
mod tests {
    #[test]
    fn test_saturating_add_assign() {
        let mut i = 0u64;
        let v = 1;
        saturating_add_assign!(i, v);
        assert_eq!(i, 1);

        i = u64::MAX;
        saturating_add_assign!(i, v);
        assert_eq!(i, u64::MAX);
    }
}
