use super::{
    bg_tasks::{BgTasksCmds, ImmutableNodeStatus, NodeActionsBatches, NodesMetrics},
    db_client::DbClient,
    types::Stats,
};

use axum::extract::FromRef;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// Main application context holding shared state for the backend/server side.
/// Contains global, thread-safe resources and channels used throughout the application.
#[derive(Clone, FromRef, Debug)]
pub struct AppContext {
    /// Database client for accessing persistent storage.
    pub db_client: DbClient,
    /// The latest available version of the node binary (if any), shared and mutable across threads.
    pub latest_bin_version: Arc<RwLock<Option<semver::Version>>>,
    /// Metrics data for all nodes, shared and mutable across threads.
    pub nodes_metrics: Arc<RwLock<NodesMetrics>>,
    /// The current locked status of all nodes, used to prevent concurrent and undesirable status changes.
    pub node_status_locked: ImmutableNodeStatus,
    /// Channel for sending task commands to the background tasks handler.
    pub bg_tasks_cmds_tx: broadcast::Sender<BgTasksCmds>,
    /// Batches of node actions currently being processed or scheduled.
    pub node_action_batches: NodeActionsBatches,
    /// Global statistics of all the node instances, shared and mutable across threads.
    pub stats: Arc<RwLock<Stats>>,
}
