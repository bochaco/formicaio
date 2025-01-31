use super::{app::ClientGlobalState, server_api_types::NodeOpts};

use alloy_primitives::U256;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

// Length of nodes PeerIds' prefix and suffix to be displayed
const PEER_ID_PREFIX_SUFFIX_LEN: usize = 12;
// Length of nodes Docker container ids' prefix to be displayed
const CONTAINER_ID_PREFIX_LEN: usize = 12;
// Length of nodes rewards address' prefix and suffix to be displayed
const REWARDS_ADDR_PREFIX_SUFFIX_LEN: usize = 8;

// Hex-encoded container id
pub type ContainerId = String;
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
    // When a node is running and connected to peers on the network but it's
    // being considered a bad node by them, then this node is considered Shunned.
    Shunned,
    Removing,
    Upgrading,
    // The node's peer-id is cleared and restarted with a fresh new one
    Recycling,
    // This is a special state just to provide a good UX, after going thru some status
    // change, e.g. Restarting, Upgrading, we set to this state till we get actual state
    // from the server during our polling cycle. The string describes the type of transition.
    Transitioned(String),
}

impl NodeStatus {
    pub fn is_creating(&self) -> bool {
        matches!(self, Self::Creating)
    }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
    pub fn is_inactive(&self) -> bool {
        matches!(self, Self::Inactive)
    }
    pub fn is_shunned(&self) -> bool {
        matches!(self, Self::Shunned)
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
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Transitioned(s) => write!(f, "{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub struct NodeInstanceInfo {
    pub container_id: ContainerId,
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
    pub node_logs: bool,
    pub rewards: Option<U256>,
    pub records: Option<usize>,
    pub relevant_records: Option<usize>,
    pub mem_used: Option<f64>,
    pub cpu_usage: Option<f64>,
    pub connected_peers: Option<usize>,
    pub kbuckets_peers: Option<usize>,
    pub shunned_count: Option<usize>,
    pub net_size: Option<usize>,
    pub ips: Option<String>,
}

impl NodeInstanceInfo {
    pub fn new(container_id: String) -> Self {
        Self {
            container_id,
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

    pub fn short_container_id(&self) -> String {
        if self.container_id.len() > CONTAINER_ID_PREFIX_LEN {
            self.container_id[..CONTAINER_ID_PREFIX_LEN].to_string()
        } else {
            self.container_id.clone()
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

// Information of a batch of node intances creation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInstancesBatch {
    pub node_opts: NodeOpts,
    pub created: u16,
    pub total: u16,
    pub interval_secs: u64,
}
