use crate::{
    app::AppContext,
    node_mgr::NodeManager,
    server_api::parse_and_validate_addr,
    types::{BatchType, NodesActionsBatch},
};

use leptos::logging;
use rand::Rng;
use std::time::Duration;
use thiserror::Error;
use tokio::{select, time::sleep};

#[derive(Debug, Error)]
pub enum ActionsBatchError {
    #[error("Invalid rewards address: {0}")]
    InvalidAddress(String),
    #[error("Cannot create batch {0}: No node IDs provided.")]
    MissingNodeId(BatchType),
}

// Helper to prepare a node actions batch
// TODO: make it part of the bg tasks with a BgTasksCmd
pub async fn prepare_node_action_batch(
    batch_type: BatchType,
    interval_secs: u64,
    app_ctx: &AppContext,
    node_manager: &NodeManager,
) -> Result<u16, ActionsBatchError> {
    match &batch_type {
        BatchType::Create { node_opts, .. } => {
            // validate rewards address before accepting the batch
            parse_and_validate_addr(&node_opts.rewards_addr)
                .map_err(ActionsBatchError::InvalidAddress)?;
        }
        BatchType::Start(l)
        | BatchType::Stop(l)
        | BatchType::Upgrade(l)
        | BatchType::Recycle(l)
        | BatchType::Remove(l) => {
            // TODO: filter out nodes which are already part of a batch,
            // perhaps even return an error...?...
            if l.is_empty() {
                return Err(ActionsBatchError::MissingNodeId(batch_type));
            }

            // let's lock all nodes which are part of the batch,
            // so the user cannot action on it till the batch is completed or cancelled.
            let duration = Duration::from_secs((interval_secs + 2) * l.len() as u64);
            for node_id in l.iter() {
                app_ctx.db_client.set_node_status_to_locked(node_id).await;

                // let's also prevent the backend from updating its status
                app_ctx
                    .node_status_locked
                    .lock(node_id.clone(), duration)
                    .await;
            }
        }
    }

    let batch_id = rand::rng().random_range(0..=u16::MAX);
    let batch_info = NodesActionsBatch::new(batch_id, batch_type, interval_secs);
    logging::log!("Creating new batch with ID {batch_id}: {batch_info:?}");

    let len = {
        let batches = &mut app_ctx.node_action_batches.write().await.1;
        batches.push(batch_info);
        batches.len()
    };

    // spawn a task if there was no other tasks already batched
    if len == 1 {
        tokio::spawn(run_batches(app_ctx.clone(), node_manager.clone()));
    }

    Ok(batch_id)
}

async fn run_batches(app_ctx: AppContext, node_manager: NodeManager) {
    let mut cancel_rx = app_ctx.node_action_batches.read().await.0.subscribe();

    loop {
        let batch_info =
            if let Some(next_batch) = app_ctx.node_action_batches.write().await.1.first_mut() {
                let mut batch = next_batch.clone();
                batch.status = "In progress".to_string();
                *next_batch = batch.clone();
                batch
            } else {
                return;
            };

        match batch_info.batch_type {
            BatchType::Create {
                ref node_opts,
                count,
            } => {
                logging::log!("Started node instances creation batch of {count} nodes ...");
                let mut i = 0;
                loop {
                    select! {
                        batch_id = cancel_rx.recv() => {
                            if matches!(batch_id, Ok(id) if id == batch_info.id) {
                                break;
                            }
                        },
                        _ = sleep(Duration::from_secs(batch_info.interval_secs)) => {
                            let mut node_opts_clone = node_opts.clone();
                            node_opts_clone.port += i;
                            node_opts_clone.metrics_port += i;
                            i += 1;
                            match node_manager.create_node_instance(node_opts_clone).await {
                                Err(err) => logging::error!(
                                    "[ERROR] Failed to create node instance {i}/{count} as part of a batch: {err}"
                                ),
                                Ok(_) => if let Some(ref mut b) = app_ctx
                                    .node_action_batches.write().await.1
                                    .iter_mut()
                                    .find(|batch| batch.id == batch_info.id)
                                {
                                    b.complete += 1;
                                }
                            }

                            if i == count {
                                break;
                            }
                        }
                    }
                }
            }
            BatchType::Start(ref nodes)
            | BatchType::Stop(ref nodes)
            | BatchType::Upgrade(ref nodes)
            | BatchType::Recycle(ref nodes)
            | BatchType::Remove(ref nodes) => {
                let count = nodes.len();
                logging::log!("Starting actions batch for {count} nodes ...");
                let mut i = 0;
                loop {
                    select! {
                        batch_id = cancel_rx.recv() => {
                            if matches!(batch_id, Ok(id) if id == batch_info.id) {
                                break;
                            }
                        },
                        _ = sleep(Duration::from_secs(batch_info.interval_secs)) => {
                            let node_id = nodes[i].clone();
                            app_ctx.node_status_locked.remove(&node_id).await;
                            app_ctx.db_client.unlock_node_status(&node_id).await;
                            let res = match batch_info.batch_type {
                                BatchType::Start(_) => node_manager.start_node_instance(node_id).await,
                                BatchType::Stop(_) => node_manager.stop_node_instance(node_id).await,
                                BatchType::Upgrade(_) => node_manager.upgrade_node_instance(&node_id).await,
                                BatchType::Recycle(_) => node_manager.recycle_node_instance(node_id).await,
                                BatchType::Remove(_) => node_manager.delete_node_instance(node_id).await,
                                BatchType::Create {..} => Ok(())
                            };

                            match res {
                                Err(err) => logging::log!(
                                    "Node action failed on node instance {}/{count} as part of a batch: {err}", i+1
                                ),
                                Ok(()) => if let Some(ref mut b) = app_ctx
                                    .node_action_batches.write().await.1
                                    .iter_mut()
                                    .find(|batch| batch.id == batch_info.id)
                                {
                                    b.complete += 1;
                                }
                            }

                            i += 1;
                            if i == count {
                                break;
                            }
                        }
                    }
                }
            }
        }

        app_ctx
            .node_action_batches
            .write()
            .await
            .1
            .retain(|batch| batch.id != batch_info.id);
    }
}
