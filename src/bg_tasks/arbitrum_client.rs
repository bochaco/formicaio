use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
};
use chrono::{DateTime, TimeZone, Utc};
use leptos::logging;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use thiserror::Error;
use url::Url;

/// Error types for Arbitrum client operations
#[derive(Debug, Error)]
pub enum ArbitrumClientError {
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Failed to parse data: {0}")]
    ParseError(String),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

/// Container for payment records for a specific address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressPayments {
    /// The rewards address
    pub address: String,
    /// List of payments received in the last few days
    pub payments: Vec<PaymentRecord>,
    /// When this data was last updated
    pub last_updated: DateTime<Utc>,
}

/// Individual payment record from blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    /// Timestamp of the block
    pub timestamp: DateTime<Utc>,
    /// Amount received in wei
    pub amount: U256,
}

/// Client for querying payment data from Arbitrum L2
pub struct ArbitrumClient {
    /// RPC endpoint URL
    endpoint: Url,
    /// Contract address to query transactions from
    contract_address: Address,
    /// Rewards addresses to monitor (transaction destinations)
    rewards_addresses: Vec<Address>,
    /// Number of days of history to track
    days_to_track: u64,
}

impl ArbitrumClient {
    /// Creates a new Arbitrum client
    ///
    /// # Arguments
    /// * `endpoint` - Arbitrum L2 RPC endpoint URL (e.g., "https://arb1.arbitrum.io/rpc")
    /// * `contract_address` - The contract address to query transactions from (as string with 0x prefix)
    /// * `rewards_addresses` - List of destination addresses to monitor (as strings with 0x prefix)
    /// * `days_to_track` - Number of days of payment history to retrieve
    ///
    /// # Errors
    /// Returns `ArbitrumClientError::InvalidAddress` if contract address is malformed
    /// Returns `ArbitrumClientError::ConfigError` if RPC URL is invalid
    pub fn new<'a>(
        endpoint: &str,
        contract_address: &str,
        rewards_addresses: impl Iterator<Item = &'a Address>,
        days_to_track: u64,
    ) -> Result<Self, ArbitrumClientError> {
        let contract_addr = contract_address
            .parse::<Address>()
            .map_err(|_| ArbitrumClientError::InvalidAddress(contract_address.to_string()))?;

        Ok(Self {
            endpoint: endpoint.parse().map_err(|_| {
                ArbitrumClientError::ConfigError("Invalid RPC endpoint".to_string())
            })?,
            contract_address: contract_addr,
            rewards_addresses: rewards_addresses.cloned().collect(),
            days_to_track,
        })
    }

    /// Fetches incoming payments for all monitored rewards addresses
    ///
    /// # Errors
    /// Returns `ArbitrumClientError` if the RPC call fails
    pub async fn fetch_incoming_payments(
        &self,
    ) -> Result<Vec<AddressPayments>, ArbitrumClientError> {
        let provider = ProviderBuilder::new().connect_http(self.endpoint.clone());

        let mut all_payments = Vec::new();
        for address in &self.rewards_addresses {
            logging::log!("Fetching payments info for address: {address}",);
            match self.fetch_payments_for_address(&provider, *address).await {
                Ok(payments) => {
                    logging::log!(
                        "Successfully retrieved {} payments info for address {address}",
                        payments.len()
                    );
                    all_payments.push(AddressPayments {
                        address: address.to_string(),
                        payments,
                        last_updated: Utc::now(),
                    });
                }
                Err(err) => {
                    logging::error!("[ERROR] Failed to fetch payments for address {address}: {err}")
                }
            }
        }

        Ok(all_payments)
    }

    /// Fetches incoming payments for a specific address
    async fn fetch_payments_for_address(
        &self,
        provider: &impl Provider,
        address: Address,
    ) -> Result<Vec<PaymentRecord>, ArbitrumClientError> {
        let current_block = provider
            .get_block_number()
            .await
            .map_err(|e| ArbitrumClientError::RpcError(e.to_string()))?;

        // Estimate blocks per day on Arbitrum (~260k blocks per day)
        let blocks_per_day = 260_000u64;
        let blocks_to_check = self.days_to_track * blocks_per_day;
        let from_block = current_block.saturating_sub(blocks_to_check);

        // Build a filter to query transactions from the contract address (contract is the event source)
        let mut filter_obj = serde_json::Map::new();
        filter_obj.insert(
            "fromBlock".to_string(),
            serde_json::json!(format!("0x{:x}", from_block)),
        );
        filter_obj.insert("toBlock".to_string(), serde_json::json!("latest"));
        // Filter by contract address as the event source
        filter_obj.insert(
            "address".to_string(),
            serde_json::json!(format!("{:?}", self.contract_address)),
        );
        // Additionally, filter by the specific reward address as the destination in the transaction
        // Pad address (20 bytes) to 32 bytes for topic filter
        let padded_address = format!("0x{:0>64}", hex::encode(address));
        filter_obj.insert(
            "topics".to_string(),
            serde_json::json!([null, null, padded_address]),
        );
        let logs: Vec<JsonValue> = provider
            .raw_request::<serde_json::Value, Vec<JsonValue>>(
                Cow::Borrowed("eth_getLogs"),
                serde_json::json!([filter_obj]),
            )
            .await
            .map_err(|e| ArbitrumClientError::RpcError(e.to_string()))?;

        let mut payment_records = Vec::new();
        for log in logs {
            // Parse fields from raw JSON log
            let block_hex = log
                .get("blockNumber")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ArbitrumClientError::ParseError("Missing block number".into()))?;

            let block_number = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
                .map_err(|_| ArbitrumClientError::ParseError("Invalid block number".into()))?;

            // Get the block to retrieve timestamp via raw RPC
            let block: JsonValue = provider
                .raw_request::<serde_json::Value, JsonValue>(
                    Cow::Borrowed("eth_getBlockByNumber"),
                    serde_json::json!([format!("0x{:x}", block_number), false]),
                )
                .await
                .map_err(|e| ArbitrumClientError::RpcError(e.to_string()))?;

            let timestamp_hex = block
                .get("timestamp")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ArbitrumClientError::ParseError("Missing timestamp".into()))?;

            let timestamp_u64 = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)
                .map_err(|_| ArbitrumClientError::ParseError("Invalid timestamp".into()))?;

            let timestamp = Utc
                .timestamp_opt(timestamp_u64 as i64, 0)
                .single()
                .ok_or_else(|| ArbitrumClientError::ParseError("Invalid timestamp".into()))?;

            // For ERC-20 transfers, the amount is in the log data as a hex-encoded 256-bit integer
            let data_str = log.get("data").and_then(|v| v.as_str()).unwrap_or_default();
            let amount = if data_str == "0x" || data_str.is_empty() {
                U256::ZERO
            } else {
                // Parse hex string to U256 and convert to decimal string
                let hex_without_prefix = data_str.trim_start_matches("0x");
                U256::from_str_radix(hex_without_prefix, 16).unwrap_or_default()
            };
            payment_records.push(PaymentRecord { timestamp, amount });
        }

        Ok(payment_records)
    }
}
