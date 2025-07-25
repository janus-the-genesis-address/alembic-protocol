//! Example Rust-based SBF program that panics

#[cfg(all(feature = "custom-panic", target_os = "Alembic"))]
#[no_mangle]
fn custom_panic(info: &core::panic::PanicInfo<'_>) {
    // Note: Full panic reporting is included here for testing purposes
    Alembic_program::msg!("program custom panic enabled");
    Alembic_program::msg!(&format!("{info}"));
}

extern crate Alembic_program;
use Alembic_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

Alembic_program::entrypoint!(process_instruction);
#[allow(clippy::unnecessary_wraps)]
fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    assert_eq!(1, 2);
    Ok(())
}
