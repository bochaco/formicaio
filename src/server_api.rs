use super::node_instance::NodeInstanceInfo;
use leptos::*;

#[cfg(feature = "ssr")]
use super::{
    node_instance::NodeStatus,
    portainer_client::{
        create_new_container, delete_container_with, get_container_info, get_container_logs_stream,
        get_containers_list, start_container_with, stop_container_with, ContainerState,
    },
};
#[cfg(feature = "ssr")]
use futures_util::StreamExt;
use server_fn::codec::{ByteStream, Streaming};

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
// TODO: read node instances metadata form a database
#[server(ListNodeInstances, "/api", "Url", "/list_nodes")]
pub async fn nodes_instances() -> Result<RwSignal<Vec<RwSignal<NodeInstanceInfo>>>, ServerFnError> {
    let containers = get_containers_list().await?;

    let nodes = containers
        .into_iter()
        .map(|container| {
            create_rw_signal(NodeInstanceInfo {
                container_id: container.Id,
                created: container.Created,
                peer_id: rand::random::<[u8; 10]>().to_vec(),
                status: NodeStatus::from(container.State),
                status_info: container.Status,
                rewards: 4321u64,
                balance: 1234u64,
                chunks: 100,
            })
        })
        .collect();

    Ok(create_rw_signal(nodes))
}

// Create and add a new node instance returning its info
// TODO: read node instances metadata form a database
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance() -> Result<NodeInstanceInfo, ServerFnError> {
    logging::log!("Creating new node container...");
    let container_id = create_new_container().await?;
    logging::log!("New node container Id: {container_id} ...");

    let container = get_container_info(&container_id).await?;
    logging::log!("New node container created: {container:?}");

    Ok(NodeInstanceInfo {
        container_id: container.Id,
        created: container.Created,
        peer_id: rand::random::<[u8; 10]>().to_vec(),
        status: NodeStatus::from(container.State),
        status_info: container.Status,
        rewards: 2109u64,
        balance: 9012u64,
        chunks: 300,
    })
}

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(container_id: String) -> Result<(), ServerFnError> {
    logging::log!("Deleting node container with Id: {container_id} ...");
    delete_container_with(&container_id).await?;
    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(container_id: String) -> Result<(), ServerFnError> {
    logging::log!("Starting node container with Id: {container_id} ...");
    start_container_with(&container_id).await?;
    Ok(())
}

// Stop a node instance with given id
#[server(StopNodeInstance, "/api", "Url", "/stop_node")]
pub async fn stop_node_instance(container_id: String) -> Result<(), ServerFnError> {
    logging::log!("Stopping node container with Id: {container_id} ...");
    stop_container_with(&container_id).await?;
    Ok(())
}

#[server(output = Streaming)]
pub async fn start_node_logs_stream(container_id: String) -> Result<ByteStream, ServerFnError> {
    logging::log!("Starting logs stream from container with Id: {container_id} ...");
    let container_logs_stream = get_container_logs_stream(&container_id).await?;
    let converted_stream = container_logs_stream.map(|item| {
        item.map_err(ServerFnError::from) // Convert the error type
    });
    Ok(ByteStream::new(converted_stream))
}
