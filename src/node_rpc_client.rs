use super::node_instance::NodeInstanceInfo;

use leptos::*;
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, KBucketsRequest, NetworkInfoRequest, NodeInfoRequest,
    RecordAddressesRequest,
};
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

    pub async fn update_node_info(&mut self, info: &mut NodeInstanceInfo) {
        if let Err(err) = self.node_info(info).await {
            logging::log!(
                "Failed to get basic info from running node using RPC endpoint {}: {err:?}",
                self.endpoint
            );
        }
        if let Err(err) = self.network_info(info).await {
            logging::log!(
                "Failed to get peers info from running node using RPC endpoint {}: {err:?}",
                self.endpoint
            );
        }
        if let Err(err) = self.record_addresses(info).await {
            logging::log!(
                "Failed to get record addresses from running node using RPC endpoint {}: {err:?}",
                self.endpoint
            );
        }
        if let Err(err) = self.kbuckets(info).await {
            logging::log!(
                "Failed to get kbuckets peers info from running node using RPC endpoint {}: {err:?}",
                self.endpoint
            );
        }
    }

    async fn node_info(&mut self, info: &mut NodeInstanceInfo) -> Result<(), RpcClientError> {
        logging::log!(
            "Sending RPC query to get node's basic info: {} ...",
            self.endpoint
        );

        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
        let node_info = response.get_ref();

        info.peer_id = Some(bs58::encode(&node_info.peer_id).into_string());
        info.bin_version = Some(node_info.bin_version.clone());
        info.balance = Some(node_info.wallet_balance);

        Ok(())
    }

    async fn network_info(&mut self, info: &mut NodeInstanceInfo) -> Result<(), RpcClientError> {
        logging::log!(
            "Sending RPC query to get node's peers info: {} ...",
            self.endpoint
        );

        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client
            .network_info(Request::new(NetworkInfoRequest {}))
            .await?;
        let network_info = response.get_ref();

        info.connected_peers = Some(network_info.connected_peers.len());

        Ok(())
    }

    async fn record_addresses(
        &mut self,
        info: &mut NodeInstanceInfo,
    ) -> Result<(), RpcClientError> {
        logging::log!(
            "Sending RPC query to get node's record addresses info: {} ...",
            self.endpoint
        );

        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client
            .record_addresses(Request::new(RecordAddressesRequest {}))
            .await?;
        let record_addresses = response.get_ref();

        info.records = Some(record_addresses.addresses.len());

        Ok(())
    }

    async fn kbuckets(&mut self, info: &mut NodeInstanceInfo) -> Result<(), RpcClientError> {
        logging::log!(
            "Sending RPC query to get node's kbuckets peers info: {} ...",
            self.endpoint
        );

        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client.k_buckets(Request::new(KBucketsRequest {})).await?;
        let kbuckets_response = response.get_ref();

        let peers_count = kbuckets_response
            .kbuckets
            .iter()
            .map(|(_ilog2_distance, peers)| peers.peers.len())
            .sum();

        info.kbuckets_peers = Some(peers_count);

        Ok(())
    }
}
