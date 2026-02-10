use super::notifications::Notification;
use crate::{
    app::{ActionTriggered, ClientGlobalState},
    server_api::{
        create_node_instance, delete_node_instance, nodes_actions_batch_create,
        start_node_logs_stream,
    },
    types::{BatchType, NodeId, NodeOpts},
};

use alloy_primitives::U256;
use gloo_timers::future::TimeoutFuture;
use leptos::{logging, prelude::*, task::spawn_local};

// Duration of each alert message shows in the UI
const ALERT_MSG_DURATION_MILLIS: u32 = 9_000;

// Format a U256 value truncating it to only 4 decimals if it's too large in attos.
pub fn truncated_balance_str(v: U256) -> String {
    if v > U256::from(1_000_000u128) {
        format!(
            "{:.4}",
            f64::from(v / U256::from(1_000_000_000_000u128)) / 1_000_000.0
        )
    } else {
        format!("{v} attos")
    }
}

// Shows an error alert message in the UI.
pub fn show_error_alert_msg(msg: String) {
    let notif = Notification::new_error(msg.clone());
    let context = expect_context::<ClientGlobalState>();
    spawn_local(async move {
        logging::log!("Alert msg. displayed: {}", notif.message);
        let notif_id = notif.id;
        context.alerts.update(|msgs| msgs.push(notif));
        TimeoutFuture::new(ALERT_MSG_DURATION_MILLIS).await;
        context.alerts.update(|msgs| {
            if let Some(notif) = msgs.iter_mut().find(|notif| notif.id == notif_id) {
                notif.shown = true;
            }
        });
    });
}

// Creates and add new node instances
pub async fn add_node_instances(
    node_opts: NodeOpts,
    count: u16,
    interval_secs: u64,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    if count > 1 {
        context
            .is_action_triggered
            .set(ActionTriggered::BatchCreatingNodes);
        let batch_type = BatchType::Create { node_opts, count };
        match nodes_actions_batch_create(batch_type, interval_secs).await {
            Ok(_batch_id) => Ok(()),
            Err(err) => {
                context.is_action_triggered.set(ActionTriggered::None);
                Err(err)
            }
        }
    } else {
        context
            .is_action_triggered
            .set(ActionTriggered::CreatingNode);
        match create_node_instance(node_opts).await {
            Ok(_node_info) => Ok(()),
            Err(err) => {
                context.is_action_triggered.set(ActionTriggered::None);
                Err(err)
            }
        }
    }
}

// Removes a node instance with given id and updates given signal
pub async fn remove_node_instance(node_id: NodeId) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    delete_node_instance(node_id.clone()).await?;

    context.nodes.update(|nodes| {
        nodes.1.remove(&node_id);
    });

    Ok(())
}

// Obtains a stream from the node's log
pub async fn node_logs_stream(
    node_id: NodeId,
    received_logs: WriteSignal<Vec<String>>,
) -> Result<(), ServerFnError> {
    use futures_util::stream::StreamExt;
    logging::log!("Initiating node logs stream from node {node_id}...");
    let mut logs_stream = start_node_logs_stream(node_id.clone()).await?.into_inner();

    let context = expect_context::<ClientGlobalState>();
    let mut cur_line = Vec::<u8>::new();
    // TODO: check 'logs_stream_is_on' signal simultaneously to stop as soon as it's set to 'false'
    while let Some(item) = logs_stream.next().await {
        match item {
            Ok(bytes) => {
                let lines = bytes.split(|&byte| byte == b'\n').collect::<Vec<_>>();
                let num_lines = lines.len();
                for (i, line) in lines.into_iter().enumerate() {
                    cur_line.extend(line);
                    if i < num_lines - 1 {
                        let log: String = String::from_utf8_lossy(&cur_line).to_string();
                        received_logs.update(|entries| entries.push(log));
                        cur_line.clear();
                    }
                }
            }
            Err(err) => {
                logging::error!("[ERROR] Error reading node logs: {err}");
                break;
            }
        }

        // use context to check if we should stop listening the logs stream
        if Some(true)
            != context
                .logs_stream_on_for
                .get_untracked()
                .map(|info| info.read().node_id == node_id)
        {
            break;
        }
    }

    logging::log!("Node logs stream ended for node {node_id}.");
    Ok(())
}
