use crate::app::BatchInProgress;

use super::{
    app::ClientGlobalState,
    node_instance::{ContainerId, NodeInstanceInfo},
    server_api::{
        create_node_instance, delete_node_instance, prepare_node_instances_batch,
        start_node_logs_stream,
    },
};

use gloo_timers::future::TimeoutFuture;
use leptos::{logging, prelude::*, task::spawn_local};
use rand::Rng;

// Duration of each alert message shows in the UI
const ALERT_MSG_DURATION_MILLIS: u32 = 9_000;

// Shows an alert message in the UI (currently as an error).
// TODO: allow to provide the type of alert, i.e. info, warning, etc.
pub fn show_alert_msg(msg: String) {
    let context = expect_context::<ClientGlobalState>();
    spawn_local(async move {
        let mut rng = rand::thread_rng();
        let random_id = rng.gen::<u64>();
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
    port: u16,
    metrics_port: u16,
    count: u16,
    rewards_addr: String,
    home_network: bool,
    auto_start: bool,
    interval_secs: u64,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    // random container_id and temporary
    let tmp_container_id = format!("tmp-{}", hex::encode(rand::random::<[u8; 6]>()));
    let tmp_container = NodeInstanceInfo {
        container_id: tmp_container_id.clone(),
        created: u64::MAX, // just so it's shown first as the newest in the UI
        ..Default::default()
    };
    context.nodes.update(|items| {
        items
            .1
            .insert(tmp_container_id.clone(), RwSignal::new(tmp_container));
    });

    if count > 1 {
        prepare_node_instances_batch(
            port,
            metrics_port,
            count,
            rewards_addr,
            home_network,
            auto_start,
            interval_secs,
        )
        .await?;
        context.nodes.update(|items| {
            items.1.remove(&tmp_container_id);
        });
        context.batch_in_progress.update(|info| {
            if let Some(b) = info {
                b.total += count;
            } else {
                *info = Some(BatchInProgress {
                    created: 0,
                    total: count,
                    auto_start,
                    interval_secs,
                });
            }
        })
    } else {
        let node_info =
            create_node_instance(port, metrics_port, rewards_addr, home_network, auto_start)
                .await?;
        context.nodes.update(|items| {
            items.1.remove(&tmp_container_id);
            items
                .1
                .insert(node_info.container_id.clone(), RwSignal::new(node_info));
        });
    };

    Ok(())
}

// Removes a node instance with given id and updates given signal
pub async fn remove_node_instance(container_id: ContainerId) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    delete_node_instance(container_id.clone()).await?;

    context.nodes.update(|nodes| {
        nodes.1.remove(&container_id);
    });

    Ok(())
}

// Obtains a stream from the node's log
pub async fn node_logs_stream(
    container_id: ContainerId,
    received_logs: WriteSignal<Vec<String>>,
) -> Result<(), ServerFnError> {
    use futures_util::stream::StreamExt;
    logging::log!("Initiating node logs stream from container {container_id}...");
    let mut logs_stream = start_node_logs_stream(container_id.clone())
        .await?
        .into_inner();

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
                        received_logs.update(|entries| entries.push(log.to_string()));
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
                .map(|id| id == container_id)
        {
            break;
        }
    }

    logging::log!("Dropped node logs stream from container {container_id}.");
    Ok(())
}
