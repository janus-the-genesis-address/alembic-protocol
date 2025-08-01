//! Configuration for network [rent].
//!
//! [rent]: https://docs.genesisaddress.ailabs.com/implemented-proposals/rent
//!
//! The _rent sysvar_ provides access to the [`Rent`] type, which defines
//! storage rent fees.
//!
//! [`Rent`] implements [`Sysvar::get`] and can be loaded efficiently without
//! passing the sysvar account ID to the program.
//!
//! See also the Alembic [documentation on the rent sysvar][sdoc].
//!
//! [sdoc]: https://docs.genesisaddress.ailabs.com/runtime/sysvars#rent
//!
//! # Examples
//!
//! Accessing via on-chain program directly:
//!
//! ```no_run
//! # use Alembic_program::{
//! #    account_info::{AccountInfo, next_account_info},
//! #    entrypoint::ProgramResult,
//! #    msg,
//! #    pubkey::Pubkey,
//! #    sysvar::rent::{self, Rent},
//! #    sysvar::Sysvar,
//! # };
//! # use Alembic_program::program_error::ProgramError;
//! #
//! fn process_instruction(
//!     program_id: &Pubkey,
//!     accounts: &[AccountInfo],
//!     instruction_data: &[u8],
//! ) -> ProgramResult {
//!
//!     let rent = Rent::get()?;
//!     msg!("rent: {:#?}", rent);
//!
//!     Ok(())
//! }
//! #
//! # use Alembic_program::sysvar::SysvarId;
//! # let p = Rent::id();
//! # let l = &mut 1009200;
//! # let d = &mut vec![152, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 100];
//! # let a = AccountInfo::new(&p, false, false, l, d, &p, false, 0);
//! # let accounts = &[a.clone(), a];
//! # process_instruction(
//! #     &Pubkey::new_unique(),
//! #     accounts,
//! #     &[],
//! # )?;
//! # Ok::<(), ProgramError>(())
//! ```
//!
//! Accessing via on-chain program's parameters:
//!
//! ```
//! # use Alembic_program::{
//! #    account_info::{AccountInfo, next_account_info},
//! #    entrypoint::ProgramResult,
//! #    msg,
//! #    pubkey::Pubkey,
//! #    sysvar::rent::{self, Rent},
//! #    sysvar::Sysvar,
//! # };
//! # use Alembic_program::program_error::ProgramError;
//! #
//! fn process_instruction(
//!     program_id: &Pubkey,
//!     accounts: &[AccountInfo],
//!     instruction_data: &[u8],
//! ) -> ProgramResult {
//!     let account_info_iter = &mut accounts.iter();
//!     let rent_account_info = next_account_info(account_info_iter)?;
//!
//!     assert!(rent::check_id(rent_account_info.key));
//!
//!     let rent = Rent::from_account_info(rent_account_info)?;
//!     msg!("rent: {:#?}", rent);
//!
//!     Ok(())
//! }
//! #
//! # use Alembic_program::sysvar::SysvarId;
//! # let p = Rent::id();
//! # let l = &mut 1009200;
//! # let d = &mut vec![152, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 100];
//! # let a = AccountInfo::new(&p, false, false, l, d, &p, false, 0);
//! # let accounts = &[a.clone(), a];
//! # process_instruction(
//! #     &Pubkey::new_unique(),
//! #     accounts,
//! #     &[],
//! # )?;
//! # Ok::<(), ProgramError>(())
//! ```
//!
//! Accessing via the RPC client:
//!
//! ```
//! # use Alembic_program::example_mocks::Alembic_sdk;
//! # use Alembic_program::example_mocks::Alembic_rpc_client;
//! # use Alembic_sdk::account::Account;
//! # use Alembic_rpc_client::rpc_client::RpcClient;
//! # use Alembic_sdk::sysvar::rent::{self, Rent};
//! # use anyhow::Result;
//! #
//! fn print_sysvar_rent(client: &RpcClient) -> Result<()> {
//! #   client.set_get_account_response(rent::ID, Account {
//! #       lamports: 1009200,
//! #       data: vec![152, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 100],
//! #       owner: Alembic_sdk::system_program::ID,
//! #       executable: false,
//! #       rent_epoch: 307,
//! # });
//! #
//!     let rent = client.get_account(&rent::ID)?;
//!     let data: Rent = bincode::deserialize(&rent.data)?;
//!
//!     Ok(())
//! }
//! #
//! # let client = RpcClient::new(String::new());
//! # print_sysvar_rent(&client)?;
//! #
//! # Ok::<(), anyhow::Error>(())
//! ```
pub use crate::rent::Rent;
use crate::{impl_sysvar_get, program_error::ProgramError, sysvar::Sysvar};

crate::declare_sysvar_id!("SysvarRent111111111111111111111111111111111", Rent);

impl Sysvar for Rent {
    impl_sysvar_get!(sol_get_rent_sysvar);
}
