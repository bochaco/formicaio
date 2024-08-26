use super::node_instance::NodeInstanceInfo;

use leptos::*;
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, NetworkInfoRequest, NodeInfoRequest,
};
use std::net::SocketAddr;
use thiserror::Error;
use tonic::Request;

#[derive(Debug, Error)]
pub enum RpcClientError {
    #[error(transparent)]
    Tonic(#[from] tonic::Status),
    #[error(transparent)]
    TransportError(#[from] tonic::transport::Error),
}

pub async fn rpc_node_info(
    addr: SocketAddr,
    info: &mut NodeInstanceInfo,
) -> Result<(), RpcClientError> {
    let endpoint = format!("https://{addr}");
    logging::log!("Sending RPC query to get node's basic info: {endpoint} ...");

    let mut client = SafeNodeClient::connect(endpoint.clone()).await?;
    let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
    let node_info = response.get_ref();

    info.peer_id = Some(bs58::encode(&node_info.peer_id).into_string());
    info.bin_version = Some(node_info.bin_version.clone());
    info.balance = Some(node_info.wallet_balance);

    Ok(())
}

pub async fn rpc_network_info(
    addr: SocketAddr,
    info: &mut NodeInstanceInfo,
) -> Result<(), RpcClientError> {
    let endpoint = format!("https://{addr}");
    logging::log!("Sending RPC query to get node's peers info: {endpoint} ...");

    let mut client = SafeNodeClient::connect(endpoint.clone()).await?;
    let response = client
        .network_info(Request::new(NetworkInfoRequest {}))
        .await?;
    let network_info = response.get_ref();

    info.connected_peers = Some(network_info.connected_peers.len());

    Ok(())
}
