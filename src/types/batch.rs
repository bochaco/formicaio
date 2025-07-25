use super::{NodeFilter, NodeId};

use serde::{Deserialize, Serialize};
use std::{
    fmt,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

/// Information of a node action batch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodesActionsBatch {
    pub id: u16,
    pub status: String,
    pub batch_type: BatchType,
    pub interval_secs: u64,
    pub complete: u16,
}

impl NodesActionsBatch {
    /// Create a new instance
    pub fn new(id: u16, batch_type: BatchType, interval_secs: u64) -> Self {
        Self {
            id,
            status: "Scheduled".to_string(),
            batch_type,
            interval_secs,
            complete: 0,
        }
    }
}

/// Type of batch and corresponding info needed to execute it
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BatchType {
    Create { node_opts: NodeOpts, count: u16 },
    Start(Vec<NodeId>),
    Stop(Vec<NodeId>),
    Upgrade(Vec<NodeId>),
    Recycle(Vec<NodeId>),
    Remove(Vec<NodeId>),
}

impl BatchType {
    pub fn is_not_create(&self) -> bool {
        !matches!(self, Self::Create { .. })
    }

    pub fn ids(&self) -> Vec<NodeId> {
        match self {
            Self::Create { .. } => vec![],
            Self::Start(ids) => ids.clone(),
            Self::Stop(ids) => ids.clone(),
            Self::Upgrade(ids) => ids.clone(),
            Self::Recycle(ids) => ids.clone(),
            Self::Remove(ids) => ids.clone(),
        }
    }
}

impl fmt::Display for BatchType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BatchType::Create { .. } => write!(f, "CREATE"),
            BatchType::Start(_) => write!(f, "START"),
            BatchType::Stop(_) => write!(f, "STOP"),
            BatchType::Upgrade(_) => write!(f, "UPGRADE"),
            BatchType::Recycle(_) => write!(f, "RECYCLE"),
            BatchType::Remove(_) => write!(f, "REMOVE"),
        }
    }
}

/// Type of batch to create with the list of nodes that match the filter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BatchOnMatch {
    StartOnMatch(NodeFilter),
    StopOnMatch(NodeFilter),
    UpgradeOnMatch(NodeFilter),
    RecycleOnMatch(NodeFilter),
    RemoveOnMatch(NodeFilter),
}

impl BatchOnMatch {
    pub fn set_filter(&mut self, filter: NodeFilter) {
        match self {
            BatchOnMatch::StartOnMatch(f)
            | BatchOnMatch::StopOnMatch(f)
            | BatchOnMatch::UpgradeOnMatch(f)
            | BatchOnMatch::RecycleOnMatch(f)
            | BatchOnMatch::RemoveOnMatch(f) => *f = filter,
        }
    }
}

/// Options when creating a new node instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeOpts {
    pub node_ip: IpAddr,
    pub port: u16,
    pub metrics_port: u16,
    pub rewards_addr: String,
    pub home_network: bool,
    pub upnp: bool,
    pub node_logs: bool,
    pub auto_start: bool,
    pub data_dir_path: PathBuf,
}

impl Default for NodeOpts {
    fn default() -> Self {
        NodeOpts {
            node_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: u16::default(),
            metrics_port: u16::default(),
            rewards_addr: String::default(),
            home_network: bool::default(),
            upnp: bool::default(),
            node_logs: bool::default(),
            auto_start: bool::default(),
            data_dir_path: PathBuf::default(),
        }
    }
}
