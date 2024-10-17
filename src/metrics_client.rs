use super::node_instance::NodeInstanceInfo;

use leptos::*;
use std::collections::BTreeMap;
use thiserror::Error;

// Default value for the nodes metrics host
const DEFAULT_NODES_METRICS_HOST: &str = "127.0.0.1";

#[derive(Debug, Error)]
pub enum MetricsClientError {
    #[error(transparent)]
    Tonic(#[from] tonic::Status),
    #[error(transparent)]
    TransportError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub struct NodeMetricsClient {
    endpoint: String,
}

impl NodeMetricsClient {
    pub fn new(ip: &Option<String>, port: u16) -> Result<Self, MetricsClientError> {
        let host = ip.clone().unwrap_or(DEFAULT_NODES_METRICS_HOST.to_string());
        Ok(Self {
            endpoint: format!("http://{host}:{port}/metrics"),
        })
    }

    pub async fn update_node_info(&mut self, info: &mut NodeInstanceInfo) {
        if let Err(err) = self.fetch_metrics(info).await {
            logging::log!(
                "Failed to get metrics from running node at endpoint {}: {err:?}",
                self.endpoint
            );
        }
    }

    // TODO: retrieve not only the rewards amounts but other metrics
    async fn fetch_metrics(
        &mut self,
        info: &mut NodeInstanceInfo,
    ) -> Result<(), MetricsClientError> {
        logging::log!(
            "Sending request to node metrics server: {} ...",
            self.endpoint
        );

        let response = reqwest::get(&self.endpoint).await?.text().await?;

        let metrics = parse_metrics(&response);
        for (key, value) in metrics {
            if key == "sn_node_total_forwarded_rewards" {
                info.rewards_received = value.parse::<u64>().ok();
            } else if key == "sn_node_current_reward_wallet_balance" {
                info.balance = value.parse::<u64>().ok();
            }
        }

        Ok(())
    }
}

// Parse the metrics into a BTreeMap
fn parse_metrics(metrics: &str) -> BTreeMap<String, String> {
    let mut parsed_metrics = BTreeMap::new();

    for line in metrics.lines() {
        if line.starts_with('#') {
            continue; // Skip comments
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0].to_string();
            let value = parts[1].to_string();
            parsed_metrics.insert(key, value);
        }
    }

    parsed_metrics
}
