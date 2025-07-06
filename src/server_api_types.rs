pub use super::{
    node_instance::{InactiveReason, NodeId, NodeInstanceInfo, NodeStatus},
    sort_nodes::NodesSortStrategy,
};

use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

/// List of nodes
pub type NodeList = HashMap<String, NodeInstanceInfo>;

/// API node status filters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum NodeStatusFilter {
    Active,
    Restarting,
    Stopping,
    Removing,
    Upgrading,
    Recycling,
    Batched,
    Inactive,
    Created,
    Stopped,
    StartFailed,
    Exited,
    Unknown,
}

impl NodeStatusFilter {
    pub fn matches(&self, status: &NodeStatus) -> bool {
        match self {
            Self::Active => status.is_active(),
            Self::Restarting => matches!(status, NodeStatus::Restarting),
            Self::Stopping => matches!(status, NodeStatus::Stopping),
            Self::Removing => matches!(status, NodeStatus::Removing),
            Self::Upgrading => matches!(status, NodeStatus::Upgrading),
            Self::Recycling => matches!(status, NodeStatus::Recycling),
            Self::Batched => matches!(status, NodeStatus::Locked(_)),
            Self::Inactive => status.is_inactive(),
            Self::Created => matches!(status, NodeStatus::Inactive(InactiveReason::Created)),
            Self::Stopped => matches!(status, NodeStatus::Inactive(InactiveReason::Stopped)),
            Self::StartFailed => {
                matches!(status, NodeStatus::Inactive(InactiveReason::StartFailed(_)))
            }
            Self::Exited => matches!(
                status,
                NodeStatus::Inactive(InactiveReason::Exited(_) | InactiveReason::Unknown)
            ),
            Self::Unknown => matches!(status, NodeStatus::Inactive(InactiveReason::Unknown)),
        }
    }
}

/// API node filters
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeFilter {
    pub node_ids: Option<Vec<NodeId>>,
    pub status: Option<Vec<NodeStatusFilter>>,
}

impl NodeFilter {
    fn status_filter_apply(&self, status: &NodeStatus, fallback_val: bool) -> bool {
        self.status
            .as_ref()
            .map(|s| s.iter().any(|sf| sf.matches(status)))
            .unwrap_or(fallback_val)
    }

    pub fn passes(&self, node_id: &NodeId, status: &NodeStatus) -> bool {
        if let Some(ids) = self.node_ids.as_ref() {
            ids.contains(node_id) || self.status_filter_apply(status, false)
        } else {
            self.status_filter_apply(status, true)
        }
    }

    pub fn matches(&self, node_id: &NodeId, status: &NodeStatus) -> bool {
        self.node_ids
            .as_ref()
            .map(|ids| ids.contains(node_id))
            .unwrap_or(false)
            || self.status_filter_apply(status, false)
    }
}

/// List of nodes, stats and currently running batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodesInstancesInfo {
    pub latest_bin_version: Option<String>,
    pub nodes: NodeList,
    pub stats: Stats,
    pub scheduled_batches: Vec<NodesActionsBatch>,
}

/// Application settings values.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppSettings {
    pub nodes_auto_upgrade: bool,
    pub nodes_auto_upgrade_delay: Duration,
    pub node_bin_version_polling_freq: Duration,
    pub nodes_metrics_polling_freq: Duration,
    pub rewards_balances_retrieval_freq: Duration,
    pub l2_network_rpc_url: String,
    pub token_contract_address: String,
    pub lcd_display_enabled: bool,
    pub lcd_device: String,
    pub lcd_addr: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Node auto-upgrading is disabled by default.
            nodes_auto_upgrade: false,
            // Delay 10 secs. between each node being auto-upgraded.
            nodes_auto_upgrade_delay: Duration::from_secs(10),
            // Check latest version of node binary every couple of hours.
            node_bin_version_polling_freq: Duration::from_secs(60 * 60 * 2),
            // How often to fetch metrics and node info from active/running nodes
            nodes_metrics_polling_freq: Duration::from_secs(5),
            // Retrieve balances every 15 mins.
            rewards_balances_retrieval_freq: Duration::from_secs(60 * 15),
            // Arbitrum One network.
            l2_network_rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
            // ANT token contract on Arbitrum One network.
            token_contract_address: "0xa78d8321B20c4Ef90eCd72f2588AA985A4BDb684".to_string(),
            // External LCD device disabled.
            lcd_display_enabled: false,
            // I2C bus number 1, i.e. device at /dev/i2c-1.
            lcd_device: "1".to_string(),
            // I2C backpack address 0x27, another common addr is: 0x3f. Check it out with 'sudo ic2detect -y <bus-number>'.
            lcd_addr: "0x27".to_string(),
        }
    }
}

/// Node stats collected by the backend and retrievable through the public server API.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total_balance: U256,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub inactive_nodes: usize,
    pub connected_peers: usize,
    pub shunned_count: usize,
    pub estimated_net_size: usize,
    pub stored_records: usize,
    pub relevant_records: usize,
}

/// Node stats formatted for UmbrelOS widgets.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WidgetFourStats {
    pub r#type: String,
    pub refresh: String,
    pub link: String,
    pub items: Vec<WidgetStat>,
}

/// Node stats collected by the backend to be retrieved for UmbrelOS widgets.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WidgetStat {
    pub title: String,
    pub text: String,
    pub subtext: String,
}

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
            Self::StartOnMatch(f) => *f = filter,
            Self::StopOnMatch(f) => *f = filter,
            Self::UpgradeOnMatch(f) => *f = filter,
            Self::RecycleOnMatch(f) => *f = filter,
            Self::RemoveOnMatch(f) => *f = filter,
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
        }
    }
}
