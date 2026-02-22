use super::{
    bg_tasks::{AgentContext, BgTasksCmds, ImmutableNodeStatus, NodeActionsBatches, NodesMetrics},
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
    /// This is used to determine if the MCP server is active and can be interacted with.
    pub mcp_status: Arc<RwLock<Option<String>>>,
    /// Shared context for the local AI agent (settings, autonomous mode flag, command channel).
    pub agent_ctx: AgentContext,
}

impl AppContext {
    /// Creates a new instance of the application context with the provided database client.
    /// Requires an async context to load agent settings from the database.
    pub async fn new(db_client: DbClient) -> Self {
        let nodes_metrics = Arc::new(RwLock::new(NodesMetrics::new(db_client.clone())));
        let (bg_tasks_cmds_tx, _rx) = broadcast::channel::<BgTasksCmds>(1_000);
        let agent_settings = db_client.get_settings().await;
        let agent_ctx = AgentContext::new(agent_settings);
        Self {
            db_client,
            latest_bin_version: Arc::new(RwLock::new(None)),
            nodes_metrics,
            node_status_locked: ImmutableNodeStatus::default(),
            bg_tasks_cmds_tx,
            node_action_batches: Arc::new(RwLock::new((broadcast::channel(3).0, Vec::new()))),
            stats: Arc::new(RwLock::new(Stats::default())),
            mcp_status: Arc::new(RwLock::new(None)),
            agent_ctx,
        }
    }
}
