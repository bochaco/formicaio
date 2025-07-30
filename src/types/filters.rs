use super::{NodeId, NodeInstanceInfo};

use serde::{Deserialize, Serialize};

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
    /// Optional list of specific node IDs to filter by
    pub node_ids: Option<Vec<NodeId>>,
    /// Optional list of node status filters to match against
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InactiveReason, NodeStatus};

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
