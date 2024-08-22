use leptos::*;

use super::{
    node_instance::NodeInstanceInfo,
    server_api::{create_node_instance, delete_node_instance, start_node_logs_stream},
};

// Creates and add a new node instance updating the given signal
pub async fn add_node_instance(
    set_nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> Result<(), ServerFnError> {
    let container = create_node_instance().await?;

    set_nodes.update(|items| {
        items.insert(0, create_rw_signal(container));
    });

    Ok(())
}

// Removes a node instance with given id and updates given signal
pub async fn remove_node_instance(
    container_id: String,
    set_nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>,
) -> Result<(), ServerFnError> {
    delete_node_instance(container_id.clone()).await?;

    set_nodes.update(|nodes| {
        nodes.retain(|node| node.get().container_id != container_id);
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

    let logs_stream_is_on = expect_context::<RwSignal<bool>>();
    while let Some(item) = logs_stream.next().await {
        match item {
            Ok(bytes) => {
                let log: String = String::from_utf8_lossy(&bytes).to_string();
                received_logs.update(|entries| entries.push(log));
            }
            Err(err) => {
                logging::log!("Error reading log: {err}");
                break;
            }
        }

        let stop = move || !logs_stream_is_on.get_untracked();
        if stop() {
            break;
        }
    }

    logging::log!("Node logs stream dropped from container {container_id}.");
    Ok(())
}
