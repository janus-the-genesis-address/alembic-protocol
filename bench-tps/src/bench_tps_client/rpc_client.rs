use {
    crate::bench_tps_client::{BenchTpsClient, BenchTpsError, Result},
    Alembic_rpc_client::rpc_client::RpcClient,
    Alembic_rpc_client_api::config::RpcBlockConfig,
    Alembic_sdk::{
        account::Account, commitment_config::CommitmentConfig, epoch_info::EpochInfo, hash::Hash,
        message::Message, pubkey::Pubkey, signature::Signature, slot_history::Slot,
        transaction::Transaction,
    },
    Alembic_transaction_status::UiConfirmedBlock,
};

impl BenchTpsClient for RpcClient {
    fn send_transaction(&self, transaction: Transaction) -> Result<Signature> {
        RpcClient::send_transaction(self, &transaction).map_err(|err| err.into())
    }

    fn send_batch(&self, transactions: Vec<Transaction>) -> Result<()> {
        for transaction in transactions {
            BenchTpsClient::send_transaction(self, transaction)?;
        }
        Ok(())
    }
    fn get_latest_blockhash(&self) -> Result<Hash> {
        RpcClient::get_latest_blockhash(self).map_err(|err| err.into())
    }

    fn get_latest_blockhash_with_commitment(
        &self,
        commitment_config: CommitmentConfig,
    ) -> Result<(Hash, u64)> {
        RpcClient::get_latest_blockhash_with_commitment(self, commitment_config)
            .map_err(|err| err.into())
    }

    fn get_transaction_count(&self) -> Result<u64> {
        RpcClient::get_transaction_count(self).map_err(|err| err.into())
    }

    fn get_transaction_count_with_commitment(
        &self,
        commitment_config: CommitmentConfig,
    ) -> Result<u64> {
        RpcClient::get_transaction_count_with_commitment(self, commitment_config)
            .map_err(|err| err.into())
    }

    fn get_epoch_info(&self) -> Result<EpochInfo> {
        RpcClient::get_epoch_info(self).map_err(|err| err.into())
    }

    fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        RpcClient::get_balance(self, pubkey).map_err(|err| err.into())
    }

    fn get_balance_with_commitment(
        &self,
        pubkey: &Pubkey,
        commitment_config: CommitmentConfig,
    ) -> Result<u64> {
        RpcClient::get_balance_with_commitment(self, pubkey, commitment_config)
            .map(|res| res.value)
            .map_err(|err| err.into())
    }

    fn get_fee_for_message(&self, message: &Message) -> Result<u64> {
        RpcClient::get_fee_for_message(self, message).map_err(|err| err.into())
    }

    fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        RpcClient::get_minimum_balance_for_rent_exemption(self, data_len).map_err(|err| err.into())
    }

    fn addr(&self) -> String {
        self.url()
    }

    fn request_airdrop_with_blockhash(
        &self,
        pubkey: &Pubkey,
        lamports: u64,
        recent_blockhash: &Hash,
    ) -> Result<Signature> {
        RpcClient::request_airdrop_with_blockhash(self, pubkey, lamports, recent_blockhash)
            .map_err(|err| err.into())
    }

    fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        RpcClient::get_account(self, pubkey).map_err(|err| err.into())
    }

    fn get_account_with_commitment(
        &self,
        pubkey: &Pubkey,
        commitment_config: CommitmentConfig,
    ) -> Result<Account> {
        RpcClient::get_account_with_commitment(self, pubkey, commitment_config)
            .map(|res| res.value)
            .map_err(|err| err.into())
            .and_then(|account| {
                account.ok_or_else(|| {
                    BenchTpsError::Custom(format!("AccountNotFound: pubkey={pubkey}"))
                })
            })
    }

    fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        RpcClient::get_multiple_accounts(self, pubkeys).map_err(|err| err.into())
    }

    fn get_slot_with_commitment(&self, commitment_config: CommitmentConfig) -> Result<Slot> {
        RpcClient::get_slot_with_commitment(self, commitment_config).map_err(|err| err.into())
    }

    fn get_blocks_with_commitment(
        &self,
        start_slot: Slot,
        end_slot: Option<Slot>,
        commitment_config: CommitmentConfig,
    ) -> Result<Vec<Slot>> {
        RpcClient::get_blocks_with_commitment(self, start_slot, end_slot, commitment_config)
            .map_err(|err| err.into())
    }

    fn get_block_with_config(
        &self,
        slot: Slot,
        rpc_block_config: RpcBlockConfig,
    ) -> Result<UiConfirmedBlock> {
        RpcClient::get_block_with_config(self, slot, rpc_block_config).map_err(|err| err.into())
    }
}
