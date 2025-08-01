//! Code for stake and vote rewards

use {
    crate::storable_accounts::StorableAccounts,
    Alembic_sdk::{
        account::AccountSharedData, clock::Slot, pubkey::Pubkey, reward_info::RewardInfo,
    },
};

#[derive(AbiExample, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StakeReward {
    pub stake_pubkey: Pubkey,
    pub stake_reward_info: RewardInfo,
    pub stake_account: AccountSharedData,
}

impl StakeReward {
    pub fn get_stake_reward(&self) -> i64 {
        self.stake_reward_info.lamports
    }
}

/// allow [StakeReward] to be passed to `StoreAccounts` directly without copies or vec construction
impl<'a> StorableAccounts<'a, AccountSharedData> for (Slot, &'a [StakeReward]) {
    fn pubkey(&self, index: usize) -> &Pubkey {
        &self.1[index].stake_pubkey
    }
    fn account(&self, index: usize) -> &AccountSharedData {
        &self.1[index].stake_account
    }
    fn slot(&self, _index: usize) -> Slot {
        // per-index slot is not unique per slot when per-account slot is not included in the source data
        self.target_slot()
    }
    fn target_slot(&self) -> Slot {
        self.0
    }
    fn len(&self) -> usize {
        self.1.len()
    }
}

#[cfg(feature = "dev-context-only-utils")]
use {
    rand::Rng,
    Alembic_sdk::{
        account::WritableAccount,
        rent::Rent,
        signature::{Keypair, Signer},
    },
    Alembic_stake_program::stake_state,
    Alembic_vote_program::vote_state,
};

// These functions/fields are only usable from a dev context (i.e. tests and benches)
#[cfg(feature = "dev-context-only-utils")]
impl StakeReward {
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();

        let rent = Rent::free();

        let validator_pubkey = Alembic_sdk::pubkey::new_rand();
        let validator_stake_lamports = 20;
        let validator_staking_keypair = Keypair::new();
        let validator_voting_keypair = Keypair::new();

        let validator_vote_account = vote_state::create_account(
            &validator_voting_keypair.pubkey(),
            &validator_pubkey,
            10,
            validator_stake_lamports,
        );

        let validator_stake_account = stake_state::create_account(
            &validator_staking_keypair.pubkey(),
            &validator_voting_keypair.pubkey(),
            &validator_vote_account,
            &rent,
            validator_stake_lamports,
        );

        Self {
            stake_pubkey: Pubkey::new_unique(),
            stake_reward_info: RewardInfo {
                reward_type: Alembic_sdk::reward_type::RewardType::Staking,
                lamports: rng.gen_range(1..200),
                post_balance: 0,  /* unused atm */
                commission: None, /* unused atm */
            },

            stake_account: validator_stake_account,
        }
    }

    pub fn credit(&mut self, amount: u64) {
        self.stake_reward_info.lamports = amount as i64;
        self.stake_reward_info.post_balance += amount;
        self.stake_account.checked_add_lamports(amount).unwrap();
    }
}
