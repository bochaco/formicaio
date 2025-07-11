use super::{NodeId, NodeInstanceInfo};

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Sort strategy for node, inner 'true' value means descending order
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
pub enum NodesSortStrategy {
    NodeId(bool),
    Status(bool),
    CreationDate(bool),
    PortNumber(bool),
    Rewards(bool),
    ShunnedCount(bool),
    NumRecords(bool),
    NumConnPeers(bool),
    Mem(bool),
    Cpu(bool),
}

impl std::fmt::Display for NodesSortStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let text = match self {
            Self::NodeId(true) => "node id ↓",
            Self::NodeId(false) => "node id ↑",
            Self::Status(true) => "status ↓",
            Self::Status(false) => "status ↑",
            Self::CreationDate(true) => "creation date ↓",
            Self::CreationDate(false) => "creation date ↑",
            Self::PortNumber(true) => "port number ↓",
            Self::PortNumber(false) => "port number ↑",
            Self::Rewards(true) => "rewards count ↓",
            Self::Rewards(false) => "rewards count ↑",
            Self::ShunnedCount(true) => "shunned count ↓",
            Self::ShunnedCount(false) => "shunned count ↑",
            Self::NumRecords(true) => "number of records ↓",
            Self::NumRecords(false) => "number of records ↑",
            Self::NumConnPeers(true) => "connected peers ↓",
            Self::NumConnPeers(false) => "connected peers ↑",
            Self::Mem(true) => "mem used ↓",
            Self::Mem(false) => "mem used ↑",
            Self::Cpu(true) => "CPU usage ↓",
            Self::Cpu(false) => "CPU usage ↑",
        };
        write!(f, "{text}")
    }
}

impl NodesSortStrategy {
    pub fn variants() -> Vec<Self> {
        vec![
            Self::NodeId(true),
            Self::NodeId(false),
            Self::Status(true),
            Self::Status(false),
            Self::CreationDate(true),
            Self::CreationDate(false),
            Self::PortNumber(true),
            Self::PortNumber(false),
            Self::Rewards(true),
            Self::Rewards(false),
            Self::ShunnedCount(true),
            Self::ShunnedCount(false),
            Self::NumRecords(true),
            Self::NumRecords(false),
            Self::NumConnPeers(true),
            Self::NumConnPeers(false),
            Self::Mem(true),
            Self::Mem(false),
            Self::Cpu(true),
            Self::Cpu(false),
        ]
    }

    pub fn from_arg_str(str: &str) -> Option<Self> {
        match str {
            "node-id" => Some(Self::NodeId(false)),
            "node-id-desc" => Some(Self::NodeId(true)),
            "status" => Some(Self::Status(false)),
            "status-desc" => Some(Self::Status(true)),
            "creation" => Some(Self::CreationDate(false)),
            "creation-desc" => Some(Self::CreationDate(true)),
            "port" => Some(Self::PortNumber(false)),
            "port-desc" => Some(Self::PortNumber(true)),
            "rewards" => Some(Self::Rewards(false)),
            "rewards-desc" => Some(Self::Rewards(true)),
            "shunned" => Some(Self::ShunnedCount(false)),
            "shunned-desc" => Some(Self::ShunnedCount(true)),
            "records" => Some(Self::NumRecords(false)),
            "records-desc" => Some(Self::NumRecords(true)),
            "conn-peers" => Some(Self::NumConnPeers(false)),
            "conn-peers-desc" => Some(Self::NumConnPeers(true)),
            "mem" => Some(Self::Mem(false)),
            "mem-desc" => Some(Self::Mem(true)),
            "cpu" => Some(Self::Cpu(false)),
            "cpu-desc" => Some(Self::Cpu(true)),
            _ => None,
        }
    }

    pub fn as_arg_str<'a>(&self) -> &'a str {
        match self {
            Self::NodeId(false) => "node-id",
            Self::NodeId(true) => "node-id-desc",
            Self::Status(false) => "status",
            Self::Status(true) => "status-desc",
            Self::CreationDate(false) => "creation",
            Self::CreationDate(true) => "creation-desc",
            Self::PortNumber(false) => "port",
            Self::PortNumber(true) => "port-desc",
            Self::Rewards(false) => "rewards",
            Self::Rewards(true) => "rewards-desc",
            Self::ShunnedCount(false) => "shunned",
            Self::ShunnedCount(true) => "shunned-desc",
            Self::NumRecords(false) => "records",
            Self::NumRecords(true) => "records-desc",
            Self::NumConnPeers(false) => "conn-peers",
            Self::NumConnPeers(true) => "conn-peers-desc",
            Self::Mem(false) => "mem",
            Self::Mem(true) => "mem-desc",
            Self::Cpu(false) => "cpu",
            Self::Cpu(true) => "cpu-desc",
        }
    }

    fn cmp_opts(a: Option<f64>, b: Option<f64>) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => {
                if a > b {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        }
    }

    pub fn cmp(&self, a: &NodeInstanceInfo, b: &NodeInstanceInfo) -> Ordering {
        match self {
            Self::NodeId(false) => a.node_id.cmp(&b.node_id),
            Self::NodeId(true) => b.node_id.cmp(&a.node_id),
            Self::Status(false) => a.status_summary().cmp(&b.status_summary()),
            Self::Status(true) => b.status_summary().cmp(&a.status_summary()),
            Self::CreationDate(false) => a.created.cmp(&b.created),
            Self::CreationDate(true) => b.created.cmp(&a.created),
            Self::PortNumber(false) => a.port.cmp(&b.port),
            Self::PortNumber(true) => b.port.cmp(&a.port),
            Self::Rewards(false) => a.rewards.cmp(&b.rewards),
            Self::Rewards(true) => b.rewards.cmp(&a.rewards),
            Self::ShunnedCount(false) => a.shunned_count.cmp(&b.shunned_count),
            Self::ShunnedCount(true) => b.shunned_count.cmp(&a.shunned_count),
            Self::NumConnPeers(false) => a.connected_peers.cmp(&b.connected_peers),
            Self::NumConnPeers(true) => b.connected_peers.cmp(&a.connected_peers),
            Self::NumRecords(false) => a.records.cmp(&b.records),
            Self::NumRecords(true) => b.records.cmp(&a.records),
            Self::Mem(false) => Self::cmp_opts(a.mem_used, b.mem_used),
            Self::Mem(true) => Self::cmp_opts(b.mem_used, a.mem_used),
            Self::Cpu(false) => Self::cmp_opts(a.cpu_usage, b.cpu_usage),
            Self::Cpu(true) => Self::cmp_opts(b.cpu_usage, a.cpu_usage),
        }
    }

    pub fn sort_items(&self, items: &mut [NodeInstanceInfo]) {
        items.sort_by(|a, b| self.cmp(a, b));
    }

    pub fn sort_view_items(&self, items: &mut [(NodeId, RwSignal<NodeInstanceInfo>)]) {
        items.sort_by(|a, b| self.cmp(&a.1.read(), &b.1.read()));
    }
}
