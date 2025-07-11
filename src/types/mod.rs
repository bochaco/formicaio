mod batch;
mod filters;
pub mod metrics;
mod node_instance;
mod node_status;
mod settings;
mod sort_nodes;
mod stats;

pub use batch::{BatchOnMatch, BatchType, NodeOpts, NodesActionsBatch};
pub use filters::{NodeFilter, NodeStatusFilter};
pub use metrics::*;
pub use node_instance::{NodeId, NodeInstanceInfo, NodePid};
pub use node_status::{InactiveReason, NodeStatus};
pub use settings::AppSettings;
pub use sort_nodes::NodesSortStrategy;
pub use stats::{Stats, WidgetFourStats, WidgetStat};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// List of nodes
pub type NodeList = HashMap<String, NodeInstanceInfo>;

/// List of nodes, stats and currently running batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodesInstancesInfo {
    pub latest_bin_version: Option<String>,
    pub nodes: NodeList,
    pub stats: Stats,
    pub scheduled_batches: Vec<NodesActionsBatch>,
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
