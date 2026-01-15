use crate::{
    app::AppContext,
    bg_tasks::{BgTasksCmds, NodesMetrics},
    db_client::DbError,
    types::{InactiveReason, NodeFilter, NodeId, NodeInstanceInfo, NodeList, NodeOpts, NodeStatus},
};

use super::{
    UPGRADE_NODE_BIN_TIMEOUT_SECS,
    docker_client::{DockerClient, DockerClientError},
};

use bytes::Bytes;
use chrono::Utc;
use futures_util::Stream;
use leptos::logging;
use semver::Version;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use sysinfo::Disks;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum NodeManagerError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    DockerClient(#[from] DockerClientError),
    #[error("Failed to request the execution of a background task: {0}")]
    BgTasks(String),
}

#[derive(Clone, Debug)]
pub struct NodeManager {
    app_ctx: AppContext,
    docker_client: DockerClient,
    disks: Arc<RwLock<Disks>>,
}

impl NodeManager {
    pub async fn new(app_ctx: AppContext) -> Result<Self, NodeManagerError> {
        Ok(Self {
            app_ctx,
            docker_client: DockerClient::new().await?,
            disks: Arc::new(RwLock::new(Disks::new())),
        })
    }

    pub async fn upgrade_master_node_binary(
        &self,
        version: Option<&Version>,
    ) -> Result<(), NodeManagerError> {
        logging::log!("Pulling Formica node image from registry ...");
        if let Err(err) = self.docker_client.pull_formica_image().await {
            logging::error!("[ERROR] Failed to pull node image: {err}");
            return Err(err.into());
        }
        *self.app_ctx.latest_bin_version.write().await = version.cloned();
        Ok(())
    }

    // Create a node instance
    pub async fn create_node_instance(
        &self,
        node_opts: NodeOpts,
    ) -> Result<NodeInstanceInfo, NodeManagerError> {
        logging::log!("Creating new node with port {} ...", node_opts.port);
        let auto_start = node_opts.auto_start;
        let node_id = self.docker_client.create_new_container(node_opts).await?;
        logging::log!("New node ID: {node_id} ...");

        let mut node_info = self.docker_client.get_container_info(&node_id).await?;
        logging::log!("New node created: {node_info:?}");

        self.app_ctx
            .db_client
            .insert_node_metadata(&node_info)
            .await;

        if auto_start {
            self.start_node_instance(node_id.clone()).await?;
            node_info = self.docker_client.get_container_info(&node_id).await?;
        }

        self.app_ctx
            .bg_tasks_cmds_tx
            .send(BgTasksCmds::CheckBalanceFor(node_info.clone()))
            .map_err(|err| NodeManagerError::BgTasks(err.to_string()))?;

        Ok(node_info)
    }

    // Start a node instance with given id
    pub async fn start_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
        let _ = self
            .app_ctx
            .db_client
            .check_node_is_not_batched(&node_id)
            .await?;

        logging::log!("Starting node with ID: {node_id} ...");

        self.app_ctx
            .db_client
            .update_node_status(&node_id, &NodeStatus::Restarting)
            .await;

        let (bin_version, peer_id, ips) =
            self.docker_client.start_container(&node_id, true).await?;

        let node_info = NodeInstanceInfo {
            node_id,
            status_changed: Utc::now().timestamp() as u64,
            bin_version: Some(bin_version.clone().unwrap_or_default()),
            peer_id: Some(peer_id.unwrap_or_default()),
            ips: Some(ips.unwrap_or_default()),
            ..Default::default()
        };

        self.app_ctx
            .db_client
            .update_node_metadata(&node_info, false)
            .await;

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

        let res = self.docker_client.stop_container(&node_id).await;

        if matches!(res, Ok(())) {
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
        }

        self.app_ctx.node_status_locked.remove(&node_id).await;

