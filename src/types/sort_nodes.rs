use super::{NodeId, NodeInstanceInfo};

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
pub enum NodeSortField {
    NodeId,
    Status,
    CreationDate,
    PortNumber,
    Rewards,
    ShunnedCount,
    NumRecords,
    NumConnPeers,
    Mem,
    Cpu,
    DiskUsage,
}

impl fmt::Display for NodeSortField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            NodeSortField::NodeId => "Node Id",
            NodeSortField::Status => "Status",
            NodeSortField::CreationDate => "Creation date",
            NodeSortField::PortNumber => "Port number",
            NodeSortField::Rewards => "Rewards count",
            NodeSortField::ShunnedCount => "Shunned count",
            NodeSortField::NumRecords => "Number of records",
            NodeSortField::NumConnPeers => "Connected peers",
            NodeSortField::Mem => "Mem used",
            NodeSortField::Cpu => "CPU usage",
            NodeSortField::DiskUsage => "Disk usage",
        };
        write!(f, "{label}")
    }
}

impl NodeSortField {
    pub fn fields() -> Vec<Self> {
        vec![
            Self::NodeId,
            Self::Status,
            Self::CreationDate,
            Self::PortNumber,
            Self::Rewards,
            Self::ShunnedCount,
            Self::NumRecords,
            Self::NumConnPeers,
            Self::Mem,
            Self::Cpu,
            Self::DiskUsage,
        ]
    }
}

/// Sort strategy for node, inner 'true' value means descending order
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
pub struct NodesSortStrategy {
    pub field: NodeSortField,
    pub is_descending: bool,
}

impl Default for NodesSortStrategy {
    fn default() -> Self {
        Self {
            field: NodeSortField::CreationDate,
            is_descending: true,
        }
    }
}

impl NodesSortStrategy {
    pub fn new(field: NodeSortField, is_descending: bool) -> Self {
        Self {
            field,
            is_descending,
        }
    }

    pub fn variants() -> Vec<Self> {
        vec![
            Self::new(NodeSortField::NodeId, true),
            Self::new(NodeSortField::NodeId, false),
            Self::new(NodeSortField::Status, true),
            Self::new(NodeSortField::Status, false),
            Self::new(NodeSortField::CreationDate, true),
            Self::new(NodeSortField::CreationDate, false),
            Self::new(NodeSortField::PortNumber, true),
            Self::new(NodeSortField::PortNumber, false),
            Self::new(NodeSortField::Rewards, true),
            Self::new(NodeSortField::Rewards, false),
            Self::new(NodeSortField::ShunnedCount, true),
            Self::new(NodeSortField::ShunnedCount, false),
            Self::new(NodeSortField::NumRecords, true),
            Self::new(NodeSortField::NumRecords, false),
            Self::new(NodeSortField::NumConnPeers, true),
            Self::new(NodeSortField::NumConnPeers, false),
            Self::new(NodeSortField::Mem, true),
            Self::new(NodeSortField::Mem, false),
            Self::new(NodeSortField::Cpu, true),
            Self::new(NodeSortField::Cpu, false),
            Self::new(NodeSortField::DiskUsage, true),
            Self::new(NodeSortField::DiskUsage, false),
        ]
    }

    pub fn from_arg_str(str: &str) -> Option<Self> {
        let strategy = match str {
            "node-id" => Self::new(NodeSortField::NodeId, false),
            "node-id-desc" => Self::new(NodeSortField::NodeId, true),
            "status" => Self::new(NodeSortField::Status, false),
            "status-desc" => Self::new(NodeSortField::Status, true),
            "creation" => Self::new(NodeSortField::CreationDate, false),
            "creation-desc" => Self::new(NodeSortField::CreationDate, true),
            "port" => Self::new(NodeSortField::PortNumber, false),
            "port-desc" => Self::new(NodeSortField::PortNumber, true),
            "rewards" => Self::new(NodeSortField::Rewards, false),
            "rewards-desc" => Self::new(NodeSortField::Rewards, true),
            "shunned" => Self::new(NodeSortField::ShunnedCount, false),
            "shunned-desc" => Self::new(NodeSortField::ShunnedCount, true),
            "records" => Self::new(NodeSortField::NumRecords, false),
            "records-desc" => Self::new(NodeSortField::NumRecords, true),
            "conn-peers" => Self::new(NodeSortField::NumConnPeers, false),
            "conn-peers-desc" => Self::new(NodeSortField::NumConnPeers, true),
            "mem" => Self::new(NodeSortField::Mem, false),
            "mem-desc" => Self::new(NodeSortField::Mem, true),
            "cpu" => Self::new(NodeSortField::Cpu, false),
            "cpu-desc" => Self::new(NodeSortField::Cpu, true),
            "disk-usage" => Self::new(NodeSortField::DiskUsage, false),
            "disk-usage-desc" => Self::new(NodeSortField::DiskUsage, true),
            _ => return None,
        };
        Some(strategy)
    }

