use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub enum NodeStatus {
    #[default]
    Creating,
    // A running node connected to peers on the network is considered Active.
    Active,
    Restarting,
    Stopping,
    // A node not connected to any peer on the network is considered Inactive.
    Inactive(InactiveReason),
    Removing,
    Upgrading,
    // The node's peer-id is cleared and restarted with a fresh new one
    Recycling,
}

#[derive(Clone, Default, Debug, Deserialize, PartialEq, Serialize)]
pub enum InactiveReason {
    // The node was just created and was never active yet.
    #[default]
    Created,
    // The node was stopped by the user, or the app was stopped altogether.
    Stopped,
    // A node which failed when attempting to start running.
    StartFailed(String),
    // A node which was active and exited for some reason.
    Exited(String),
    // The node was found inactive but it's unknown why.
    Unknown,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Inactive(InactiveReason::Created) => write!(f, "Created"),
            Self::Inactive(InactiveReason::Stopped) => write!(f, "Stopped"),
            Self::Inactive(InactiveReason::StartFailed(reason)) => {
                write!(f, "Start failed ({reason})")
            }
            Self::Inactive(InactiveReason::Exited(reason)) => write!(f, "Exited ({reason})"),
            Self::Inactive(InactiveReason::Unknown) => write!(f, "Exited (unknown reason)"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl NodeStatus {
    pub fn is_creating(&self) -> bool {
        matches!(self, Self::Creating)
    }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
    pub fn is_restarting(&self) -> bool {
        matches!(self, Self::Restarting)
    }
    pub fn is_stopping(&self) -> bool {
        matches!(self, Self::Stopping)
    }
    pub fn is_removing(&self) -> bool {
        matches!(self, Self::Removing)
    }
    pub fn is_inactive(&self) -> bool {
        matches!(self, Self::Inactive(_))
    }
    pub fn is_created(&self) -> bool {
        matches!(self, Self::Inactive(InactiveReason::Created))
    }
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Inactive(InactiveReason::Stopped))
    }
    pub fn is_start_failed(&self) -> bool {
        matches!(self, Self::Inactive(InactiveReason::StartFailed(_)))
    }
    pub fn is_exited(&self) -> bool {
        matches!(self, Self::Inactive(InactiveReason::Exited(_)))
    }
    pub fn is_inactive_unknown(&self) -> bool {
        matches!(self, Self::Inactive(InactiveReason::Unknown))
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
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_status_variants_and_methods() {
        let creating = NodeStatus::Creating;
        let active = NodeStatus::Active;
        let restarting = NodeStatus::Restarting;
        let stopping = NodeStatus::Stopping;
        let removing = NodeStatus::Removing;
        let upgrading = NodeStatus::Upgrading;
        let recycling = NodeStatus::Recycling;
        let inactive_created = NodeStatus::Inactive(InactiveReason::Created);
        let inactive_stopped = NodeStatus::Inactive(InactiveReason::Stopped);
        let inactive_start_failed =
            NodeStatus::Inactive(InactiveReason::StartFailed("fail".to_string()));
        let inactive_exited = NodeStatus::Inactive(InactiveReason::Exited("bye".to_string()));
        let inactive_unknown = NodeStatus::Inactive(InactiveReason::Unknown);

        assert!(creating.is_creating());
        assert!(active.is_active());
        assert!(inactive_created.is_inactive());
        assert!(inactive_exited.is_exited());
        assert!(recycling.is_recycling());
        assert!(upgrading.is_upgrading());
        assert!(restarting.is_transitioning());
        assert!(removing.is_transitioning());
        assert!(stopping.is_transitioning());
        assert!(upgrading.is_transitioning());
        assert!(recycling.is_transitioning());
        assert!(!active.is_transitioning());
        assert_eq!(inactive_created.to_string(), "Created");
        assert_eq!(inactive_stopped.to_string(), "Stopped");
        assert_eq!(inactive_start_failed.to_string(), "Start failed (fail)");
        assert_eq!(inactive_exited.to_string(), "Exited (bye)");
        assert_eq!(inactive_unknown.to_string(), "Exited (unknown reason)");
    }

    #[test]
    fn test_node_status_default() {
        // Test that Creating is the default
        let default_status: NodeStatus = Default::default();
        assert!(default_status.is_creating());
        assert_eq!(default_status, NodeStatus::Creating);
    }

    #[test]
    fn test_inactive_reason_default() {
        // Test that Created is the default
        let default_reason: InactiveReason = Default::default();
        assert!(matches!(default_reason, InactiveReason::Created));
    }

    #[test]
    fn test_inactive_status_variants() {
        // Test all inactive status variants
        let inactive_statuses = vec![
            NodeStatus::Inactive(InactiveReason::Created),
            NodeStatus::Inactive(InactiveReason::Stopped),
            NodeStatus::Inactive(InactiveReason::StartFailed("test".to_string())),
            NodeStatus::Inactive(InactiveReason::Exited("test".to_string())),
            NodeStatus::Inactive(InactiveReason::Unknown),
        ];

        for status in inactive_statuses {
            assert!(
                status.is_inactive(),
                "Status {:?} should be inactive",
                status
            );
        }

        // Test that non-inactive statuses are not marked as inactive
        let non_inactive_statuses = vec![
            NodeStatus::Creating,
            NodeStatus::Active,
            NodeStatus::Restarting,
            NodeStatus::Stopping,
            NodeStatus::Removing,
            NodeStatus::Upgrading,
            NodeStatus::Recycling,
        ];

        for status in non_inactive_statuses {
            assert!(
                !status.is_inactive(),
                "Status {:?} should not be inactive",
                status
            );
        }
    }
}
