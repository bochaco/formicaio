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
    show_alert_msg(notif);
}

// Shows a warning alert message in the UI.
#[cfg(feature = "hydrate")]
pub fn show_warning_alert_msg(msg: String) {
    let notif = Notification::new_warning(msg.clone());
    show_alert_msg(notif);
}

// Helper to show an alert message in the UI.
fn show_alert_msg(notif: Notification) {
    let context = expect_context::<ClientGlobalState>();
    spawn_local(async move {
        logging::log!("Alert message displayed: {}", notif.message);
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

pub fn human_readable_percent(pct: f64) -> String {
    if pct == 0.0 {
        return "0%".into();
    }
    let neg = pct.is_sign_negative();
    let v = pct.abs();

    // full-word suffixes from thousand up to septillion (10^24)
    let units = [
        (1e24_f64, "septillion"),
        (1e21_f64, "sextillion"),
        (1e18_f64, "quintillion"),
        (1e15_f64, "quadrillion"),
        (1e12_f64, "trillion"),
        (1e9_f64, "billion"),
        (1e6_f64, "million"),
        (1e3_f64, "thousand"),
    ];

    // For normal percentages (<1000%) use fixed sensible decimals and trim trailing zeros
    if v < 1000.0 {
        let mut s = if v >= 100.0 {
            format!("{:.0}", v.round())
        } else if v >= 10.0 {
            format!("{:.1}", (v * 10.0).round() / 10.0)
        } else {
            format!("{:.2}", (v * 100.0).round() / 100.0)
        };
        while s.contains('.') && (s.ends_with('0') || s.ends_with('.')) {
            s.pop();
        }
        return if neg {
            format!("-{s}%")
        } else {
            format!("+{s}%")
        };
    }

    // For very large percentages use full-word suffixes then percent sign
    for &(threshold, suffix) in &units {
        if v >= threshold {
            let scaled = v / threshold;
            let formatted = if scaled >= 100.0 {
                format!("{scaled:.0}")
            } else if scaled >= 10.0 {
                format!("{:.1}", (scaled * 10.0).round() / 10.0)
            } else {
                format!("{:.2}", (scaled * 100.0).round() / 100.0)
            };
            let out = if neg {
                format!("-{formatted} {suffix}%")
            } else {
                format!("+{formatted} {suffix}%")
            };
            return out;
        }
    }

    // fallback (shouldn't reach)
    if neg {
        format!("-{v}%")
    } else {
        format!("+{v}%")
    }
}
