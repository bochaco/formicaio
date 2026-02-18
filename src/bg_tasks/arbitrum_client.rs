use crate::db_client::DbClient;
use alloy::{
    primitives::{Address, B256, U256, keccak256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{BlockNumberOrTag, Filter, eth::Log},
};
use chrono::{DateTime, TimeZone, Utc};
use leptos::logging;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tokio::time::{Duration, sleep};
use url::Url;

// Arbitrum One produces approximately 288,000 blocks per day.
// That's based on a block time of roughly 0.25–0.3 seconds — about 12,000 blocks
// per hour. Worth noting that Arbitrum block production depends entirely on
// chain usage, meaning blocks are only produced when there are transactions
// to sequence Arbitrum, so the actual number fluctuates. But under normal
// activity levels, ~288K/day is the standard estimate used in practice.
const BLOCKS_PER_DAY: u64 = 300_000;
const GET_LOGS_CHUNK_SIZE: u64 = 100_000;
// Number of days of earnings history to track. We need to compare the
// last 3 months with previous 3 months, i.e. at least 6 months of historic records.
const NUM_DAYS_TO_TRACK_EARNINGS: u64 = 210;
// Maximum number of blocks to process per fetch call to throttle RPC usage.
// Remaining blocks are fetched on the next scheduled invocation.
const MAX_BLOCKS_PER_FETCH_CALL: u64 = 1_000_000;
// Delay inserted between consecutive chunk requests to avoid overwhelming the RPC server.
const INTER_CHUNK_DELAY: Duration = Duration::from_millis(500);

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
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaymentRecord {
    /// Timestamp of the block
    pub timestamp: DateTime<Utc>,
    /// Amount received in wei
    pub amount: U256,
    /// Block number where the payment was recorded
    pub block_number: u64,
}

/// Client for querying payment data from Arbitrum L2
pub struct ArbitrumClient {
    /// RPC endpoint URL
    endpoint: Url,
    /// Contract address to query transactions from
    contract_address: Address,
    /// Rewards addresses to monitor (transaction destinations)
    rewards_addresses: Vec<Address>,
    /// Database client for caching earnings
    db_client: DbClient,
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
        db_client: DbClient,
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
            db_client,
        })
    }

    /// Prune old earnings history records based on the predefined number of days to track
    pub async fn prune_history(
        endpoint: &str,
        db_client: &DbClient,
    ) -> Result<(), ArbitrumClientError> {
        let endpoint = endpoint
            .parse()
            .map_err(|_| ArbitrumClientError::ConfigError("Invalid RPC endpoint".to_string()))?;

        let provider = ProviderBuilder::new().connect_http(endpoint);
        let current_block = provider
            .get_block_number()
            .await
            .map_err(|e| ArbitrumClientError::RpcError(e.to_string()))?;
        let blocks_to_fetch = NUM_DAYS_TO_TRACK_EARNINGS * BLOCKS_PER_DAY;
        let default_from = current_block.saturating_sub(blocks_to_fetch);

        db_client.delete_old_earnings(default_from).await;
        Ok(())
    }

    /// Fetches incoming payments for all monitored rewards addresses
    /// First retrieves cached payments from database (if available), then fetches new ones from RPC.
    /// To avoid overwhelming the RPC server, processing is capped at MAX_BLOCKS_PER_FETCH_CALL
    /// blocks per call. The second element of the returned tuple indicates whether all blocks
    /// up to the current chain tip have been processed (true = fully synced).
    ///
    /// # Errors
    /// Returns `ArbitrumClientError` if the RPC call fails
    pub async fn fetch_incoming_payments(
        &self,
    ) -> Result<(Vec<AddressPayments>, bool), ArbitrumClientError> {
        let provider = ProviderBuilder::new().connect_http(self.endpoint.clone());

        // Phase 1: collect cached payments and missing ranges per address
        let mut cached_per_addr: HashMap<Address, HashSet<PaymentRecord>> = HashMap::new();
        let mut missing_ranges: Vec<(u64, u64)> = Vec::new();

        let current_block = provider
            .get_block_number()
            .await
            .map_err(|e| ArbitrumClientError::RpcError(e.to_string()))?;
        let blocks_to_fetch = NUM_DAYS_TO_TRACK_EARNINGS * BLOCKS_PER_DAY;
        let default_from = current_block.saturating_sub(blocks_to_fetch);

        for address in &self.rewards_addresses {
            logging::log!("Collecting cached earnings for address: {address}");
            // Load cached payments
            let (cached, max_cached) = self
                .db_client
                .get_earnings(address, default_from)
                .await
                .unwrap_or_default();
            // Use last synced block+1 when available
            let requested_from = max_cached.unwrap_or(default_from);
            let requested_to = current_block;

            // determine missing ranges for this address
            if requested_from <= requested_to {
                missing_ranges.push((requested_from, requested_to));
            }

            cached_per_addr.insert(*address, cached);
        }

        // Merge overlapping missing ranges across addresses
        missing_ranges.sort_by_key(|(from, _)| *from);
        let mut merged_ranges: Vec<(u64, u64)> = Vec::new();
        for (s, e) in missing_ranges {
            if let Some(last) = merged_ranges.last_mut()
                && s <= last.1.saturating_add(1)
            {
                // overlap or contiguous
                last.1 = last.1.max(e);
                continue;
            }
            merged_ranges.push((s, e));
        }

        // Phase 2: fetch merged ranges once and split logs by recipient.
        // A rolling budget caps total blocks processed per call to throttle RPC usage.
        let transfer_signature = keccak256("Transfer(address,address,uint256)");
        // Pre-compute topic2 values for our reward addresses.
        // Filtering by topic2 server-side keeps response sizes small and avoids the
        // RPC server's 10k-log limit that would otherwise be hit when querying busy ranges.
        let addr_topics: Vec<B256> = self
            .rewards_addresses
            .iter()
            .map(|addr| addr.into_word())
            .collect();
        let mut remaining_budget = MAX_BLOCKS_PER_FETCH_CALL;
        let mut fully_synced = true;

        'outer: for (from_block, to_block) in merged_ranges {
            if remaining_budget == 0 {
                fully_synced = false;
                break;
            }

            logging::log!("Fetching earnings history from block #{from_block}-#{to_block} ...");
            // Filter by topic2 (recipient) so the RPC server returns only transfers
            // to our reward addresses, staying well within the 10k-log response limit.
            let filter = Filter::new()
                .address(self.contract_address)
                .event_signature(transfer_signature)
                .topic2(addr_topics.clone())
                .from_block(BlockNumberOrTag::Number(from_block))
                .to_block(BlockNumberOrTag::Number(to_block));

            match self
                .get_logs_chunked(&provider, filter, &mut cached_per_addr, remaining_budget)
                .await
            {
                Ok((blocks_processed, completed)) => {
                    remaining_budget = remaining_budget.saturating_sub(blocks_processed);
                    if !completed {
                        fully_synced = false;
                        break 'outer;
                    }
                }
                Err(err) => {
                    logging::error!(
                        "[ERROR] Failed to fetch merged logs for range {from_block}-{to_block}: {err}"
                    );
                    fully_synced = false;
                }
            }
        }

        // Phase 3: assemble final per-address payments
        let all_payments = cached_per_addr
            .into_iter()
            .map(|(address, payments)| AddressPayments {
                address: address.to_string(),
                payments: payments.into_iter().collect(),
                last_updated: Utc::now(),
            })
            .collect();

        Ok((all_payments, fully_synced))
    }

    // Fetch logs in chunks of GET_LOGS_CHUNK_SIZE blocks, up to max_blocks total.
    // Returns (blocks_processed, completed) where completed=true means the full
    // range was consumed (i.e. we were not stopped by the max_blocks budget).
    async fn get_logs_chunked(
        &self,
        provider: &impl Provider,
        base_filter: Filter,
        cached_per_addr: &mut HashMap<Address, HashSet<PaymentRecord>>,
        max_blocks: u64,
    ) -> Result<(u64, bool), ArbitrumClientError> {
        let from_block = match base_filter.get_from_block() {
            Some(n) => n,
            _ => return Err(ArbitrumClientError::ParseError("Invalid from_block".into())),
        };

        let to_block = provider
            .get_block_number()
            .await
            .map_err(|e| ArbitrumClientError::RpcError(e.to_string()))?;

        let mut current = from_block;
        let mut latest_cached_bn = 0u64;
        let mut blocks_fetched = 0u64;

        while current <= to_block && blocks_fetched < max_blocks {
            let remaining_budget = max_blocks - blocks_fetched;
            let end = (current + GET_LOGS_CHUNK_SIZE.min(remaining_budget) - 1).min(to_block);

            let filter = base_filter
                .clone()
                .from_block(BlockNumberOrTag::Number(current))
                .to_block(BlockNumberOrTag::Number(end));

            match provider.get_logs(&filter).await {
                Ok(logs) => {
                    // cache in db
                    self.cache_logs(logs, provider, cached_per_addr).await;
                    latest_cached_bn = end;
                    blocks_fetched += end - current + 1;
                    current = end + 1;
                    // Throttle to avoid overwhelming the RPC server
                    sleep(INTER_CHUNK_DELAY).await;
                }
                Err(err) => {
                    logging::error!("[ERROR] Failed to get blocks from {current} to {end}: {err}");
                    break;
                }
            }
        }

        if latest_cached_bn > 0 {
            for addr in &self.rewards_addresses {
                self.db_client
                    .store_earnings(addr, U256::ZERO, latest_cached_bn, Utc::now().timestamp())
                    .await;
            }
            logging::log!(
                "Successfully cached earnings history in DB up to block #{latest_cached_bn}"
            );
        }

        let completed = current > to_block;
        Ok((blocks_fetched, completed))
    }

    async fn cache_logs(
        &self,
        logs: Vec<Log>,
        provider: &impl Provider,
        cached_per_addr: &mut HashMap<Address, HashSet<PaymentRecord>>,
    ) {
        // Pre-filter logs to only those relevant to monitored addresses, and collect
        // unique block numbers so we fetch each block's timestamp only once.
        struct PendingLog {
            recipient: Address,
            block_number: u64,
            amount: U256,
        }
        let mut pending: Vec<PendingLog> = Vec::new();
        let mut unique_blocks: HashSet<u64> = HashSet::new();

        for log in logs {
            // parse recipient from topic[2]
            let recipient_addr = match log.topics().get(2) {
                Some(topic) => {
                    let b: &[u8] = topic.as_ref();
                    Address::from_slice(&b[12..])
                }
                None => continue,
            };

            if !self.rewards_addresses.contains(&recipient_addr) {
                continue;
            }

            let block_number = match log.block_number {
                Some(bn) => bn,
                None => continue,
            };

            let amount = if log.data().data.is_empty() {
                U256::ZERO
            } else {
                U256::from_be_slice(&log.data().data)
            };

            unique_blocks.insert(block_number);
            pending.push(PendingLog {
                recipient: recipient_addr,
                block_number,
                amount,
            });
        }

        // Fetch each unique block once to get its timestamp.
        let mut block_timestamps: HashMap<u64, DateTime<Utc>> = HashMap::new();
        for bn in unique_blocks {
            let block = match provider
                .get_block_by_number(BlockNumberOrTag::Number(bn))
                .await
            {
                Ok(Some(b)) => b,
                _ => continue,
            };
            if let Some(ts) = Utc.timestamp_opt(block.header.timestamp as i64, 0).single() {
                block_timestamps.insert(bn, ts);
            }
        }

        // Now process each pending log using the cached timestamps.
        for PendingLog {
            recipient,
            block_number,
            amount,
        } in pending
        {
            let timestamp = match block_timestamps.get(&block_number) {
                Some(ts) => *ts,
                None => continue,
            };

            self.db_client
                .store_earnings(&recipient, amount, block_number, timestamp.timestamp())
                .await;

            cached_per_addr
                .entry(recipient)
                .or_default()
                .insert(PaymentRecord {
                    timestamp,
                    amount,
                    block_number,
                });
        }
    }
}
