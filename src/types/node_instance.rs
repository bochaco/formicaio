use crate::app::ClientGlobalState;

use super::{InactiveReason, NodeId, NodeStatus};

use alloy_primitives::U256;
use chrono::Utc;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fmt, net::IpAddr, path::PathBuf};

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 12;
// Length of nodes rewards address' prefix and suffix to be displayed
const REWARDS_ADDR_PREFIX_SUFFIX_LEN: usize = 8;

// PID of a node when running as a OS native process
pub type NodePid = u32;

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub enum ReachabilityCheckStatus {
    #[default]
    NotRun,
    InProgress(f64),
    Done(String),
    Unknown(String),
}

impl fmt::Display for ReachabilityCheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NotRun => write!(f, "Not run"),
            Self::InProgress(percent) => write!(f, "{percent:.2}% (in progress)"),
            Self::Done(s) => write!(f, "Resulted in '{s}'"),
            Self::Unknown(s) => write!(f, "Unknown, '{s}'"),
        }
    }
}

impl ReachabilityCheckStatus {
    pub fn in_progress(&self) -> bool {
        matches!(self, Self::InProgress(_))
    }
}

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub struct NodeInstanceInfo {
    /// Hex-encoded unique identifier for the node
    pub node_id: NodeId,
    /// Process ID of the node when running as a native OS process
    pub pid: Option<NodePid>,
    /// UNIX timestamp (seconds) when the node instance was created
    pub created: u64,
    /// UNIX timestamp (seconds) when the node's status last changed
    pub status_changed: u64,
    /// Current status of the node (active, inactive, etc.)
    pub status: NodeStatus,
    /// When true, the node's status is locked and cannot be changed by user actions
    pub is_status_locked: bool,
    /// When true, the node's status is unknown (unreachable for metrics); 'status' holds the last known value
    pub is_status_unknown: bool,
    /// Base58-encoded Peer ID bytes for the node
    pub peer_id: Option<String>,
    /// Additional information or message about the node's status
    pub status_info: String,
    /// Version of the node binary, if known
    pub bin_version: Option<String>,
    /// TCP port used by the node for main operations
    pub port: Option<u16>,
    /// TCP port used by the node for metrics reporting
    pub metrics_port: Option<u16>,
    /// Listening IP address set by the user for the node (IPv4 or IPv6, including special values like `0.0.0.0` or `::`)
    pub node_ip: Option<IpAddr>,
    /// Current balance of the node (if known)
    pub balance: Option<U256>,
    /// Hex-encoded rewards address for the node
    pub rewards_addr: Option<String>,
    /// Whether UPnP is enabled for this node
    pub upnp: bool,
    /// Whether reachability check is enabled for this node
    pub reachability_check: bool,
    /// Whether node logs are enabled for this node
    pub node_logs: bool,
    /// Current rewards earned by the node
    pub rewards: Option<U256>,
    /// Total number of records stored by the node
    pub records: Option<usize>,
    /// Number of relevant records for the node
    pub relevant_records: Option<usize>,
    /// Memory used by the node in MB (if active)
    pub mem_used: Option<f64>,
    /// CPU usage percentage for the node (if active)
    pub cpu_usage: Option<f64>,
    /// Node disk usage in bytes
    pub disk_usage: Option<u64>,
    /// Number of peers currently connected to the node
    pub connected_peers: Option<usize>,
    /// Number of peers in the node's k-buckets
    pub kbuckets_peers: Option<usize>,
    /// Number of times the node has shunned peers
    pub shunned_count: Option<usize>,
    /// Estimated total network size as seen by the node
    pub net_size: Option<usize>,
    /// Comma-separated list of IP addresses in the host
    pub ips: Option<String>,
    /// Custom data directory path for this node instance
    pub data_dir_path: Option<PathBuf>,
    /// Reachability status of the node (from metrics server)
    pub reachability: Option<ReachabilityCheckStatus>,
}

impl NodeInstanceInfo {
    pub fn new(node_id: impl Into<NodeId>) -> Self {
        Self {
            node_id: node_id.into(),
            ..Default::default()
        }
    }

    pub fn status_summary(&self) -> String {
        let status_str = self.status.to_string();
        if self.is_status_locked {
            format!("{status_str} (batched)")
        } else if self.is_status_unknown {
            format!("Unknown (it was {status_str})")
        } else {
            status_str
        }
    }

    pub fn lock_status(&mut self) {
        self.is_status_locked = true;
    }

    pub fn upgrade_available(&self) -> bool {
        let context = expect_context::<ClientGlobalState>();
        context.latest_bin_version.read_untracked().is_some()
            && self.bin_version.is_some()
            && context.latest_bin_version.read_untracked() != self.bin_version
    }

    pub fn upgradeable(&self) -> bool {
        self.status.is_active() && self.upgrade_available()
    }

    pub fn short_node_id(&self) -> String {
        self.node_id.short_node_id()
    }

    pub fn short_peer_id(&self) -> Option<String> {
        self.peer_id.as_ref().map(|id| {
            format!(
                "{}. . .{}",
                &id[..PEER_ID_PREFIX_SUFFIX_LEN],
                &id[id.len() - PEER_ID_PREFIX_SUFFIX_LEN..]
            )
        })
    }

