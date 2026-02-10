use super::{NodeFilter, NodeId};

use serde::{Deserialize, Serialize};
use std::{
    fmt,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

/// Represents the current status of a batch operation on nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BatchStatus {
    /// The batch is scheduled and waiting to be processed.
    Scheduled,
    /// The batch is currently being processed.
    InProgress,
    /// The batch is currently being processed, but some actions have failed.
    /// Contains the count of failed actions and the last error encountered.
    InProgressWithFailures(u16, String),
    /// The batch has completed with failures. Contains the last error encountered.
    Failed(String),
}

impl fmt::Display for BatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BatchStatus::Scheduled => write!(f, "Scheduled"),
            BatchStatus::InProgress => write!(f, "In progress"),
            BatchStatus::InProgressWithFailures(count, msg) => {
                write!(f, "In progress with {count} failures, last error: {msg}")
            }
            BatchStatus::Failed(msg) => write!(f, "Failed, last error: {msg}"),
        }
    }
}

impl BatchStatus {
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_) | Self::InProgressWithFailures(_, _))
    }
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// Represents a batch of actions to be performed on node instances, such as creation, start, stop, etc.
/// Used to track the progress and status of bulk node operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodesActionsBatch {
    /// Unique identifier for the batch.
    pub id: u16,
    /// Current status of the batch (e.g., "Scheduled", "Running", "Completed").
    pub status: BatchStatus,
    /// The type of batch operation and its associated data.
    pub batch_type: BatchType,
    /// Interval in seconds between each action in the batch.
    pub interval_secs: u64,
    /// Number of actions completed successfully in the batch.
    pub complete: u16,
}

impl NodesActionsBatch {
    /// Create a new instance
    pub fn new(id: u16, batch_type: BatchType, interval_secs: u64) -> Self {
        Self {
            id,
            status: BatchStatus::Scheduled,
            batch_type,
            interval_secs,
            complete: 0,
        }
    }
}

/// Describes the type of batch operation to perform on nodes, along with any required data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BatchType {
    /// Create a batch of new node instances with the given options and count.
    Create {
        /// Options for the nodes to be created in this batch.
        node_opts: NodeOpts,
        /// Number of nodes to create.
        count: u16,
    },
    /// Start the specified node instances.
    Start(Vec<NodeId>),
    /// Stop the specified node instances.
    Stop(Vec<NodeId>),
    /// Upgrade the specified node instances.
    Upgrade(Vec<NodeId>),
    /// Recycle (restart with new peer-id) the specified node instances.
    Recycle(Vec<NodeId>),
    /// Remove (delete) the specified node instances.
    Remove(Vec<NodeId>),
}

impl BatchType {
    pub fn is_not_create(&self) -> bool {
        !matches!(self, Self::Create { .. })
    }

    pub fn ids(&self) -> Vec<NodeId> {
        match self {
            Self::Create { .. } => vec![],
            Self::Start(ids)
            | Self::Stop(ids)
            | Self::Upgrade(ids)
            | Self::Recycle(ids)
            | Self::Remove(ids) => ids.clone(),
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
    /// Listening IP address set by the user for the node (IPv4 or IPv6, including special values like `0.0.0.0` or `::`)
    pub node_ip: IpAddr,
    /// TCP port used by the node for main operations
    pub port: u16,
    /// TCP port used by the node for metrics reporting
    pub metrics_port: u16,
    /// Hex-encoded rewards address for the node
    pub rewards_addr: String,
    /// Whether UPnP is enabled for this node
    pub upnp: bool,
    /// Whether reachability check is enabled for this node
    pub reachability_check: bool,
    /// Whether node logs are enabled for this node
    pub node_logs: bool,
    /// Whether to automatically start the node after creation
    pub auto_start: bool,
    /// Custom data directory path for this node instance
    pub data_dir_path: PathBuf,
}

impl Default for NodeOpts {
    fn default() -> Self {
        NodeOpts {
            node_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: u16::default(),
            metrics_port: u16::default(),
            rewards_addr: String::default(),
            upnp: bool::default(),
            reachability_check: bool::default(),
            node_logs: bool::default(),
            auto_start: bool::default(),
            data_dir_path: PathBuf::default(),
        }
    }
}
