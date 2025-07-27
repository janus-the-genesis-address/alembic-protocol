use {
    crate::{
        cluster_slots_service::cluster_slots::ClusterSlots,
        repair::{outstanding_requests::OutstandingRequests, serve_repair::ShredRepairType},
    },
    Alembic_gossip::cluster_info::ClusterInfo,
    Alembic_runtime::bank_forks::BankForks,
    Alembic_sdk::{pubkey::Pubkey, quic::NotifyKeyUpdate},
    std::{
        collections::HashSet,
        net::UdpSocket,
        sync::{Arc, RwLock},
    },
};

#[derive(Clone)]
pub struct AdminRpcRequestMetadataPostInit {
    pub cluster_info: Arc<ClusterInfo>,
    pub bank_forks: Arc<RwLock<BankForks>>,
    pub vote_account: Pubkey,
    pub repair_whitelist: Arc<RwLock<HashSet<Pubkey>>>,
    pub notifies: Vec<Arc<dyn NotifyKeyUpdate + Sync + Send>>,
    pub repair_socket: Arc<UdpSocket>,
    pub outstanding_repair_requests: Arc<RwLock<OutstandingRequests<ShredRepairType>>>,
    pub cluster_slots: Arc<ClusterSlots>,
}
