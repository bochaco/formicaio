#[cfg(not(feature = "native"))]
use super::docker_client::{DockerClient, DockerClientError};
#[cfg(feature = "native")]
use super::node_manager::{NodeManager, NodeManagerError};

use super::{
    app::ImmutableNodeStatus,
    db_client::DbClient,
    node_instance::{NodeId, NodeInstanceInfo},
    server_api::helper_upgrade_node_instance,
    server_api_types::AppSettings,
};
use leptos::logging;
use semver::Version;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    time::{interval, Duration, Interval},
};

// How often to perform a metrics pruning in the DB.
const METRICS_PRUNING_FREQ: Duration = Duration::from_secs(60 * 60); // every hour.

// Frequency to pull a new version of the formica image.
const FORMICA_IMAGE_PULLING_FREQ: Duration = Duration::from_secs(60 * 60 * 6); // every 6 hours.

// Frequency to poll node status from Docker engine
const NODE_STATUS_POLLING_FREQ: Duration = Duration::from_secs(5);

// App settings and set of intervals used to schedule each of the tasks.
pub struct TasksContext {
    pub formica_image_pulling: Interval,
    pub node_bin_version_check: Interval,
    pub balances_retrieval: Interval,
    pub metrics_pruning: Interval,
    pub nodes_metrics_polling: Interval,
    pub nodes_status_polling: Interval,
    pub app_settings: AppSettings,
}

impl TasksContext {
    pub fn from(settings: AppSettings) -> Self {
        let mut balances_retrieval = interval(settings.rewards_balances_retrieval_freq);
        balances_retrieval.reset(); // the task will trigger the first check by itself

        Self {
            formica_image_pulling: interval(FORMICA_IMAGE_PULLING_FREQ),
            node_bin_version_check: interval(settings.node_bin_version_polling_freq),
            balances_retrieval,
            metrics_pruning: interval(METRICS_PRUNING_FREQ),
            nodes_metrics_polling: interval(settings.nodes_metrics_polling_freq),
            nodes_status_polling: interval(NODE_STATUS_POLLING_FREQ),
            app_settings: settings,
        }
    }

    pub fn apply_settings(&mut self, settings: AppSettings) {
        logging::log!("Applying new settings values immediataly to bg tasks: {settings:#?}");

        // helper to create a new interval only if new period differs from current
        let update_interval = |target: &mut Interval, new_period: Duration| {
            let curr_period = target.period();
            if new_period != curr_period {
                *target = interval(new_period);
                // reset interval to start next period from this instant
                target.reset();
            }
        };

        update_interval(
            &mut self.node_bin_version_check,
            settings.node_bin_version_polling_freq,
        );
        update_interval(
            &mut self.balances_retrieval,
            settings.rewards_balances_retrieval_freq,
        );
        update_interval(
            &mut self.nodes_metrics_polling,
            settings.nodes_metrics_polling_freq,
        );
        self.app_settings = settings;
    }
}

#[derive(Clone)]
pub struct NodeManagerProxy {
    pub db_client: DbClient,
    #[cfg(not(feature = "native"))]
    pub docker_client: DockerClient,
    #[cfg(feature = "native")]
    pub node_manager: NodeManager,
}

#[cfg(not(feature = "native"))]
impl NodeManagerProxy {
    pub async fn get_nodes_list(
        &self,
        all: bool,
    ) -> Result<Vec<NodeInstanceInfo>, DockerClientError> {
        self.docker_client.get_containers_list(all).await
    }

    pub async fn upgrade_node_instance(
        &self,
        node_id: &NodeId,
        node_status_locked: &ImmutableNodeStatus,
    ) {
        if let Err(err) = helper_upgrade_node_instance(
            node_id,
            node_status_locked,
            &self.db_client,
            &self.docker_client,
        )
        .await
        {
            logging::log!(
                "Failed to auto-upgrade node binary for node instance {node_id}: {err:?}."
            );
        }
    }

    pub async fn pull_formica_image(&self) -> Result<(), DockerClientError> {
        logging::log!("Pulling formica node image ...");
        self.docker_client.pull_formica_image().await
    }

    pub async fn upgrade_master_node_binary(
        &self,
        version: &Version,
        latest_bin_version: Arc<Mutex<Option<Version>>>,
    ) {
        *latest_bin_version.lock().await = Some(version.clone());
    }
}

#[cfg(feature = "native")]
impl NodeManagerProxy {
    pub async fn get_nodes_list(
        &self,
        all: bool,
    ) -> Result<Vec<NodeInstanceInfo>, NodeManagerError> {
        use super::node_instance::InactiveReason;

        let mut active_nodes = self.node_manager.get_active_nodes_list().await?;
        let nodes_in_db = self.db_client.get_nodes_list().await;

        let nodes = nodes_in_db
            .into_iter()
            .filter_map(|(_, mut node_info)| {
                match node_info.pid.map(|pid| active_nodes.remove(&pid)) {
                    None => { /*it has no pid*/ },
                    Some(None) if node_info.status.is_inactive() => { /*we already know it's inactive*/ },
                    Some(None) => node_info.set_status_inactive(InactiveReason::Unknown), // it died/exited
                    Some(Some(None)) => node_info.set_status_active(), // it was found active
                    Some(Some(Some(reason))) => node_info.set_status_inactive(reason), // it was found dead
                }

                if all || node_info.status.is_active() {
                    Some(node_info)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // TODO: what if there are active PIDs not found in DB...
        // ...populate them in DB so the user can see/delete them...?

        Ok(nodes)
    }

    pub async fn upgrade_node_instance(
        &self,
        node_id: &NodeId,
        node_status_locked: &ImmutableNodeStatus,
    ) {
        if let Err(err) = helper_upgrade_node_instance(
            node_id,
            node_status_locked,
            &self.db_client,
            &self.node_manager,
        )
        .await
        {
            logging::log!(
                "Failed to auto-upgrade node binary for node instance {node_id}: {err:?}."
            );
        }
    }

    pub async fn pull_formica_image(&self) -> Result<(), NodeManagerError> {
        Ok(())
    }

    pub async fn upgrade_master_node_binary(
        &self,
        version: &Version,
        latest_bin_version: Arc<Mutex<Option<Version>>>,
    ) {
        match self
            .node_manager
            .upgrade_master_node_binary(Some(version))
            .await
        {
            Ok(v) => *latest_bin_version.lock().await = Some(v),
            Err(err) => {
                logging::error!("Failed to download v{version} of node binary: {err:?}")
            }
        }
    }
}
