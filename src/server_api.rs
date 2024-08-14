use super::node_instance::{NodeInstanceInfo, NodeStatus};

use leptos::*;

// Obtain the list of existing nodes instances with their info
// TODO: replace with actual implementation of it
#[server(NodeInstances, "/api", "Url", "/nodes")]
pub async fn nodes_instances() -> Result<RwSignal<Vec<RwSignal<NodeInstanceInfo>>>, ServerFnError> {
    let nodes = vec![
        NodeInstanceInfo {
            name: "safenode1".to_string(),
            peer_id: rand::random::<[u8; 10]>().to_vec(),
            status: NodeStatus::Active,
            rewards: 4321u64,
            balance: 1234u64,
            chunks: 100,
        },
        NodeInstanceInfo {
            name: "safenode2".to_string(),
            peer_id: rand::random::<[u8; 10]>().to_vec(),
            status: NodeStatus::Inactive,
            rewards: 8765u64,
            balance: 5678u64,
            chunks: 200,
        },
    ];

    // TODO: this is just to mimic an async call
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // start with a set of three rows
    Ok(create_rw_signal(vec![
        create_rw_signal(nodes[0].clone()),
        create_rw_signal(nodes[1].clone()),
    ]))
}

// Create and add a new node instance returning its info
// TODO: replace with actual implementation of it
pub fn add_node_instance(set_nodes: RwSignal<Vec<RwSignal<NodeInstanceInfo>>>) {
    set_nodes.update(|items| {
        items.push(create_rw_signal(NodeInstanceInfo {
            name: "safenode3".to_string(),
            peer_id: rand::random::<[u8; 10]>().to_vec(),
            status: NodeStatus::Inactive,
            rewards: 2109u64,
            balance: 9012u64,
            chunks: 300,
        }));
    });
}
