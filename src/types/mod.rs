mod batch;
mod filters;
pub mod metrics;
mod node_id;
mod node_instance;
mod node_status;
mod settings;
mod sort_nodes;
mod stats;

pub use batch::{BatchOnMatch, BatchStatus, BatchType, NodeOpts, NodesActionsBatch};
pub use filters::{NodeFilter, NodeStatusFilter};
pub use metrics::*;
pub use node_id::NodeId;
pub use node_instance::{NodeInstanceInfo, NodePid, ReachabilityCheckStatus, shortened_address};
pub use node_status::{InactiveReason, NodeStatus};
pub use settings::AppSettings;
pub use sort_nodes::{NodeSortField, NodesSortStrategy};
pub use stats::{EarningsStats, PeriodStats, Stats, WidgetFourStats, WidgetStat};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// List of nodes keyed by node ID
pub type NodeList = HashMap<NodeId, NodeInstanceInfo>;

/// List of nodes, stats and currently running batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodesInstancesInfo {
    /// Latest available version of the node binary
    pub latest_bin_version: Option<String>,
    /// HashMap of all node instances, keyed by node ID
    pub nodes: NodeList,
    /// Aggregated statistics across all nodes
    pub stats: Stats,
    /// List of scheduled batch operations for nodes
    pub scheduled_batches: Vec<NodesActionsBatch>,
}
