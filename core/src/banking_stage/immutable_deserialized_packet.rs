use {
    Alembic_perf::packet::Packet,
    Alembic_runtime::compute_budget_details::{ComputeBudgetDetails, GetComputeBudgetDetails},
    Alembic_sdk::{
        feature_set,
        hash::Hash,
        message::Message,
        sanitize::SanitizeError,
        short_vec::decode_shortu16_len,
        signature::Signature,
        transaction::{
            AddressLoader, SanitizedTransaction, SanitizedVersionedTransaction,
            VersionedTransaction,
        },
    },
    std::{cmp::Ordering, mem::size_of, sync::Arc},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum DeserializedPacketError {
    #[error("ShortVec Failed to Deserialize")]
    // short_vec::decode_shortu16_len() currently returns () on error
    ShortVecError(()),
    #[error("Deserialization Error: {0}")]
    DeserializationError(#[from] bincode::Error),
    #[error("overflowed on signature size {0}")]
    SignatureOverflowed(usize),
    #[error("packet failed sanitization {0}")]
    SanitizeError(#[from] SanitizeError),
    #[error("transaction failed prioritization")]
    PrioritizationFailure,
    #[error("vote transaction failure")]
    VoteTransactionError,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImmutableDeserializedPacket {
    original_packet: Packet,
    transaction: SanitizedVersionedTransaction,
    message_hash: Hash,
    is_simple_vote: bool,
    compute_budget_details: ComputeBudgetDetails,
}

impl ImmutableDeserializedPacket {
    pub fn new(packet: Packet) -> Result<Self, DeserializedPacketError> {
        let versioned_transaction: VersionedTransaction = packet.deserialize_slice(..)?;
        let sanitized_transaction = SanitizedVersionedTransaction::try_from(versioned_transaction)?;
        let message_bytes = packet_message(&packet)?;
        let message_hash = Message::hash_raw_message(message_bytes);
        let is_simple_vote = packet.meta().is_simple_vote_tx();

        // drop transaction if prioritization fails.
        let mut compute_budget_details = sanitized_transaction
            .get_compute_budget_details(packet.meta().round_compute_unit_price())
            .ok_or(DeserializedPacketError::PrioritizationFailure)?;

        // set compute unit price to zero for vote transactions
        if is_simple_vote {
            compute_budget_details.compute_unit_price = 0;
        };

        Ok(Self {
            original_packet: packet,
            transaction: sanitized_transaction,
            message_hash,
            is_simple_vote,
            compute_budget_details,
        })
    }

    pub fn original_packet(&self) -> &Packet {
        &self.original_packet
    }

    pub fn transaction(&self) -> &SanitizedVersionedTransaction {
        &self.transaction
    }

    pub fn message_hash(&self) -> &Hash {
        &self.message_hash
    }

    pub fn is_simple_vote(&self) -> bool {
        self.is_simple_vote
    }

    pub fn compute_unit_price(&self) -> u64 {
        self.compute_budget_details.compute_unit_price
    }

    pub fn compute_unit_limit(&self) -> u64 {
        self.compute_budget_details.compute_unit_limit
    }

    pub fn compute_budget_details(&self) -> ComputeBudgetDetails {
        self.compute_budget_details.clone()
    }

    // This function deserializes packets into transactions, computes the blake3 hash of transaction
    // messages, and verifies secp256k1 instructions.
    pub fn build_sanitized_transaction(
        &self,
        feature_set: &Arc<feature_set::FeatureSet>,
        votes_only: bool,
        address_loader: impl AddressLoader,
    ) -> Option<SanitizedTransaction> {
        if votes_only && !self.is_simple_vote() {
            return None;
        }
        let tx = SanitizedTransaction::try_new(
            self.transaction().clone(),
            *self.message_hash(),
            self.is_simple_vote(),
            address_loader,
        )
        .ok()?;
        tx.verify_precompiles(feature_set).ok()?;
        Some(tx)
    }
}

impl PartialOrd for ImmutableDeserializedPacket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImmutableDeserializedPacket {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compute_unit_price().cmp(&other.compute_unit_price())
    }
}

/// Read the transaction message from packet data
fn packet_message(packet: &Packet) -> Result<&[u8], DeserializedPacketError> {
    let (sig_len, sig_size) = packet
        .data(..)
        .and_then(|bytes| decode_shortu16_len(bytes).ok())
        .ok_or(DeserializedPacketError::ShortVecError(()))?;
    sig_len
        .checked_mul(size_of::<Signature>())
        .and_then(|v| v.checked_add(sig_size))
        .and_then(|msg_start| packet.data(msg_start..))
        .ok_or(DeserializedPacketError::SignatureOverflowed(sig_size))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        Alembic_sdk::{signature::Keypair, system_transaction},
    };

    #[test]
    fn simple_deserialized_packet() {
        let tx = system_transaction::transfer(
            &Keypair::new(),
            &Alembic_sdk::pubkey::new_rand(),
            1,
            Hash::new_unique(),
        );
        let packet = Packet::from_data(None, tx).unwrap();
        let deserialized_packet = ImmutableDeserializedPacket::new(packet);

        assert!(deserialized_packet.is_ok());
    }
}
