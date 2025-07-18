use crate::app::ClientGlobalState;

use super::{InactiveReason, NodeStatus};

use alloy_primitives::U256;
use chrono::Utc;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::PathBuf};

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 12;
// Length of nodes ids' prefix to be displayed
const NODE_ID_PREFIX_LEN: usize = 12;
// Length of nodes rewards address' prefix and suffix to be displayed
const REWARDS_ADDR_PREFIX_SUFFIX_LEN: usize = 8;

// Hex-encoded node id
pub type NodeId = String;
// PID of a node when running as a OS native process
pub type NodePid = u32;

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub struct NodeInstanceInfo {
    pub node_id: NodeId,
    pub pid: Option<NodePid>,
    pub created: u64,
    pub status_changed: u64,
    pub status: NodeStatus,
    // When locked, users cannot change its status by executing any type of action on it.
    pub is_status_locked: bool,
    // Its status is not known, it has been unreachable when trying fetch metrics.
    // The value kept in 'status' field is the last one being known.
    pub is_status_unknown: bool,
    pub peer_id: Option<String>, // base58-encoded Peer Id bytes
    pub status_info: String,
    pub bin_version: Option<String>,
    pub port: Option<u16>,
    pub metrics_port: Option<u16>,
    pub node_ip: Option<IpAddr>,
    pub balance: Option<U256>,
    pub rewards_addr: Option<String>, // hex-encoded rewards address
    pub home_network: bool,
    pub upnp: bool,
    pub node_logs: bool,
    pub rewards: Option<U256>,
    pub records: Option<usize>,
    pub relevant_records: Option<usize>,
    pub mem_used: Option<f64>,
    pub cpu_usage: Option<f64>,
    pub connected_peers: Option<usize>,
    pub connected_relay_clients: Option<usize>,
    pub kbuckets_peers: Option<usize>,
    pub shunned_count: Option<usize>,
    pub net_size: Option<usize>,
    pub ips: Option<String>,
    pub data_dir_path: Option<PathBuf>, // Custom data directory path for this node
}

impl NodeInstanceInfo {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
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
        if self.node_id.len() > NODE_ID_PREFIX_LEN {
            self.node_id[..NODE_ID_PREFIX_LEN].to_string()
        } else {
            self.node_id.clone()
        }
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
        self.records = Some(0);
        self.relevant_records = None;
        self.connected_peers = Some(0);
        self.connected_relay_clients = None;
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
        assert_eq!(default_info.node_id, "");
        assert_eq!(default_info.status, NodeStatus::Creating);
        assert!(!default_info.is_status_locked);
        assert!(!default_info.is_status_unknown);

        let info = NodeInstanceInfo::new("node123".to_string());
        assert_eq!(info.node_id, "node123");
        assert_eq!(info.status, NodeStatus::Creating);
    }

    #[test]
    fn test_status_summary_and_lock_status() {
        let mut info = NodeInstanceInfo::new("node1".to_string());
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
        let mut info = NodeInstanceInfo::new("node2".to_string());
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
        let mut info = NodeInstanceInfo::new("node3".to_string());
        info.status = NodeStatus::Active;
        info.set_status_to_unknown();
        assert!(info.is_status_unknown);
        assert_eq!(info.mem_used, None);
        assert_eq!(info.cpu_usage, None);
        assert_eq!(info.records, Some(0));
        assert_eq!(info.connected_peers, Some(0));
        assert_eq!(info.kbuckets_peers, Some(0));
    }

    #[test]
    fn test_lock_status() {
        let mut info = NodeInstanceInfo::new("node4".to_string());
        assert!(!info.is_status_locked);
        info.lock_status();
        assert!(info.is_status_locked);
    }

    #[test]
    fn test_set_status_changed_now_updates_timestamp() {
        let mut info = NodeInstanceInfo::new("node5".to_string());
        let before = now_ts();
        info.set_status_changed_now();
        let after = now_ts();
        assert!(info.status_changed >= before && info.status_changed <= after);
    }

    #[test]
    fn test_set_status_active_updates_status_changed() {
        let mut info = NodeInstanceInfo::new("node6".to_string());
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
        let mut info = NodeInstanceInfo::new("node7".to_string());
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
        let mut info = NodeInstanceInfo::new("node8".to_string());
        info.status = NodeStatus::Active;
        let before = now_ts();
        info.set_status_to_unknown();
        let after = now_ts();
        assert!(info.is_status_unknown);
        assert!(info.status_changed >= before && info.status_changed <= after);
    }
}