    pub fn short_rewards_addr(&self) -> Option<String> {
        self.rewards_addr.as_ref().map(|addr| {
            format!(
                "0x{}...{}",
                &addr[..REWARDS_ADDR_PREFIX_SUFFIX_LEN],
                &addr[addr.len() - REWARDS_ADDR_PREFIX_SUFFIX_LEN..]
            )
        })
    }

    pub fn set_status_active(&mut self) {
        if !self.is_status_unknown || self.status.is_inactive() {
            self.is_status_unknown = false;
            self.set_status_and_ts(NodeStatus::Active);
        }
    }

    pub fn set_status_inactive(&mut self, reason: InactiveReason) {
        self.is_status_unknown = false;
        self.set_status_and_ts(NodeStatus::Inactive(reason));
    }

    pub fn set_status_to_unknown(&mut self) {
        self.set_status_changed_now();
        self.is_status_unknown = true;
        self.mem_used = None;
        self.cpu_usage = None;
        self.disk_usage = None;
        self.records = Some(0);
        self.relevant_records = None;
        self.connected_peers = Some(0);
        self.kbuckets_peers = Some(0);
        self.shunned_count = None;
        self.net_size = None;
    }

    pub fn set_status_changed_now(&mut self) {
        self.status_changed = Utc::now().timestamp() as u64;
    }

    fn set_status_and_ts(&mut self, status: NodeStatus) {
        if self.status != status {
            self.set_status_changed_now();
            self.status = status;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn now_ts() -> u64 {
        Utc::now().timestamp() as u64
    }

    #[test]
    fn test_node_instance_info_default_and_new() {
        let default_info = NodeInstanceInfo::default();
        assert_eq!(default_info.node_id, "".into());
        assert_eq!(default_info.status, NodeStatus::Creating);
        assert!(!default_info.is_status_locked);
        assert!(!default_info.is_status_unknown);

        let info = NodeInstanceInfo::new("node123");
        assert_eq!(info.node_id, "node123".into());
        assert_eq!(info.status, NodeStatus::Creating);
    }

    #[test]
    fn test_status_summary_and_lock_status() {
        let mut info = NodeInstanceInfo::new("node1");
        info.status = NodeStatus::Active;
        assert_eq!(info.status_summary(), "Active");
        info.is_status_locked = true;
        assert_eq!(info.status_summary(), "Active (batched)");
        info.is_status_locked = false;
        info.is_status_unknown = true;
        assert_eq!(info.status_summary(), "Unknown (it was Active)");
    }

    #[test]
    fn test_set_status_active_and_inactive() {
        let mut info = NodeInstanceInfo::new("node2");
        info.status = NodeStatus::Inactive(InactiveReason::Stopped);
        info.is_status_unknown = true;
        info.set_status_active();
        assert_eq!(info.status, NodeStatus::Active);
        assert!(!info.is_status_unknown);

        info.set_status_inactive(InactiveReason::Exited("bye".to_string()));
        assert_eq!(
            info.status,
            NodeStatus::Inactive(InactiveReason::Exited("bye".to_string()))
        );
        assert!(!info.is_status_unknown);
    }

    #[test]
    fn test_set_status_to_unknown() {
        let mut info = NodeInstanceInfo::new("node3");
        info.status = NodeStatus::Active;
        info.set_status_to_unknown();
        assert!(info.is_status_unknown);
        assert_eq!(info.mem_used, None);
        assert_eq!(info.cpu_usage, None);
        assert_eq!(info.disk_usage, None);
        assert_eq!(info.records, Some(0));
        assert_eq!(info.connected_peers, Some(0));
        assert_eq!(info.kbuckets_peers, Some(0));
    }

    #[test]
    fn test_lock_status() {
        let mut info = NodeInstanceInfo::new("node4");
        assert!(!info.is_status_locked);
        info.lock_status();
        assert!(info.is_status_locked);
    }

    #[test]
    fn test_set_status_changed_now_updates_timestamp() {
        let mut info = NodeInstanceInfo::new("node5");
        let before = now_ts();
        info.set_status_changed_now();
        let after = now_ts();
        assert!(info.status_changed >= before && info.status_changed <= after);
    }

    #[test]
    fn test_set_status_active_updates_status_changed() {
        let mut info = NodeInstanceInfo::new("node6");
        info.status = NodeStatus::Inactive(InactiveReason::Stopped);
        info.is_status_unknown = true;
        let before = now_ts();
        info.set_status_active();
        let after = now_ts();
        assert_eq!(info.status, NodeStatus::Active);
        assert!(!info.is_status_unknown);
        assert!(info.status_changed >= before && info.status_changed <= after);
    }

    #[test]
    fn test_set_status_inactive_updates_status_changed() {
        let mut info = NodeInstanceInfo::new("node7");
        info.status = NodeStatus::Active;
        let before = now_ts();
        info.set_status_inactive(InactiveReason::Exited("bye".to_string()));
        let after = now_ts();
        assert_eq!(
            info.status,
            NodeStatus::Inactive(InactiveReason::Exited("bye".to_string()))
        );
        assert!(!info.is_status_unknown);
        assert!(info.status_changed >= before && info.status_changed <= after);
    }

    #[test]
    fn test_set_status_to_unknown_updates_status_changed() {
        let mut info = NodeInstanceInfo::new("node8");
        info.status = NodeStatus::Active;
        let before = now_ts();
        info.set_status_to_unknown();
        let after = now_ts();
        assert!(info.is_status_unknown);
        assert!(info.status_changed >= before && info.status_changed <= after);
    }
}
