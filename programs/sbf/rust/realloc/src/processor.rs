//! Example Rust-based SBF realloc test program

#![cfg(feature = "program")]
#![allow(clippy::arithmetic_side_effects)]

extern crate Alembic_program;
use {
    crate::instructions::*,
    Alembic_program::{
        account_info::AccountInfo,
        entrypoint::{ProgramResult, MAX_PERMITTED_DATA_INCREASE},
        msg,
        program::invoke,
        pubkey::Pubkey,
        system_instruction, system_program,
    },
    std::{convert::TryInto, mem},
};

Alembic_program::entrypoint!(process_instruction);
#[allow(clippy::unnecessary_wraps)]
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let account = &accounts[0];

    match instruction_data[0] {
        REALLOC => {
            let (bytes, _) = instruction_data[2..].split_at(std::mem::size_of::<usize>());
            let new_len = usize::from_le_bytes(bytes.try_into().unwrap());
            msg!("realloc to {}", new_len);
            account.realloc(new_len, false)?;
            assert_eq!(new_len, account.data_len());
        }
        REALLOC_EXTEND => {
            let pre_len = account.data_len();
            let (bytes, _) = instruction_data[2..].split_at(std::mem::size_of::<usize>());
            let new_len = pre_len.saturating_add(usize::from_le_bytes(bytes.try_into().unwrap()));
            msg!("realloc extend by {}", new_len);
            account.realloc(new_len, false)?;
            assert_eq!(new_len, account.data_len());
        }
        REALLOC_EXTEND_AND_UNDO => {
            let pre_len = account.data_len();
            let (bytes, _) = instruction_data[2..].split_at(std::mem::size_of::<usize>());
            let new_len = pre_len.saturating_add(usize::from_le_bytes(bytes.try_into().unwrap()));
            msg!("realloc extend by {}", new_len);
            account.realloc(new_len, false)?;
            msg!("undo realloc");
            account.realloc(pre_len, false)?;
            assert_eq!(pre_len, account.data_len());
        }
        REALLOC_EXTEND_AND_FILL => {
            let pre_len = account.data_len();
            let fill = instruction_data[2];
            let (bytes, _) = instruction_data[4..].split_at(std::mem::size_of::<usize>());
            let new_len = pre_len.saturating_add(usize::from_le_bytes(bytes.try_into().unwrap()));
            msg!("realloc extend by {}", new_len);
            account.realloc(new_len, false)?;
            assert_eq!(new_len, account.data_len());
            account.try_borrow_mut_data()?[pre_len..].fill(fill);
        }
        // extend the account and do a 8-bytes write across the original account
        // length and the realloc region
        EXTEND_AND_WRITE_U64 => {
            let pre_len = account.data_len();
            let new_len = mem::size_of::<u64>();
            assert!(pre_len < new_len);
            account.realloc(new_len, false)?;
            assert_eq!(new_len, account.data_len());

            let (bytes, _) = instruction_data[1..].split_at(new_len);
            let value = u64::from_le_bytes(bytes.try_into().unwrap());
            msg!(
                "write {} to account {:p}",
                value,
                account.try_borrow_data().unwrap().as_ptr()
            );
            // exercise memory write
            unsafe {
                *account
                    .try_borrow_mut_data()
                    .unwrap()
                    .as_mut_ptr()
                    .cast::<u64>() = value
            };
            // exercise memory read
            assert_eq!(
                unsafe {
                    *account
                        .try_borrow_mut_data()
                        .unwrap()
                        .as_ptr()
                        .cast::<u64>()
                },
                value
            );
        }
        REALLOC_AND_ASSIGN => {
            msg!("realloc and assign");
            account.realloc(MAX_PERMITTED_DATA_INCREASE, false)?;
            assert_eq!(MAX_PERMITTED_DATA_INCREASE, account.data_len());
            account.assign(&system_program::id());
            assert_eq!(*account.owner, system_program::id());
        }
        REALLOC_AND_ASSIGN_TO_SELF_VIA_SYSTEM_PROGRAM => {
            msg!("realloc and assign to self via system program");
            let pre_len = account.data_len();
            account.realloc(pre_len.saturating_add(MAX_PERMITTED_DATA_INCREASE), false)?;
            assert_eq!(
                pre_len.saturating_add(MAX_PERMITTED_DATA_INCREASE),
                account.data_len()
            );
            invoke(
                &system_instruction::assign(account.key, program_id),
                accounts,
            )?;
            assert_eq!(account.owner, program_id);
        }
        ASSIGN_TO_SELF_VIA_SYSTEM_PROGRAM_AND_REALLOC => {
            msg!("assign to self via system program and realloc");
            let pre_len = account.data_len();
            invoke(
                &system_instruction::assign(account.key, program_id),
                accounts,
            )?;
            assert_eq!(account.owner, program_id);
            account.realloc(pre_len.saturating_add(MAX_PERMITTED_DATA_INCREASE), false)?;
            assert_eq!(
                account.data_len(),
                pre_len.saturating_add(MAX_PERMITTED_DATA_INCREASE)
            );
        }
        DEALLOC_AND_ASSIGN_TO_CALLER => {
            msg!("dealloc and assign to caller");
            account.realloc(0, false)?;
            assert_eq!(account.data_len(), 0);
            account.assign(accounts[1].key);
            assert_eq!(account.owner, accounts[1].key);
        }
        CHECK => {
            msg!("check");
            assert_eq!(100, account.data_len());
            let data = account.try_borrow_mut_data()?;
            for x in data[0..5].iter() {
                assert_eq!(0, *x);
            }
            for x in data[5..].iter() {
                assert_eq!(2, *x);
            }
        }
        ZERO_INIT => {
            account.realloc(10, false)?;
            {
                let mut data = account.try_borrow_mut_data()?;
                for i in 0..10 {
                    assert_eq!(0, data[i]);
                }
                data.fill(1);
                for i in 0..10 {
                    assert_eq!(1, data[i]);
                }
            }

            account.realloc(5, false)?;
            account.realloc(10, false)?;
            {
                let data = account.try_borrow_data()?;
                for i in 0..10 {
                    assert_eq!(1, data[i]);
                }
            }

            account.realloc(5, false)?;
            account.realloc(10, true)?;
            {
                let data = account.try_borrow_data()?;
                for i in 0..5 {
                    assert_eq!(1, data[i]);
                }
                for i in 5..10 {
                    assert_eq!(0, data[i]);
                }
            }
        }
        REALLOC_EXTEND_FROM_SLICE => {
            msg!("realloc extend from slice");
            let data = &instruction_data[1..];
            let prev_len = account.data_len();
            account.realloc(prev_len.saturating_add(data.len()), false)?;
            account.data.borrow_mut()[prev_len..].copy_from_slice(data);
        }
        _ => panic!(),
    }

    Ok(())
}
