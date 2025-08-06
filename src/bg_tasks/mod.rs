mod batches;
#[cfg(not(feature = "lcd-disabled"))]
mod lcd;
mod metrics_client;
mod tasks;
mod tasks_ctx;

use super::{
    app::AppContext,
    node_mgr::NodeManager,
    types::{AppSettings, NodeId, NodeInstanceInfo, NodesActionsBatch},
};

pub use batches::{ActionsBatchError, prepare_node_action_batch};
pub use metrics_client::NodesMetrics;

use alloy::sol;
#[cfg(not(feature = "lcd-disabled"))]
use lcd::display_stats_on_lcd;
use leptos::logging;
use std::{collections::HashMap, sync::Arc};
use tasks::{balance_checker_task, check_node_bin_version, prune_metrics, update_nodes_info};
use tasks_ctx::TasksContext;
use tokio::{
    select,
    sync::{RwLock, broadcast},
    time::{Duration, Instant},
};

// Set of scheduled/running batches indexed by their Id
pub type NodeActionsBatches = Arc<RwLock<(broadcast::Sender<u16>, Vec<NodesActionsBatch>)>>;

// ERC20 token contract ABI
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TokenContract,
    "artifacts/token_contract_abi.json"
);

// Type of actions that can be requested to the bg jobs.
#[derive(Clone, Debug)]
pub enum BgTasksCmds {
    ApplySettings(AppSettings),
    CheckBalanceFor(NodeInstanceInfo),
    DeleteBalanceFor(NodeInstanceInfo),
    CheckAllBalances,
}

// List of nodes which status is temporarily immutable/locked.
#[derive(Clone, Debug, Default)]
pub struct ImmutableNodeStatus(Arc<RwLock<HashMap<super::types::NodeId, LockedStatus>>>);

#[derive(Clone, Debug)]
struct LockedStatus {
    // Timestamp when the status has been locked.
    timestamp: Instant,
    // Expiration information for when it should be unlocked.
    expiration_time: Duration,
}

impl ImmutableNodeStatus {
    pub async fn lock(&self, node_id: NodeId, expiration_time: Duration) {
        self.0.write().await.insert(
            node_id,
            LockedStatus {
                timestamp: Instant::now(),
                expiration_time,
            },
        );
    }

    pub async fn remove(&self, node_id: &NodeId) {
        self.0.write().await.remove(node_id);
    }

    // Check if the node id is still in the list, but also check if
    // its expiration has already passed and therefore has to be removed from the list.
    pub async fn is_still_locked(&self, node_id: &NodeId) -> bool {
        let info = self.0.read().await.get(node_id).cloned();
        match info {
            None => false,
            Some(LockedStatus {
                timestamp,
                expiration_time,
            }) => {
                if timestamp.elapsed() >= expiration_time {
                    self.remove(node_id).await;
                    false
                } else {
                    true
                }
            }
        }
    }
}

// Spawn any required background tasks
pub fn spawn_bg_tasks(app_ctx: AppContext, node_manager: NodeManager, settings: AppSettings) {
    logging::log!("Background tasks initialized with settings: {settings:#?}");
    let mut ctx = TasksContext::from(settings);

    let lcd_stats = Arc::new(RwLock::new(
        [(
            "Formicaio".to_string(),
            format!("v{}", env!("CARGO_PKG_VERSION")),
        )]
        .into_iter()
        .collect::<HashMap<String, String>>(),
    ));

    // Based on settings, setup LCD external device to display stats.
    #[cfg(not(feature = "lcd-disabled"))]
    if ctx.app_settings.lcd_display_enabled {
        tokio::spawn(display_stats_on_lcd(
            ctx.app_settings.clone(),
            app_ctx.bg_tasks_cmds_tx.subscribe(),
            lcd_stats.clone(),
        ));
    }

    // Spawn task which checks address balances as requested on the provided channel
    tokio::spawn(balance_checker_task(
        ctx.app_settings.clone(),
        node_manager.clone(),
        app_ctx.db_client.clone(),
        lcd_stats.clone(),
        app_ctx.bg_tasks_cmds_tx.clone(),
        app_ctx.stats.clone(),
    ));

    tokio::spawn(async move {
        let mut bg_tasks_cmds_rx = app_ctx.bg_tasks_cmds_tx.subscribe();
        loop {
            select! {
                settings = bg_tasks_cmds_rx.recv() => {
                    if let Ok(BgTasksCmds::ApplySettings(s)) = settings {
                        #[cfg(not(feature = "lcd-disabled"))]
                        if s.lcd_display_enabled && (!ctx.app_settings.lcd_display_enabled
                            || ctx.app_settings.lcd_device != s.lcd_device
                            || ctx.app_settings.lcd_addr != s.lcd_addr)
                        {
                            logging::log!("Setting up LCD display with new device parameters...");
                            // TODO: when it fails, send error back to the client,
                            // perhaps we need websockets for errors like this one.
                            tokio::spawn(display_stats_on_lcd(
                                s.clone(),
                                app_ctx.bg_tasks_cmds_tx.subscribe(),
                                lcd_stats.clone()
                            ));
                        }

                        ctx.apply_settings(s);
                    }
                },
                _ = ctx.formica_image_pulling.tick() => {
                    let node_manager = node_manager.clone();
                    tokio::spawn(async move {
                        if let Err(err) = node_manager.pull_formica_image().await {
                            logging::error!("[ERROR] Periodic task failed to pull node image: {err}");
                        }
                    });
                },
                _ = ctx.node_bin_version_check.tick() => {
                    tokio::spawn(check_node_bin_version(
                        node_manager.clone(),
                        app_ctx.db_client.clone(),
                    ));
                },
                _ = ctx.balances_retrieval.tick() => {
                    let _ = app_ctx.bg_tasks_cmds_tx.send(BgTasksCmds::CheckAllBalances);
                },
                _ = ctx.metrics_pruning.tick() => {
                    tokio::spawn(prune_metrics(
                        node_manager.clone(),
                        app_ctx.db_client.clone()
                    ));
                },
                _ = ctx.nodes_metrics_polling.tick() => {
                    let query_bin_version = ctx.app_settings.lcd_display_enabled;

                    // we don't spawn a task for this one just in case it's taking
                    // too long to complete and we may start overwhelming the backend
                    // with multiple overlapping tasks being launched.
                    update_nodes_info(
                        &node_manager,
                        app_ctx.clone(),
                        query_bin_version,
                        &lcd_stats
                    ).await;
                    // reset interval to start next period from this instant,
                    // regardless how long the above polling task lasted.
                    ctx.nodes_metrics_polling.reset_after(ctx.nodes_metrics_polling.period());
                }
            }
        }
    });
}
