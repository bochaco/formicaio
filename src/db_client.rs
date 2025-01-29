use super::{
    metrics::{Metrics, NodeMetric},
    node_instance::{ContainerId, NodeId, NodeInstanceInfo, NodeStatus},
    server_api_types::AppSettings,
};

use alloy::primitives::U256;
use leptos::{logging, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::SqlitePool,
    FromRow, QueryBuilder, Row, Sqlite,
};
use std::{
    collections::HashMap,
    env::{self, current_dir},
    path::Path,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    SqlxError(#[from] sqlx::Error),
}

// Sqlite DB filename.
const SQLITE_DB_FILENAME: &str = "formicaio.db";
// Env var name to set the path for the DB file.
const DB_PATH: &str = "DB_PATH";
// Default path for the DB file.
const DEFAULT_DB_PATH: &str = "./";

// Struct stored on the DB with application settings.
#[derive(Clone, Debug, Deserialize, FromRow, Serialize)]
struct CachedSettings {
    nodes_auto_upgrade: bool,
    nodes_auto_upgrade_delay_secs: u64,
    node_bin_version_polling_freq_secs: u64,
    nodes_metrics_polling_freq_secs: u64,
    rewards_balances_retrieval_freq_secs: u64,
    l2_network_rpc_url: String,
    token_contract_address: String,
    lcd_display_enabled: bool,
    lcd_device: String,
    lcd_addr: String,
}

// Struct stored on the DB caching nodes metadata.
#[derive(Clone, Debug, Deserialize, FromRow, Serialize)]
struct CachedNodeMetadata {
    container_id: String,
    pid: u32,
    created: u64,
    status: String,
    peer_id: String,
    bin_version: String,
    port: u16,
    metrics_port: u16,
    rewards: String,
    balance: String,
    rewards_addr: String,
    home_network: bool,
    node_logs: bool,
    records: String,
    connected_peers: String,
    kbuckets_peers: String,
    ips: String,
}

impl CachedNodeMetadata {
    // Update the node info with data obtained from DB, but only those
    // fields with non zero/empty values; zero/empty value means it was unknown when stored.
    pub fn merge_onto(&self, info: &mut NodeInstanceInfo) {
        if !self.container_id.is_empty() {
            info.container_id = self.container_id.clone();
        }
        if self.pid > 0 {
            info.pid = Some(self.pid);
        }
        if self.created > 0 {
            info.created = self.created;
        }
        if let Ok(status) = serde_json::from_str(&self.status) {
            info.status = status;
        }
        if !self.peer_id.is_empty() {
            info.peer_id = Some(self.peer_id.clone());
        }
        if !self.bin_version.is_empty() {
            info.bin_version = Some(self.bin_version.clone());
        }
        if self.port > 0 {
            info.port = Some(self.port);
        }
        if self.metrics_port > 0 {
            info.metrics_port = Some(self.metrics_port);
        }
        if !self.rewards.is_empty() {
            if let Ok(v) = U256::from_str(&self.rewards) {
                info.rewards = Some(v);
            }
        }
        info.home_network = self.home_network;
        info.node_logs = self.node_logs;
        if !self.balance.is_empty() {
            if let Ok(v) = U256::from_str(&self.balance) {
                info.balance = Some(v);
            }
        }
        if !self.rewards_addr.is_empty() {
            info.rewards_addr = Some(self.rewards_addr.clone());
        }
        if let Ok(v) = self.records.parse::<usize>() {
            info.records = Some(v);
        }
        if let Ok(v) = self.connected_peers.parse::<usize>() {
            info.connected_peers = Some(v);
        }
        if let Ok(v) = self.kbuckets_peers.parse::<usize>() {
            info.kbuckets_peers = Some(v);
        }
        if !self.ips.is_empty() {
            info.ips = Some(self.ips.clone());
        }
    }
}

// Client to interface with the local Sqlite database
#[derive(Clone, Debug)]
pub struct DbClient {
    db: Arc<Mutex<SqlitePool>>,
}

impl DbClient {
    // Create a connection to local Sqlite DB where nodes metadata is cached.
    pub async fn connect() -> Result<Self, sqlx::Error> {
        let db_path = match env::var(DB_PATH) {
            Ok(v) => Path::new(&v).to_path_buf(),
            Err(_) => Path::new(DEFAULT_DB_PATH).to_path_buf(),
        };
        // Sqlite DB URL to connect to with sqlx.
        let sqlite_db_url = format!("sqlite:{}", db_path.join(SQLITE_DB_FILENAME).display());

        if !Sqlite::database_exists(&sqlite_db_url)
            .await
            .unwrap_or(false)
        {
            logging::log!("Creating database {sqlite_db_url}");
            match Sqlite::create_database(&sqlite_db_url).await {
                Ok(()) => logging::log!("Created database successfully!"),
                Err(err) => {
                    logging::log!("Failed to create database: {err}");
                    return Err(err);
                }
            }
        }

        let db = SqlitePool::connect(&sqlite_db_url).await?;

        let migrations = current_dir()?.join("migrations");
        logging::log!("Applying database migration scripts from: {migrations:?} ...");
        Migrator::new(migrations).await?.run(&db).await?;

        logging::log!("Database migrations applied successfully!");
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
        })
    }

    // Retrieve list of nodes from local cache DB
    pub async fn get_nodes_list(&self) -> HashMap<NodeId, NodeInstanceInfo> {
        let db_lock = self.db.lock().await;
        let mut retrieved_nodes = HashMap::default();
        match sqlx::query_as::<_, CachedNodeMetadata>("SELECT * FROM nodes")
            .fetch_all(&*db_lock)
            .await
        {
            Ok(nodes) => {
                for node in nodes {
                    let mut node_info = NodeInstanceInfo::default();
                    node.merge_onto(&mut node_info);
                    retrieved_nodes.insert(node.container_id, node_info);
                }
            }
            Err(err) => logging::log!("Sqlite query error: {err}"),
        }

        retrieved_nodes
    }

    // Retrieve node metadata from local cache DB
    pub async fn get_node_metadata(&self, info: &mut NodeInstanceInfo) {
        let db_lock = self.db.lock().await;
        match sqlx::query_as::<_, CachedNodeMetadata>("SELECT * FROM nodes WHERE container_id=?")
            .bind(info.container_id.clone())
            .fetch_all(&*db_lock)
            .await
        {
            Ok(nodes) => {
                for node in nodes {
                    if node.container_id == info.container_id {
                        node.merge_onto(info);
                    }
                }
            }
            Err(err) => logging::log!("Sqlite query error: {err}"),
        }
    }

    // Retrieve node binary version from local cache DB
    pub async fn get_node_bin_version(&self, container_id: &str) -> Option<String> {
        let db_lock = self.db.lock().await;
        match sqlx::query("SELECT bin_version FROM nodes WHERE container_id=?")
            .bind(container_id)
            .fetch_all(&*db_lock)
            .await
        {
            Ok(records) => records.first().and_then(|r| {
                let v: String = r.get("bin_version");
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            }),
            Err(err) => {
                logging::log!("Sqlite bin version query error: {err}");
                None
            }
        }
    }

    // Retrieve the list of nodes which have a binary version not matching the provided version
    // TODO: use semantic version to make the comparison.
    pub async fn get_outdated_nodes_list(
        &self,
        version: &str,
    ) -> Result<Vec<(ContainerId, String)>, DbError> {
        let db_lock = self.db.lock().await;
        let data = sqlx::query(
            "SELECT container_id, bin_version FROM nodes WHERE status = ? AND bin_version != ?",
        )
        .bind(json!(NodeStatus::Active).to_string())
        .bind(version)
        .fetch_all(&*db_lock)
        .await?;

        let version = data
            .iter()
            .map(|v| (v.get("container_id"), v.get("bin_version")))
            .collect();

        Ok(version)
    }

    // Insert node metadata onto local cache DB
    pub async fn insert_node_metadata(&self, info: &NodeInstanceInfo) {
        let query_str = "INSERT OR REPLACE INTO nodes (\
                container_id, created, status, \
                port, metrics_port, \
                rewards_addr, home_network, node_logs, \
                records, connected_peers, kbuckets_peers \
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            .to_string();

        let db_lock = self.db.lock().await;
        match sqlx::query(&query_str)
            .bind(info.container_id.clone())
            .bind(info.created.to_string())
            .bind(json!(info.status).to_string())
            .bind(info.port)
            .bind(info.metrics_port)
            .bind(info.rewards_addr.clone())
            .bind(info.home_network)
            .bind(info.node_logs)
            .bind(info.records.map_or("".to_string(), |v| v.to_string()))
            .bind(
                info.connected_peers
                    .map_or("".to_string(), |v| v.to_string()),
            )
            .bind(
                info.kbuckets_peers
                    .map_or("".to_string(), |v| v.to_string()),
            )
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite insert query error: {err}"),
        }
    }

    // Update node metadata on local cache DB
    pub async fn update_node_metadata(&self, info: &NodeInstanceInfo, update_status: bool) {
        let mut updates = Vec::new();
        let mut params = Vec::new();

        if update_status {
            updates.push("status=?");
            params.push(json!(info.status).to_string());
        }

        if let Some(pid) = &info.pid {
            updates.push("pid=?");
            params.push(pid.to_string());
        }
        if let Some(peer_id) = &info.peer_id {
            updates.push("peer_id=?");
            params.push(peer_id.clone());
        }
        if let Some(bin_version) = &info.bin_version {
            updates.push("bin_version=?");
            params.push(bin_version.clone());
        }
        if let Some(rewards) = &info.rewards {
            updates.push("rewards=?");
            params.push(rewards.to_string());
        }
        if let Some(balance) = &info.balance {
            updates.push("balance=?");
            params.push(balance.to_string());
        }
        if let Some(records) = &info.records {
            updates.push("records=?");
            params.push(records.to_string());
        }
        if let Some(connected_peers) = &info.connected_peers {
            updates.push("connected_peers=?");
            params.push(connected_peers.to_string());
        }
        if let Some(kbuckets_peers) = &info.kbuckets_peers {
            updates.push("kbuckets_peers=?");
            params.push(kbuckets_peers.to_string());
        }
        if let Some(ips) = &info.ips {
            updates.push("ips=?");
            params.push(ips.clone());
        }

        if updates.is_empty() {
            return; // no updates to make
        }

        let query_str = format!(
            "UPDATE nodes SET {} WHERE container_id=?",
            updates.join(", ")
        );
        params.push(info.container_id.clone());

        let mut query = sqlx::query(&query_str);
        for p in params {
            query = query.bind(p);
        }

        let db_lock = self.db.lock().await;
        match query.execute(&*db_lock).await {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    // insert a new record then
                    drop(db_lock);
                    self.insert_node_metadata(info).await;
                }
            }
            Err(err) => logging::log!("Sqlite update query error: {err}"),
        }
    }

    // Remove node metadata from local cache DB
    pub async fn delete_node_metadata(&self, container_id: &str) {
        let db_lock = self.db.lock().await;
        match sqlx::query("DELETE FROM nodes WHERE container_id = ?")
            .bind(container_id)
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite delete query error: {err}"),
        }
    }

    // Update node metadata onto local cache DB by specifying specific fields and new values
    pub async fn update_node_metadata_fields(
        &self,
        container_id: &str,
        fields_values: &[(&str, &str)],
    ) {
        let (updates, mut params) =
            fields_values
                .iter()
                .fold((vec![], vec![]), |(mut u, mut p), (field, param)| {
                    u.push(format!("{field}=?"));
                    p.push(*param);
                    (u, p)
                });
        params.push(container_id);

        let query_str = format!(
            "UPDATE nodes SET {} WHERE container_id=?",
            updates.join(", ")
        );

        let mut query = sqlx::query(&query_str);
        for p in params {
            query = query.bind(p);
        }

        let db_lock = self.db.lock().await;
        match query.execute(&*db_lock).await {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite update query error: {err}"),
        }
    }

    // Convenient method to update node status field on local cache DB
    pub async fn update_node_status(&self, container_id: &str, status: NodeStatus) {
        self.update_node_metadata_fields(container_id, &[("status", &json!(&status).to_string())])
            .await
    }

    // Retrieve node metrics from local cache DB
    pub async fn get_node_metrics(&self, container_id: ContainerId, since: Option<i64>) -> Metrics {
        let db_lock = self.db.lock().await;
        let mut node_metrics = Metrics::new();

        match sqlx::query(
            "SELECT * FROM nodes_metrics WHERE container_id = ? AND timestamp > ? ORDER BY timestamp",
        )
        .bind(container_id.clone())
        .bind(since.unwrap_or_default())
        .fetch_all(&*db_lock)
        .await
        {
            Ok(metrics) => {
                metrics.into_iter().for_each(|m| {
                    let key: String = m.get("key");
                    let entry = node_metrics.entry(key.clone()).or_default();
                    entry.push(NodeMetric {
                        timestamp: m.get("timestamp"),
                        key,
                        value: m.get("value"),
                    });
                });
            }
            Err(err) => logging::log!("Sqlite query error: {err}"),
        }

        node_metrics
    }

    // Store node metrics onto local cache DB
    pub async fn store_node_metrics(
        &self,
        container_id: ContainerId,
        metrics: impl IntoIterator<Item = &NodeMetric>,
    ) {
        let metrics = metrics.into_iter().collect::<Vec<_>>();
        if metrics.is_empty() {
            return;
        }

        let mut query_builder =
            QueryBuilder::new("INSERT INTO nodes_metrics (container_id, timestamp, key, value) ");

        query_builder.push_values(metrics, |mut b, metric| {
            b.push_bind(container_id.clone())
                .push_bind(metric.timestamp)
                .push_bind(metric.key.clone())
                .push_bind(metric.value.clone());
        });

        let db_lock = self.db.lock().await;
        match query_builder.build().execute(&*db_lock).await {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite insert query error: {err}."),
        }
    }

    // Remove node metrics from local cache DB
    pub async fn delete_node_metrics(&self, container_id: &str) {
        let db_lock = self.db.lock().await;
        match sqlx::query("DELETE FROM nodes_metrics WHERE container_id = ?")
            .bind(container_id)
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite delete query error: {err}"),
        }
    }

    // Remove metrics for a container so there are no more than max_size records
    pub async fn remove_oldest_metrics(&self, container_id: ContainerId, max_size: usize) {
        let db_lock = self.db.lock().await;
        match sqlx::query(
            "DELETE FROM nodes_metrics WHERE \
                container_id = ? AND timestamp <= \
                    (SELECT DISTINCT timestamp \
                        FROM nodes_metrics \
                        WHERE container_id = ? \
                        ORDER BY timestamp DESC \
                        LIMIT 1 OFFSET ? \
                    )",
        )
        .bind(container_id.clone())
        .bind(container_id)
        .bind(max_size as i64)
        .execute(&*db_lock)
        .await
        {
            Ok(res) => logging::log!("Removed {} metrics records", res.rows_affected()),
            Err(err) => logging::log!("Sqlite pruning query error: {err}"),
        }
    }

    // Retrieve the settings values
    pub async fn get_settings(&self) -> AppSettings {
        let db_lock = self.db.lock().await;
        match sqlx::query_as::<_, CachedSettings>("SELECT * FROM settings")
            .fetch_all(&*db_lock)
            .await
            .map(|s| s.first().cloned())
        {
            Ok(Some(s)) => AppSettings {
                nodes_auto_upgrade: s.nodes_auto_upgrade,
                nodes_auto_upgrade_delay: Duration::from_secs(s.nodes_auto_upgrade_delay_secs),
                node_bin_version_polling_freq: Duration::from_secs(
                    s.node_bin_version_polling_freq_secs,
                ),
                nodes_metrics_polling_freq: Duration::from_secs(s.nodes_metrics_polling_freq_secs),
                rewards_balances_retrieval_freq: Duration::from_secs(
                    s.rewards_balances_retrieval_freq_secs,
                ),
                l2_network_rpc_url: s.l2_network_rpc_url.clone(),
                token_contract_address: s.token_contract_address.clone(),
                lcd_display_enabled: s.lcd_display_enabled,
                lcd_device: s.lcd_device.clone(),
                lcd_addr: s.lcd_addr.clone(),
            },
            Ok(None) => {
                logging::log!("No settings found in DB, we'll be using defaults.");
                AppSettings::default()
            }
            Err(err) => {
                logging::log!("Sqlite query error on settings: {err}. We'll be using defaults.");
                AppSettings::default()
            }
        }
    }

    // Update the settings values
    pub async fn update_settings(&self, settings: &AppSettings) -> Result<(), DbError> {
        let db_lock = self.db.lock().await;
        match sqlx::query(
            "UPDATE settings SET \
            nodes_auto_upgrade = ?, \
            nodes_auto_upgrade_delay_secs = ?, \
            node_bin_version_polling_freq_secs = ?, \
            nodes_metrics_polling_freq_secs = ?, \
            rewards_balances_retrieval_freq_secs = ?, \
            l2_network_rpc_url = ?, \
            token_contract_address = ?, \
            lcd_display_enabled = ?, \
            lcd_device = ?, \
            lcd_addr = ?",
        )
        .bind(settings.nodes_auto_upgrade)
        .bind(settings.nodes_auto_upgrade_delay.as_secs() as i64)
        .bind(settings.node_bin_version_polling_freq.as_secs() as i64)
        .bind(settings.nodes_metrics_polling_freq.as_secs() as i64)
        .bind(settings.rewards_balances_retrieval_freq.as_secs() as i64)
        .bind(settings.l2_network_rpc_url.clone())
        .bind(settings.token_contract_address.clone())
        .bind(settings.lcd_display_enabled)
        .bind(settings.lcd_device.clone())
        .bind(settings.lcd_addr.clone())
        .execute(&*db_lock)
        .await
        {
            Ok(_) => {
                logging::log!("New app settings updated in DB cache.");
                Ok(())
            }
            Err(err) => {
                logging::log!("Sqlite settings update error: {err}");
                Err(err.into())
            }
        }
    }
}
