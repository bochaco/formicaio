use super::node_instance::NodeInstanceInfo;
use leptos::*;

#[cfg(feature = "ssr")]
use super::{
    node_instance::NodeStatus,
    portainer_client::{
        create_new_container, delete_container_with, get_container_info, get_containers_list,
        start_container_with, stop_container_with, ContainerState,
    },
};

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
// TODO: replace with actual implementation of it
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
                rewards: 4321u64,
                balance: 1234u64,
                chunks: 100,
            })
        })
        .collect();

    // start with a set of three rows
    Ok(create_rw_signal(nodes))
}

// Create and add a new node instance returning its info
// TODO: replace with actual implementation of it
#[server(CreateNodeInstance, "/api", "Url", "/create_node")]
pub async fn create_node_instance() -> Result<NodeInstanceInfo, ServerFnError> {
    let container_id = create_new_container().await?;

    logging::log!("NEW CONTAINER ID: {container_id}");

    let container = get_container_info(&container_id).await?;
    logging::log!("NEW CONTAINER: {container:?}");

    Ok(NodeInstanceInfo {
        container_id: container.Id,
        created: container.Created,
        peer_id: rand::random::<[u8; 10]>().to_vec(),
        status: NodeStatus::from(container.State),
        rewards: 2109u64,
        balance: 9012u64,
        chunks: 300,
    })
}

// Delete a node instance with given id
#[server(DeleteNodeInstance, "/api", "Url", "/delete_node")]
pub async fn delete_node_instance(container_id: String) -> Result<(), ServerFnError> {
    delete_container_with(&container_id).await?;
    Ok(())
}

// Start a node instance with given id
#[server(StartNodeInstance, "/api", "Url", "/start_node")]
pub async fn start_node_instance(container_id: String) -> Result<(), ServerFnError> {
    start_container_with(&container_id).await?;
    Ok(())
}

// Stop a node instance with given id
#[server(StopNodeInstance, "/api", "Url", "/stop_node")]
pub async fn stop_node_instance(container_id: String) -> Result<(), ServerFnError> {
    stop_container_with(&container_id).await?;
    Ok(())
}

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
