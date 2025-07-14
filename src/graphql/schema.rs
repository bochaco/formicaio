use crate::app::ServerGlobalState;
use crate::types::{NodeId, NodeInstanceInfo, NodeMetric, Stats};
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::resolvers::Query;

pub type MetricsSchema = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn create_schema(state: Arc<ServerGlobalState>) -> MetricsSchema {
    Schema::build(Query::new(state), EmptyMutation, EmptySubscription).finish()
}

#[derive(async_graphql::SimpleObject)]
pub struct NodeMetricType {
    pub key: String,
    pub value: String,
    pub timestamp: i64,
}

impl From<NodeMetric> for NodeMetricType {
    fn from(metric: NodeMetric) -> Self {
        Self {
            key: metric.key,
            value: metric.value,
            timestamp: metric.timestamp,
        }
    }
}

#[derive(async_graphql::SimpleObject)]
pub struct NodeInstanceInfoType {
    pub node_id: String,
    pub pid: Option<u32>,
    pub created: u64,
    pub status_changed: u64,
    pub status: String,
    pub is_status_locked: bool,
    pub is_status_unknown: bool,
    pub peer_id: Option<String>,
    pub status_info: String,
    pub bin_version: Option<String>,
    pub port: Option<u16>,
    pub metrics_port: Option<u16>,
    pub node_ip: Option<String>,
    pub balance: Option<String>,
    pub rewards_addr: Option<String>,
    pub home_network: bool,
    pub upnp: bool,
    pub node_logs: bool,
    pub rewards: Option<String>,
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

impl From<NodeInstanceInfo> for NodeInstanceInfoType {
    fn from(info: NodeInstanceInfo) -> Self {
        Self {
            node_id: info.node_id,
            pid: info.pid,
            created: info.created,
            status_changed: info.status_changed,
            status: info.status.to_string(),
            is_status_locked: info.is_status_locked,
            is_status_unknown: info.is_status_unknown,
            peer_id: info.peer_id,
            status_info: info.status_info,
            bin_version: info.bin_version,
            port: info.port,
            metrics_port: info.metrics_port,
            node_ip: info.node_ip.map(|ip| ip.to_string()),
            balance: info.balance.map(|b| b.to_string()),
            rewards_addr: info.rewards_addr,
            home_network: info.home_network,
            upnp: info.upnp,
            node_logs: info.node_logs,
            rewards: info.rewards.map(|r| r.to_string()),
            records: info.records,
            relevant_records: info.relevant_records,
            mem_used: info.mem_used,
            cpu_usage: info.cpu_usage,
            connected_peers: info.connected_peers,
            connected_relay_clients: info.connected_relay_clients,
            kbuckets_peers: info.kbuckets_peers,
            shunned_count: info.shunned_count,
            net_size: info.net_size,
            ips: info.ips,
        }
    }
}

#[derive(async_graphql::SimpleObject)]
pub struct StatsType {
    pub total_balance: String,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub inactive_nodes: usize,
    pub connected_peers: usize,
    pub shunned_count: usize,
    pub estimated_net_size: usize,
    pub stored_records: usize,
    pub relevant_records: usize,
}

impl From<Stats> for StatsType {
    fn from(stats: Stats) -> Self {
        Self {
            total_balance: stats.total_balance.to_string(),
            total_nodes: stats.total_nodes,
            active_nodes: stats.active_nodes,
            inactive_nodes: stats.inactive_nodes,
            connected_peers: stats.connected_peers,
            shunned_count: stats.shunned_count,
            estimated_net_size: stats.estimated_net_size,
            stored_records: stats.stored_records,
            relevant_records: stats.relevant_records,
        }
    }
}

#[derive(async_graphql::InputObject)]
pub struct MetricsFilter {
    pub since: Option<i64>,
}

#[derive(async_graphql::InputObject)]
pub struct NodeFilter {
    pub status: Option<String>,
    pub home_network: Option<bool>,
    pub upnp: Option<bool>,
    pub node_ids: Option<Vec<String>>,
}
