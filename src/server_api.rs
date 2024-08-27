use super::node_instance::NodeInstanceInfo;

#[cfg(feature = "ssr")]
use super::{
    metadata_db::{db_delete_node_metadata, db_get_node_metadata, db_store_node_metadata},
    node_instance::NodeStatus,
    node_rpc_client::{rpc_network_info, rpc_node_info},
    portainer_client::{
        create_new_container, delete_container_with, get_container_info, get_container_logs_stream,
        get_containers_list, start_container_with, stop_container_with, ContainerState,
    },
};

#[cfg(feature = "ssr")]
use futures_util::StreamExt;
use leptos::*;
use server_fn::codec::{ByteStream, Streaming};
#[cfg(feature = "ssr")]
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[cfg(feature = "ssr")]
impl From<ContainerState> for NodeStatus {
    fn from(item: ContainerState) -> NodeStatus {
        match item {
            ContainerState::created => NodeStatus::Inactive,
            ContainerState::restarting => NodeStatus::Restarting,
            ContainerState::running => NodeStatus::Active,
            ContainerState::removing => NodeStatus::Removing,
            ContainerState::paused | ContainerState::exited | ContainerState::dead => {
                NodeStatus::Inactive
            }
        }
    }
}

// Obtain the list of existing nodes instances with their info
#[server(ListNodeInstances, "/api", "Url", "/list_nodes")]
pub async fn nodes_instances() -> Result<RwSignal<Vec<RwSignal<NodeInstanceInfo>>>, ServerFnError> {
    let containers = get_containers_list().await?;

    let mut nodes = vec![];
    for container in containers {
        let mut node_instance_info = NodeInstanceInfo {
            container_id: container.Id,
            created: container.Created,
            peer_id: None,
            status: NodeStatus::from(container.State),
            status_info: container.Status,
            bin_version: None,
            port: None,
            rpc_api_port: None,
            rewards: None,
            balance: None,
            chunks: None,
            connected_peers: None,
        };

        // we first read node metadata cached in the database
        // TODO: fetch metadata of all containers from DB with a single DB call
        db_get_node_metadata(&mut node_instance_info).await?;

        // if the node is Active, we can also fetch up to date info using its RPC API
        retrive_and_cache_updated_metadata(&mut node_instance_info).await?;

        nodes.push(create_rw_signal(node_instance_info))
    }

    Ok(create_rw_signal(nodes))
}

// Create and add a new node instance returning its info
// TODO: read node instances metadata form a database
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance(
    port: u16,
    rpc_api_port: u16,
) -> Result<NodeInstanceInfo, ServerFnError> {
    logging::log!("Creating new node container with port {port}, RPC API port {rpc_api_port} ...");
    let container_id = create_new_container(port, rpc_api_port).await?;
    logging::log!("New node container Id: {container_id} ...");

    let container = get_container_info(&container_id).await?;
    logging::log!("New node container created: {container:?}");

    let node_instance_info = NodeInstanceInfo {
        container_id: container.Id,
        created: container.Created,
        peer_id: Some(bs58::encode(rand::random::<[u8; 10]>().to_vec()).into_string()),
        status: NodeStatus::from(container.State),
        status_info: container.Status,
        bin_version: None,
        port: Some(port),
        rpc_api_port: Some(rpc_api_port),
        rewards: None,
        balance: None,
        chunks: None,
        connected_peers: None,
    };

    db_store_node_metadata(&node_instance_info).await?;

    Ok(node_instance_info)
}

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(container_id: String) -> Result<(), ServerFnError> {
    logging::log!("Deleting node container with Id: {container_id} ...");
    delete_container_with(&container_id).await?;
    db_delete_node_metadata(&container_id).await?;
    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(container_id: String) -> Result<(), ServerFnError> {
    logging::log!("Starting node container with Id: {container_id} ...");
    start_container_with(&container_id).await?;
    // TODO: retrive and cache up to date node's metadata
    Ok(())
}

// Stop a node instance with given id
#[server(StopNodeInstance, "/api", "Url", "/stop_node")]
pub async fn stop_node_instance(container_id: String) -> Result<(), ServerFnError> {
    logging::log!("Stopping node container with Id: {container_id} ...");
    stop_container_with(&container_id).await?;
    // TODO: retrive and cache up to date node's metadata
    Ok(())
}

// Start streaming logs from a node instance with given id
#[server(output = Streaming)]
pub async fn start_node_logs_stream(container_id: String) -> Result<ByteStream, ServerFnError> {
    logging::log!("Starting logs stream from container with Id: {container_id} ...");
    let container_logs_stream = get_container_logs_stream(&container_id).await?;
    let converted_stream = container_logs_stream.map(|item| {
        item.map_err(ServerFnError::from) // convert the error type
    });
    Ok(ByteStream::new(converted_stream))
}

// If the node is active, retrieve up to date node's metadata through
// its RPC API and update its cache in local database.
#[cfg(feature = "ssr")]
async fn retrive_and_cache_updated_metadata(
    node_instance_info: &mut NodeInstanceInfo,
) -> Result<(), ServerFnError> {
    if node_instance_info.status.is_active() {
        if let Some(port) = node_instance_info.rpc_api_port {
            let rpc_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);

            // TODO: send info back to the user if we receive an error from using RPC client.
            if let Err(err) = rpc_node_info(rpc_addr, node_instance_info).await {
                logging::log!("Failed to get basic info from running node using RPC endpoint {rpc_addr}: {err}");
            }
            if let Err(err) = rpc_network_info(rpc_addr, node_instance_info).await {
                logging::log!(
                    "Failed to peers info from running node using RPC endpoint {rpc_addr}: {err}"
                );
            }
            // update DB with this new info we just obtained
            db_store_node_metadata(&node_instance_info).await?;
        }
    }

    Ok(())
}
