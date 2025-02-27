use super::app::ClientGlobalState;

use alloy_primitives::U256;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

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
pub enum NodeStatus {
    #[default]
    Creating,
    // A running node connected to peers on the network is considered Active.
    Active,
    Restarting,
    Stopping,
    // A node not connected to any peer on the network is considered Inactive.
    Inactive,
    Removing,
    Upgrading,
    // The node's peer-id is cleared and restarted with a fresh new one
    Recycling,
    // This is a special state just to provide a good UX, after going thru some status
    // change, e.g. Restarting, Upgrading, we set to this state till we get actual state
    // from the server during our polling cycle. The string describes the type of transition.
    Transitioned(String),
    // Locked, users cannot change its status by executing any type of action on it.
    // It also holds the current status.
    Locked(Box<NodeStatus>),
}

impl NodeStatus {
    pub fn is_creating(&self) -> bool {
        matches!(self, Self::Creating)
    }
    pub fn is_active(&self) -> bool {
        match self {
            Self::Active => true,
            Self::Locked(s) => s.is_active(),
            _ => false,
        }
    }
    pub fn is_inactive(&self) -> bool {
        match self {
            Self::Inactive => true,
            Self::Locked(s) => s.is_inactive(),
            _ => false,
        }
    }
    pub fn is_recycling(&self) -> bool {
        matches!(self, Self::Recycling)
    }
    pub fn is_upgrading(&self) -> bool {
        matches!(self, Self::Upgrading)
    }
    pub fn is_transitioning(&self) -> bool {
        matches!(
            self,
            Self::Creating
                | Self::Restarting
                | Self::Stopping
                | Self::Removing
                | Self::Upgrading
                | Self::Recycling
                | Self::Transitioned(_)
        )
    }
    pub fn is_transitioned(&self) -> bool {
        matches!(self, Self::Transitioned(_))
    }
    pub fn is_locked(&self) -> bool {
        matches!(self, Self::Locked(_))
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Transitioned(s) => write!(f, "{s}"),
            Self::Locked(s) => write!(f, "{s} (batched)"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub struct NodeInstanceInfo {
    pub node_id: NodeId,
    pub pid: Option<NodePid>,
    pub created: u64,
    pub status_changed: Option<u64>,
    pub peer_id: Option<String>, // base58-encoded Peer Id bytes
    pub status: NodeStatus,
    pub status_info: String,
    pub bin_version: Option<String>,
    pub port: Option<u16>,
    pub metrics_port: Option<u16>,
    pub node_ip: Option<String>,
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
}

impl NodeInstanceInfo {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            ..Default::default()
        }
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
}
