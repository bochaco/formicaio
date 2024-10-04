use super::{
    app::ClientGlobalState,
    node_instance::NodeInstanceInfo,
    server_api::{create_node_instance, delete_node_instance, start_node_logs_stream},
};

use leptos::*;

// Creates and add a new node instance updating the given signal
pub async fn add_node_instance(
    port: u16,
    rpc_api_port: u16,
    beta_tester_id: String,
) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    let tmp_container_id = format!("tmp-{}", hex::encode(rand::random::<[u8; 6]>().to_vec())); // random and temporary
    let tmp_container = NodeInstanceInfo {
        container_id: tmp_container_id.clone(),
        created: std::u64::MAX, // just so it's shown first as the newest in the UI
        ..Default::default()
    };
    context.nodes.update(|items| {
        items.insert(tmp_container_id.clone(), create_rw_signal(tmp_container));
    });

    let container = create_node_instance(port, rpc_api_port, beta_tester_id).await?;

    context.nodes.update(|items| {
        items.remove(&tmp_container_id);
        items.insert(container.container_id.clone(), create_rw_signal(container));
    });

    Ok(())
}

// Removes a node instance with given id and updates given signal
pub async fn remove_node_instance(container_id: String) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    delete_node_instance(container_id.clone()).await?;

    context.nodes.update(|nodes| {
        nodes.remove(&container_id);
    });

    Ok(())
}

// Obtains a stream from the node's log
pub async fn node_logs_stream(
    container_id: String,
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
                        received_logs.update(|entries| entries.push(format!("{log}")));
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
        let stop = move || !context.logs_stream_is_on.get_untracked();
        if stop() {
            break;
        }
    }

    logging::log!("Node logs stream dropped from container {container_id}.");
    Ok(())
}
