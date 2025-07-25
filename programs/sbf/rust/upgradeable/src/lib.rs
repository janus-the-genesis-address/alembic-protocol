//! Example Rust-based SBF upgradeable program

extern crate Alembic_program;
use Alembic_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, pubkey::Pubkey, sysvar::clock,
};

Alembic_program::entrypoint!(process_instruction);
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("Upgradeable program");
    assert_eq!(accounts.len(), 1);
    assert_eq!(*accounts[0].key, clock::id());
    Err(42.into())
}
