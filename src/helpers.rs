use super::{
    app::ClientGlobalState,
    server_api::{create_node_instance, delete_node_instance, start_node_logs_stream},
};

use leptos::*;

// Creates and add a new node instance updating the given signal
pub async fn add_node_instance(port: u16, rpc_api_port: u16) -> Result<(), ServerFnError> {
    let context = expect_context::<ClientGlobalState>();

    let container = create_node_instance(port, rpc_api_port).await?;

    context.nodes.update(|items| {
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
    // TODO: check 'logs_stream_is_on' signal simultaneously to stop as soon as it's set to 'false'
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

        // use context to check if we should stop listening the logs stream
        let stop = move || !context.logs_stream_is_on.get_untracked();
        if stop() {
            break;
        }
    }

    logging::log!("Node logs stream dropped from container {container_id}.");
    Ok(())
}
