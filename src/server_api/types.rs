pub use crate::{
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
    pub fn matches(&self, node_info: &NodeInstanceInfo) -> bool {
        match self {
            Self::Active => node_info.status.is_active(),
            Self::Restarting => node_info.status.is_restarting(),
            Self::Stopping => node_info.status.is_stopping(),
            Self::Removing => node_info.status.is_removing(),
            Self::Upgrading => node_info.status.is_upgrading(),
            Self::Recycling => node_info.status.is_recycling(),
            Self::Batched => node_info.is_status_locked,
            Self::Inactive => node_info.status.is_inactive(),
            Self::Created => node_info.status.is_created(),
            Self::Stopped => node_info.status.is_stopped(),
            Self::StartFailed => node_info.status.is_start_failed(),
            Self::Exited => node_info.status.is_exited() || node_info.status.is_inactive_unknown(),
            Self::Unknown => node_info.status.is_inactive_unknown(),
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
    fn status_filter_apply(&self, node_info: &NodeInstanceInfo, fallback_val: bool) -> bool {
        if let Some(s) = self.status.as_ref() {
            if s.is_empty() {
                fallback_val
            } else {
                s.iter().any(|sf| sf.matches(node_info))
            }
        } else {
            fallback_val
        }
    }

    pub fn passes(&self, node_info: &NodeInstanceInfo) -> bool {
        if let Some(ids) = self.node_ids.as_ref() {
            if ids.is_empty() {
                self.status_filter_apply(node_info, true)
            } else {
                ids.contains(&node_info.node_id) || self.status_filter_apply(node_info, false)
            }
        } else {
            self.status_filter_apply(node_info, true)
        }
    }

    pub fn matches(&self, node_info: &NodeInstanceInfo) -> bool {
        self.node_ids
            .as_ref()
            .map(|ids| ids.contains(&node_info.node_id))
            .unwrap_or(false)
            || self.status_filter_apply(node_info, false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_filter_default_and_matches() {
        let filter = NodeFilter::default();
        let mut info = NodeInstanceInfo::new("node1".to_string());
        info.status = NodeStatus::Active;
        assert!(filter.passes(&info));
        assert!(!filter.matches(&info));
    }

    #[test]
    fn test_node_filter_with_node_ids() {
        let filter = NodeFilter {
            node_ids: Some(vec!["node1".to_string(), "node2".to_string()]),
            status: None,
        };
        let info1 = NodeInstanceInfo::new("node1".to_string());
        let info2 = NodeInstanceInfo::new("node2".to_string());
        let info3 = NodeInstanceInfo::new("node3".to_string());

        assert!(filter.passes(&info1));
        assert!(filter.passes(&info2));
        assert!(!filter.passes(&info3));
        assert!(filter.matches(&info1));
        assert!(filter.matches(&info2));
        assert!(!filter.matches(&info3));
    }

    #[test]
    fn test_node_filter_with_status_filters() {
        let filter = NodeFilter {
            node_ids: None,
            status: Some(vec![NodeStatusFilter::Active, NodeStatusFilter::Restarting]),
        };
        let mut active_info = NodeInstanceInfo::new("active_node".to_string());
        active_info.status = NodeStatus::Active;
        let mut restarting_info = NodeInstanceInfo::new("restarting_node".to_string());
        restarting_info.status = NodeStatus::Restarting;
        let mut inactive_info = NodeInstanceInfo::new("inactive_node".to_string());
        inactive_info.status = NodeStatus::Inactive(InactiveReason::Stopped);

        assert!(filter.passes(&active_info));
        assert!(filter.passes(&restarting_info));
        assert!(!filter.passes(&inactive_info));
        assert!(filter.matches(&active_info));
        assert!(filter.matches(&restarting_info));
        assert!(!filter.matches(&inactive_info));
    }

    #[test]
    fn test_node_filter_with_both_node_ids_and_status() {
        let filter = NodeFilter {
            node_ids: Some(vec!["node1".to_string()]),
            status: Some(vec![NodeStatusFilter::Active]),
        };
        let mut matching_info = NodeInstanceInfo::new("node1".to_string());
        matching_info.status = NodeStatus::Active;
        let mut wrong_status_info = NodeInstanceInfo::new("node1".to_string());
        wrong_status_info.status = NodeStatus::Inactive(InactiveReason::Stopped);
        let mut wrong_id_info = NodeInstanceInfo::new("node2".to_string());
        wrong_id_info.status = NodeStatus::Active;

        // passes() should return true if either node_id matches OR status matches
        assert!(filter.passes(&matching_info));
        assert!(filter.passes(&wrong_status_info)); // node_id matches
        assert!(filter.passes(&wrong_id_info)); // status matches

        // matches() should return true if either node_id matches OR status matches
        assert!(filter.matches(&matching_info));
        assert!(filter.matches(&wrong_status_info)); // node_id matches
        assert!(filter.matches(&wrong_id_info)); // status matches
    }

    #[test]
    fn test_node_filter_with_inactive_reasons() {
        let filter = NodeFilter {
            node_ids: None,
            status: Some(vec![
                NodeStatusFilter::Created,
                NodeStatusFilter::Stopped,
                NodeStatusFilter::StartFailed,
                NodeStatusFilter::Exited,
                NodeStatusFilter::Unknown,
            ]),
        };

        let mut created_info = NodeInstanceInfo::new("created_node".to_string());
        created_info.status = NodeStatus::Inactive(InactiveReason::Created);
        let mut stopped_info = NodeInstanceInfo::new("stopped_node".to_string());
        stopped_info.status = NodeStatus::Inactive(InactiveReason::Stopped);
        let mut start_failed_info = NodeInstanceInfo::new("start_failed_node".to_string());
        start_failed_info.status =
            NodeStatus::Inactive(InactiveReason::StartFailed("error".to_string()));
        let mut exited_info = NodeInstanceInfo::new("exited_node".to_string());
        exited_info.status = NodeStatus::Inactive(InactiveReason::Exited("bye".to_string()));
        let mut unknown_info = NodeInstanceInfo::new("unknown_node".to_string());
        unknown_info.status = NodeStatus::Inactive(InactiveReason::Unknown);

        assert!(filter.passes(&created_info));
        assert!(filter.passes(&stopped_info));
        assert!(filter.passes(&start_failed_info));
        assert!(filter.passes(&exited_info));
        assert!(filter.passes(&unknown_info));
    }

    #[test]
    fn test_node_filter_with_batched_status() {
        let filter = NodeFilter {
            node_ids: None,
            status: Some(vec![NodeStatusFilter::Batched]),
        };
        let mut locked_info = NodeInstanceInfo::new("locked_node".to_string());
        locked_info.is_status_locked = true;
        let mut unlocked_info = NodeInstanceInfo::new("unlocked_node".to_string());
        unlocked_info.is_status_locked = false;

        assert!(filter.passes(&locked_info));
        assert!(!filter.passes(&unlocked_info));
    }

    #[test]
    fn test_node_filter_empty_filters() {
        let empty_filter = NodeFilter {
            node_ids: None,
            status: None,
        };
        let info = NodeInstanceInfo::new("any_node".to_string());

        // passes() should return true when no filters are set
        assert!(empty_filter.passes(&info));
        // matches() should return false when no filters are set
        assert!(!empty_filter.matches(&info));
    }

    #[test]
    fn test_node_filter_with_empty_vectors() {
        let empty_ids_filter = NodeFilter {
            node_ids: Some(vec![]),
            status: None,
        };
        let empty_status_filter = NodeFilter {
            node_ids: None,
            status: Some(vec![]),
        };
        let info = NodeInstanceInfo::new("any_node".to_string());

        // Both should behave like no filters
        assert!(empty_ids_filter.passes(&info));
        assert!(!empty_ids_filter.matches(&info));
        assert!(empty_status_filter.passes(&info));
        assert!(!empty_status_filter.matches(&info));
    }
}
