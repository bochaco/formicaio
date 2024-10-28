use super::{app::ContainerId, metadata_db::DbClient};

use leptos::*;
use sn_protocol::safenode_proto::{safe_node_client::SafeNodeClient, NodeInfoRequest};
use thiserror::Error;
use tonic::Request;

// Default value for the nodes RPC API host
const DEFAULT_NODES_RPC_HOST: &str = "127.0.0.1";

#[derive(Debug, Error)]
pub enum RpcClientError {
    #[error(transparent)]
    Tonic(#[from] tonic::Status),
    #[error(transparent)]
    TransportError(#[from] tonic::transport::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub struct NodeRpcClient {
    endpoint: String,
}

impl NodeRpcClient {
    pub fn new(ip: &Option<String>, port: u16) -> Result<Self, RpcClientError> {
        let host = ip.clone().unwrap_or(DEFAULT_NODES_RPC_HOST.to_string());
        Ok(Self {
            endpoint: format!("https://{host}:{port}"),
        })
    }

    pub async fn update_node_info(&self, container_id: &ContainerId, db_client: &DbClient) {
        match self.node_info().await {
            Ok((peer_id, bin_version)) => {
                if let Err(err) = db_client
                    .update_node_metadata_field(container_id, "peer_id", &peer_id)
                    .await
                {
                    logging::log!(
                        "Failed to update DB cached peer_id info fetched through RPC API: {err}"
                    );
                }
                if let Err(err) = db_client
                    .update_node_metadata_field(container_id, "bin_version", &bin_version)
                    .await
                {
                    logging::log!(
                        "Failed to update DB cached bin_version fetched through RPC API: {err}"
                    )
                }
            }
            Err(err) => logging::log!(
                "Failed to get basic info from running node using RPC endpoint {}: {err:?}",
                self.endpoint
            ),
        }
    }

    async fn node_info(&self) -> Result<(String, String), RpcClientError> {
        logging::log!(
            "Sending RPC query to get node's basic info: {} ...",
            self.endpoint
        );

        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
        let node_info = response.get_ref();

        let peer_id = bs58::encode(&node_info.peer_id).into_string();

        Ok((peer_id, node_info.bin_version.clone()))
    }
}