        Ok(res?)
    }

    // Delete a node instance with given id
    pub async fn delete_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
        let node_info = self.docker_client.get_container_info(&node_id).await?;
        self.docker_client.delete_container(&node_id).await?;
        self.app_ctx.db_client.delete_node_metadata(&node_id).await;
        self.app_ctx
            .nodes_metrics
            .write()
            .await
            .remove_node_metrics(&node_id)
            .await;

        self.app_ctx
            .bg_tasks_cmds_tx
            .send(BgTasksCmds::DeleteBalanceFor(node_info))
            .map_err(|err| NodeManagerError::BgTasks(err.to_string()))?;

        Ok(())
    }

    // Upgrade a node instance with given id
    pub async fn upgrade_node_instance(&self, node_id: &NodeId) -> Result<(), NodeManagerError> {
        let _ = self
            .app_ctx
            .db_client
            .check_node_is_not_batched(node_id)
            .await?;

        // TODO: use docker 'extract' api to simply copy the new node binary into the container.
        self.app_ctx
            .node_status_locked
            .lock(
                node_id.clone(),
                Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS),
            )
            .await;
        self.app_ctx
            .db_client
            .update_node_status(node_id, &NodeStatus::Upgrading)
            .await;

        let res = self
            .docker_client
            .upgrade_node_in_container(node_id, true)
            .await;

        if let Ok((ref new_version, ref ips)) = res {
            logging::log!(
                "Node binary upgraded to v{} in node {node_id}.",
                new_version.as_deref().unwrap_or("[unknown]")
            );

            // set bin_version to new version obtained
            let node_info = NodeInstanceInfo {
                node_id: node_id.clone(),
                status: NodeStatus::Upgrading,
                status_changed: Utc::now().timestamp() as u64,
                bin_version: Some(new_version.clone().unwrap_or_default()),
                ips: Some(ips.clone().unwrap_or_default()),
                ..Default::default()
            };

            self.app_ctx
                .db_client
                .update_node_metadata(&node_info, true)
                .await;
        }

        self.app_ctx.node_status_locked.remove(node_id).await;

        let _ = res?;

        Ok(())
    }

    // Recycle a node instance by restarting it with a new node peer-id
    pub async fn recycle_node_instance(&self, node_id: NodeId) -> Result<(), NodeManagerError> {
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
            .update_node_status(&node_id, &NodeStatus::Recycling)
            .await;

        let (bin_version, peer_id, ips) = self
            .docker_client
            .regenerate_peer_id_in_container(&node_id, true)
            .await?;

        let node_info = NodeInstanceInfo {
            node_id: node_id.clone(),
            status_changed: Utc::now().timestamp() as u64,
            bin_version: Some(bin_version.clone().unwrap_or_default()),
            peer_id: Some(peer_id.unwrap_or_default()),
            ips: Some(ips.unwrap_or_default()),
            ..Default::default()
        };

        self.app_ctx
            .db_client
            .update_node_metadata(&node_info, false)
            .await;

        self.app_ctx.node_status_locked.remove(&node_id).await;

        Ok(())
    }

    // Obtain a non-filtered list of existing nodes.
    pub async fn get_nodes_list(&self) -> Result<Vec<NodeInstanceInfo>, NodeManagerError> {
        let nodes = self.docker_client.get_containers_list().await?;
        Ok(nodes)
    }

    // Obtain a filtered list of existing nodes instances with their up to date info.
    pub async fn filtered_nodes_list(
        &self,
        filter: Option<NodeFilter>,
        nodes_metrics: Arc<RwLock<NodesMetrics>>,
    ) -> Result<NodeList, NodeManagerError> {
        let nodes_list = self.docker_client.get_containers_list().await?;
        let mut nodes = HashMap::new();
        for mut node_info in nodes_list.into_iter() {
            // we first read node metadata cached in the database
            // TODO: fetch metadata of all nodes from DB with a single DB call
            self.app_ctx
                .db_client
                .get_node_metadata(&mut node_info, false)
                .await;

            // TODO: pass the filter/s to docker-client
            if let Some(ref f) = filter
                && !f.passes(&node_info)
            {
                continue;
            }

            // if the node is Active, let's also get up to date metrics
            // info that was retrieved through the metrics server
            if node_info.status.is_active() {
                nodes_metrics.read().await.update_node_info(&mut node_info);
            }

            nodes.insert(node_info.node_id.clone(), node_info);
        }

        Ok(nodes)
    }

    // Return a node logs stream.
    pub async fn get_node_logs_stream(
        &self,
        node_id: &NodeId,
    ) -> Result<impl Stream<Item = Result<Bytes, DockerClientError>> + use<>, NodeManagerError>
    {
        let stream = self
            .docker_client
            .get_container_logs_stream(node_id)
            .await?;
        Ok(stream)
    }

    // Get node data dir based on node-mgr root dir and node custom data dir if set
    pub async fn get_node_data_dir(&self, node_info: &NodeInstanceInfo) -> PathBuf {
        self.docker_client
            .get_storage_mount_point(&node_info.node_id)
            .await
            .unwrap_or_default()
    }

    // Get the total and free space of only the mount points where nodes are storing data,
    // i.e. ignore all other mount points which are not being used by nodes to store data.
    pub async fn get_disks_usage(&self, base_paths: HashSet<PathBuf>) -> (u64, u64) {
        let mut disks = self.disks.write().await;
        super::get_disks_usage(&mut disks, base_paths).await
    }
}
