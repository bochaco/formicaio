use crate::{
    app::AppContext,
    bg_tasks::{ActionsBatchError, BgTasksCmds, NodesMetrics, prepare_node_action_batch},
    db_client::DbError,
    server_api::parse_and_validate_addr,
    types::{
        BatchType, InactiveReason, NodeFilter, NodeId, NodeInstanceInfo, NodeList, NodeOpts,
        NodeStatus,
    },
};

use super::{
    UPGRADE_NODE_BIN_TIMEOUT_SECS,
    native_nodes::{NativeNodes, NativeNodesError},
};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::Stream;
use leptos::logging;
use semver::Version;
use std::{path::PathBuf, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum NodeManagerError {
    #[error("Invalid rewards address: {0}")]
    RewardsAddressError(String),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    NativeNodeFailure(#[from] NativeNodesError),
    #[error("Failed to request the execution of a background task: {0}")]
    BgTasks(String),
    #[error(
        "Invalid arguments: Cannot set 'node-start-interval' when 'no-auto-start' flag is enabled."
    )]
    InvalidInitArgs,
    #[error(transparent)]
    BgTasksError(#[from] ActionsBatchError),
}

#[derive(Clone, Debug)]
pub struct NodeManager {
    app_ctx: AppContext,
    native_nodes: NativeNodes,
}

impl NodeManager {
    pub async fn new(
        app_ctx: AppContext,
        data_dir_path: Option<PathBuf>,
        no_auto_start: bool,
        node_start_interval: Option<u64>,
    ) -> Result<Self, NodeManagerError> {
        let native_nodes =
            NativeNodes::new(app_ctx.node_status_locked.clone(), data_dir_path).await?;
        let node_manager = Self {
            app_ctx,
            native_nodes,
        };

        // let's make sure we have node binary installed before continuing
        node_manager.upgrade_master_node_binary(None).await?;

        // let's create a batch to start nodes which were Active
        let nodes_in_db = node_manager.app_ctx.db_client.get_nodes_list().await;
        let mut active_nodes = vec![];
        for (node_id, node_info) in nodes_in_db {
            if node_info.status.is_active() {
                // let's set it to inactive otherwise it won't be started
                node_manager
                    .app_ctx
                    .db_client
                    .update_node_status(&node_id, &NodeStatus::Inactive(InactiveReason::Stopped))
                    .await;
                active_nodes.push(node_id);
            } else if node_info.is_status_locked {
                node_manager
                    .app_ctx
                    .db_client
                    .unlock_node_status(&node_id)
                    .await;
            }
        }

        let auto_start_interval = if no_auto_start {
            if node_start_interval.is_some() {
                return Err(NodeManagerError::InvalidInitArgs);
            } else {
                None
            }
        } else {
            node_start_interval.or(Some(5))
        };

        if let Some(node_start_interval) = auto_start_interval
            && !active_nodes.is_empty()
        {
            logging::log!(
                "Auto-starting {} previously active nodes with {node_start_interval} second intervals",
                active_nodes.len()
            );
            let _ = prepare_node_action_batch(
                BatchType::Start(active_nodes),
                node_start_interval,
                &node_manager.app_ctx,
                &node_manager,
            )
            .await?;
        }

        Ok(node_manager)
    }

    pub async fn upgrade_master_node_binary(
        &self,
        version: Option<&Version>,
    ) -> Result<(), NodeManagerError> {
        let v = self
            .native_nodes
            .upgrade_master_node_binary(version)
            .await?;
        *self.app_ctx.latest_bin_version.write().await = Some(v);
        Ok(())
    }

    // Create a node instance
    pub async fn create_node_instance(
        &self,
        node_opts: NodeOpts,
    ) -> Result<NodeInstanceInfo, NodeManagerError> {
        let node_id = NodeId::random();
        logging::log!(
            "Creating new node with listening IP '{}', port {}, and ID {node_id} ...",
            node_opts.node_ip,
            node_opts.port
        );

        let _ = parse_and_validate_addr(&node_opts.rewards_addr)
            .map_err(NodeManagerError::RewardsAddressError)?;

        let node_info = NodeInstanceInfo {
            node_id: node_id.clone(),
            created: Utc::now().timestamp() as u64,
            status: NodeStatus::Inactive(InactiveReason::Created),
            status_changed: Utc::now().timestamp() as u64,
            node_ip: Some(node_opts.node_ip),
            port: Some(node_opts.port),
            metrics_port: Some(node_opts.metrics_port),
            rewards_addr: Some(node_opts.rewards_addr),
            upnp: node_opts.upnp,
            node_logs: node_opts.node_logs,
            data_dir_path: Some(node_opts.data_dir_path.clone()),
            ..Default::default()
        };

        if let Err(err) = self.native_nodes.new_node(&node_info).await {
            logging::error!("[ERROR] Failed to create new node: {err:?}");
            return Err(err.into());
        }

        self.app_ctx
            .db_client
            .insert_node_metadata(&node_info)
            .await;
        logging::log!("New node created successfully with ID: {node_id}");

        if node_opts.auto_start {
            self.start_node_instance(node_id.clone()).await?;
        }

        self.app_ctx
            .bg_tasks_cmds_tx
            .send(BgTasksCmds::CheckBalanceFor(node_info.clone()))
            .map_err(|err| NodeManagerError::BgTasks(err.to_string()))?;

        Ok(node_info)
    }

    // Start a node instance with given id
    pub async fn start_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
        let mut node_info = self
            .app_ctx
            .db_client
            .check_node_is_not_batched(&node_id)
            .await?;
        if node_info.status.is_active() {
            return Ok(());
        }

        logging::log!("Starting node with ID: {node_id} ...");
        self.app_ctx
            .node_status_locked
            .lock(node_id.clone(), Duration::from_secs(20))
            .await;

        node_info.status = NodeStatus::Restarting;
        self.app_ctx
            .db_client
            .update_node_status(&node_id, &node_info.status)
            .await;
        let res = self.native_nodes.spawn_new_node(&mut node_info).await;

        node_info.status = match &res {
            Ok(pid) => {
                self.app_ctx.db_client.update_node_pid(&node_id, *pid).await;
                NodeStatus::Active
            }
            Err(err) => NodeStatus::Inactive(InactiveReason::StartFailed(err.to_string())),
        };

        node_info.set_status_changed_now();
        self.app_ctx
            .db_client
            .update_node_metadata(&node_info, true)
            .await;
        self.app_ctx.node_status_locked.remove(&node_id).await;

        res?;
        Ok(())
    }

    // Stop a node instance with given id
    pub async fn stop_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
        let _ = self
            .app_ctx
            .db_client
            .check_node_is_not_batched(&node_id)
            .await?;

        self.app_ctx
            .node_status_locked
            .lock(node_id.clone(), Duration::from_secs(20))
            .await;
        self.app_ctx
            .db_client
            .update_node_status(&node_id, &NodeStatus::Stopping)
            .await;

        self.native_nodes.kill_node(&node_id).await;

        // set connected/kbucket peers back to 0 and update cache
        let node_info = NodeInstanceInfo {
            node_id: node_id.clone(),
            status: NodeStatus::Inactive(InactiveReason::Stopped),
            status_changed: Utc::now().timestamp() as u64,
            connected_peers: Some(0),
            kbuckets_peers: Some(0),
            records: Some(0),
            ips: Some("".to_string()),
            ..Default::default()
        };

        self.app_ctx
            .db_client
            .update_node_metadata(&node_info, true)
            .await;

        self.app_ctx.node_status_locked.remove(&node_id).await;

        Ok(())
    }

    // Delete a node instance with given id
    pub async fn delete_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
        let mut node_info = NodeInstanceInfo::new(node_id);
        self.app_ctx
            .db_client
            .get_node_metadata(&mut node_info, true)
            .await;
        if node_info.status.is_active() {
            // kill node's process
            self.native_nodes.kill_node(&node_info.node_id).await;
        }

        // remove node's metadata and directory
        self.app_ctx
            .db_client
            .delete_node_metadata(&node_info.node_id)
            .await;
        self.native_nodes.remove_node_dir(&node_info).await;

        self.app_ctx
            .nodes_metrics
            .write()
            .await
            .remove_node_metrics(&node_info.node_id)
            .await;

        self.app_ctx
            .bg_tasks_cmds_tx
            .send(BgTasksCmds::DeleteBalanceFor(node_info))
            .map_err(|err| NodeManagerError::BgTasks(err.to_string()))?;

        Ok(())
    }

    // Upgrade a node instance with given id
    pub async fn upgrade_node_instance(&self, node_id: &NodeId) -> Result<(), NodeManagerError> {
        let mut node_info = self
            .app_ctx
            .db_client
            .check_node_is_not_batched(node_id)
            .await?;

        self.app_ctx
            .node_status_locked
            .lock(
                node_id.clone(),
                Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
            )
            .await;

        node_info.status = NodeStatus::Upgrading;
        self.app_ctx
            .db_client
            .update_node_status(node_id, &node_info.status)
            .await;

        let res = self.native_nodes.upgrade_node(&mut node_info).await;

        node_info.status = match &res {
            Ok(pid) => {
                logging::log!(
                    "Node binary upgraded to v{} in node {node_id}, new PID: {pid}.",
                    node_info.bin_version.as_deref().unwrap_or("[unknown]")
                );
                self.app_ctx.db_client.update_node_pid(node_id, *pid).await;
                NodeStatus::Active
            }
            Err(err) => NodeStatus::Inactive(InactiveReason::StartFailed(err.to_string())),
        };

        node_info.set_status_changed_now();
        self.app_ctx
            .db_client
            .update_node_metadata(&node_info, true)
            .await;
        self.app_ctx.node_status_locked.remove(node_id).await;

        res?;
        Ok(())
    }

    // Recycle a node instance by restarting it with a new node peer-id
    pub async fn recycle_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
        let mut node_info = self
            .app_ctx
            .db_client
            .check_node_is_not_batched(&node_id)
            .await?;

        self.app_ctx
            .node_status_locked
            .lock(node_id.clone(), Duration::from_secs(20))
            .await;

        node_info.status = NodeStatus::Recycling;
        self.app_ctx
            .db_client
            .update_node_status(&node_id, &node_info.status)
            .await;

        let res = self.native_nodes.regenerate_peer_id(&mut node_info).await;

        node_info.status = match &res {
            Ok(pid) => {
                self.app_ctx.db_client.update_node_pid(&node_id, *pid).await;
                NodeStatus::Active
            }
            Err(err) => NodeStatus::Inactive(InactiveReason::StartFailed(err.to_string())),
        };

        node_info.set_status_changed_now();
        self.app_ctx
            .db_client
            .update_node_metadata(&node_info, true)
            .await;
        self.app_ctx.node_status_locked.remove(&node_id).await;

        res?;
        Ok(())
    }

    // Obtain a non-filtered list of existing nodes.
    pub async fn get_nodes_list(&self) -> Result<Vec<NodeInstanceInfo>, NodeManagerError> {
        let nodes_in_db = self.app_ctx.db_client.get_nodes_list().await;
        let nodes = self.native_nodes.get_nodes_list(nodes_in_db).await?;
        Ok(nodes)
    }

    // Obtain a filtered list of existing nodes instances with their up to date info.
    pub async fn filtered_nodes_list(
        &self,
        filter: Option<NodeFilter>,
        nodes_metrics: Arc<RwLock<NodesMetrics>>,
    ) -> Result<NodeList, NodeManagerError> {
        let mut nodes = self.app_ctx.db_client.get_nodes_list().await;
        // TODO: pass the filter/s to the db-client
        if let Some(filter) = filter {
            nodes.retain(|_, info| filter.passes(info));
        }

        for (_, node_info) in nodes.iter_mut() {
            helper_gen_status_info(node_info);
            if node_info.status.is_active() {
                // let's get up to date metrics info
                // which was retrieved through the metrics server
                nodes_metrics.read().await.update_node_info(node_info);
            }
        }

        Ok(nodes)
    }

    // Return a node logs stream.
    pub async fn get_node_logs_stream(
        &self,
        node_id: &NodeId,
    ) -> Result<impl Stream<Item = Result<Bytes, NativeNodesError>> + use<>, NodeManagerError> {
        let mut node_info = NodeInstanceInfo::new(node_id.clone());
        self.app_ctx
            .db_client
            .get_node_metadata(&mut node_info, true)
            .await;

        let stream = self.native_nodes.get_node_logs_stream(&node_info).await?;
        Ok(stream)
    }
}

// Helper to generate a string with additional info about current node's status
fn helper_gen_status_info(node_info: &mut NodeInstanceInfo) {
    let status = &node_info.status;
    let status_info = if status.is_transitioning() {
        "".to_string()
    } else {
        let changed =
            DateTime::<Utc>::from_timestamp(node_info.status_changed as i64, 0).unwrap_or_default();
        let elapsed = Utc::now() - changed;
        let elapsed_str = if elapsed.num_weeks() > 1 {
            format!("{} weeks", elapsed.num_weeks())
        } else if elapsed.num_days() > 1 {
            format!("{} days", elapsed.num_days())
        } else if elapsed.num_hours() > 1 {
            format!("{} hours", elapsed.num_hours())
        } else if elapsed.num_minutes() > 1 {
            format!("{} minutes", elapsed.num_minutes())
        } else if elapsed.num_seconds() > 1 {
            format!("{} seconds", elapsed.num_seconds())
        } else {
            "about a second".to_string()
        };
        if status.is_active() {
            format!("Up {elapsed_str}")
        } else if status.is_inactive() {
            format!("{elapsed_str} ago")
        } else {
            format!("Since {elapsed_str} ago")
        }
    };

    node_info.status_info = status_info;
}
