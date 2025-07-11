use super::{
    app::ClientGlobalState,
    server_api::{
        create_node_instance, delete_node_instance, nodes_actions_batch_create,
        start_node_logs_stream,
    },
    types::{BatchType, NodeId, NodeInstanceInfo, NodeOpts, NodesActionsBatch},
};

use alloy_primitives::U256;
use gloo_timers::future::TimeoutFuture;
use leptos::{logging, prelude::*, task::spawn_local};
use rand::Rng;

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

// Shows an alert message in the UI (currently as an error).
// TODO: allow to provide the type of alert, i.e. info, warning, etc.
pub fn show_alert_msg(msg: String) {
    let context = expect_context::<ClientGlobalState>();
    spawn_local(async move {
        let mut rng = rand::rng();
        let random_id = rng.random::<u64>();
        logging::log!("Alert msg. displayed: {msg}");
        context.alerts.update(|msgs| msgs.push((random_id, msg)));
        TimeoutFuture::new(ALERT_MSG_DURATION_MILLIS).await;
        context
            .alerts
            .update(|msgs| msgs.retain(|(id, _)| *id != random_id));
    });
}

// Creates and add new node instances
pub async fn add_node_instances(
    node_opts: NodeOpts,
    count: u16,
    interval_secs: u64,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    // random node_id and temporary
    let tmp_node_id = format!("tmp-{}", hex::encode(rand::random::<[u8; 6]>()));
    let tmp_node = NodeInstanceInfo {
        node_id: tmp_node_id.clone(),
        created: u64::MAX, // just so it's shown first as the newest in the UI
        ..Default::default()
    };
    context.nodes.update(|items| {
        items.1.insert(tmp_node_id.clone(), RwSignal::new(tmp_node));
    });

    if count > 1 {
        let batch_type = BatchType::Create { node_opts, count };
        let batch_id = nodes_actions_batch_create(batch_type.clone(), interval_secs).await?;
        context.nodes.update(|items| {
            items.1.remove(&tmp_node_id);
        });
        let batch_info = NodesActionsBatch::new(batch_id, batch_type, interval_secs);
        context
            .scheduled_batches
            .update(|batches| batches.push(RwSignal::new(batch_info)))
    } else {
        let node_info = create_node_instance(node_opts).await?;
        context.nodes.update(|items| {
            items.1.remove(&tmp_node_id);
            if !items.1.contains_key(&node_info.node_id) {
                items
                    .1
                    .insert(node_info.node_id.clone(), RwSignal::new(node_info));
            }
        });
    };

    Ok(())
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
                logging::log!("Error reading log: {err}");
                break;
            }
        }

        // use context to check if we should stop listening the logs stream
        if Some(true)
            != context
                .logs_stream_on_for
                .get_untracked()
                .map(|id| id == node_id)
        {
            break;
        }
    }

    logging::log!("Dropped node logs stream from node {node_id}.");
    Ok(())
}