    pub fn as_arg_str<'a>(&self) -> &'a str {
        match (self.field, self.is_descending) {
            (NodeSortField::NodeId, false) => "node-id",
            (NodeSortField::NodeId, true) => "node-id-desc",
            (NodeSortField::Status, false) => "status",
            (NodeSortField::Status, true) => "status-desc",
            (NodeSortField::CreationDate, false) => "creation",
            (NodeSortField::CreationDate, true) => "creation-desc",
            (NodeSortField::PortNumber, false) => "port",
            (NodeSortField::PortNumber, true) => "port-desc",
            (NodeSortField::Rewards, false) => "rewards",
            (NodeSortField::Rewards, true) => "rewards-desc",
            (NodeSortField::ShunnedCount, false) => "shunned",
            (NodeSortField::ShunnedCount, true) => "shunned-desc",
            (NodeSortField::NumRecords, false) => "records",
            (NodeSortField::NumRecords, true) => "records-desc",
            (NodeSortField::NumConnPeers, false) => "conn-peers",
            (NodeSortField::NumConnPeers, true) => "conn-peers-desc",
            (NodeSortField::Mem, false) => "mem",
            (NodeSortField::Mem, true) => "mem-desc",
            (NodeSortField::Cpu, false) => "cpu",
            (NodeSortField::Cpu, true) => "cpu-desc",
            (NodeSortField::DiskUsage, false) => "disk-usage",
            (NodeSortField::DiskUsage, true) => "disk-usage-desc",
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

    fn cmp_opts_u64(a: Option<u64>, b: Option<u64>) -> Ordering {
        Self::cmp_opts(a.map(|v| v as f64), b.map(|v| v as f64))
    }

    pub fn cmp(&self, a: &NodeInstanceInfo, b: &NodeInstanceInfo) -> Ordering {
        match (self.field, self.is_descending) {
            (NodeSortField::NodeId, false) => a.node_id.cmp(&b.node_id),
            (NodeSortField::NodeId, true) => b.node_id.cmp(&a.node_id),
            (NodeSortField::Status, false) => a.status_summary().cmp(&b.status_summary()),
            (NodeSortField::Status, true) => b.status_summary().cmp(&a.status_summary()),
            (NodeSortField::CreationDate, false) => a.created.cmp(&b.created),
            (NodeSortField::CreationDate, true) => b.created.cmp(&a.created),
            (NodeSortField::PortNumber, false) => a.port.cmp(&b.port),
            (NodeSortField::PortNumber, true) => b.port.cmp(&a.port),
            (NodeSortField::Rewards, false) => a.rewards.cmp(&b.rewards),
            (NodeSortField::Rewards, true) => b.rewards.cmp(&a.rewards),
            (NodeSortField::ShunnedCount, false) => a.shunned_count.cmp(&b.shunned_count),
            (NodeSortField::ShunnedCount, true) => b.shunned_count.cmp(&a.shunned_count),
            (NodeSortField::NumConnPeers, false) => a.connected_peers.cmp(&b.connected_peers),
            (NodeSortField::NumConnPeers, true) => b.connected_peers.cmp(&a.connected_peers),
            (NodeSortField::NumRecords, false) => a.records.cmp(&b.records),
            (NodeSortField::NumRecords, true) => b.records.cmp(&a.records),
            (NodeSortField::Mem, false) => Self::cmp_opts(a.mem_used, b.mem_used),
            (NodeSortField::Mem, true) => Self::cmp_opts(b.mem_used, a.mem_used),
            (NodeSortField::Cpu, false) => Self::cmp_opts(a.cpu_usage, b.cpu_usage),
            (NodeSortField::Cpu, true) => Self::cmp_opts(b.cpu_usage, a.cpu_usage),
            (NodeSortField::DiskUsage, false) => Self::cmp_opts_u64(a.disk_usage, b.disk_usage),
            (NodeSortField::DiskUsage, true) => Self::cmp_opts_u64(b.disk_usage, a.disk_usage),
        }
    }

    pub fn sort_items(&self, items: &mut [NodeInstanceInfo]) {
        items.sort_by(|a, b| self.cmp(a, b));
    }

    pub fn sort_view_items(&self, items: &mut [(NodeId, RwSignal<NodeInstanceInfo>)]) {
        items.sort_by(|a, b| self.cmp(&a.1.read(), &b.1.read()));
    }
}
