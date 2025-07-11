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
    pub node_ids: Option<Vec<NodeId>>,
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
