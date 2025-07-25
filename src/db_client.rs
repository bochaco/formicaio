use super::types::{
    AppSettings, Metrics, NodeId, NodeInstanceInfo, NodeMetric, NodePid, NodeStatus,
};

use alloy_primitives::U256;
use leptos::{logging, prelude::*};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{
    FromRow, QueryBuilder, Row, Sqlite,
    migrate::{MigrateDatabase, Migrator},
    sqlite::SqlitePool,
};
use std::{
    collections::HashMap,
    env::{self, current_dir},
    path::{Path, PathBuf},
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
    node_id: String,
    pid: u32,
    created: u64,
    status_changed: u64,
    status: String,
    is_status_locked: bool,
    is_status_unknown: bool,
    peer_id: String,
    bin_version: String,
    ip_addr: String,
    port: u16,
    metrics_port: u16,
    rewards: String,
    balance: String,
    rewards_addr: String,
    home_network: bool,
    upnp: bool,
    node_logs: bool,
    records: String,
    connected_peers: String,
    kbuckets_peers: String,
    ips: String,
    data_dir_path: String,
}

impl CachedNodeMetadata {
    // Update the node info with data obtained from DB, but only those
    // fields with non zero/empty values; zero/empty value means it was unknown when stored.
    fn merge_onto(&self, info: &mut NodeInstanceInfo, get_status: bool) {
        if !self.node_id.is_empty() {
            info.node_id = self.node_id.clone();
        }
        if self.pid > 0 {
            info.pid = Some(self.pid);
        }
        if self.created > 0 {
            info.created = self.created;
        }
        if self.status_changed > 0 {
            info.status_changed = self.status_changed;
        }
        if get_status {
            if let Ok(status) = serde_json::from_str(&self.status) {
                info.status = status;
            }
        }
        info.is_status_locked = self.is_status_locked;
        info.is_status_unknown = self.is_status_unknown;
        if !self.peer_id.is_empty() {
            info.peer_id = Some(self.peer_id.clone());
        }
        if !self.bin_version.is_empty() {
            info.bin_version = Some(self.bin_version.clone());
        }
        if let Ok(v) = self.ip_addr.parse() {
            info.node_ip = Some(v);
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
        info.upnp = self.upnp;
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
        if !self.data_dir_path.is_empty() {
            info.data_dir_path = Some(PathBuf::from(&self.data_dir_path));
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
    pub async fn connect(data_dir_path: Option<PathBuf>) -> Result<Self, sqlx::Error> {
        let db_path = if let Some(path) = data_dir_path {
            path
        } else {
            match env::var(DB_PATH) {
                Ok(v) => Path::new(&v).to_path_buf(),
                Err(_) => Path::new(DEFAULT_DB_PATH).to_path_buf(),
            }
        };

        // Sqlite DB URL to connect to with sqlx.
        let sqlite_db_url = format!("sqlite:{}", db_path.join(SQLITE_DB_FILENAME).display());

        if !Sqlite::database_exists(&sqlite_db_url)
            .await
            .unwrap_or(false)
        {
            logging::log!("Creating database at: {sqlite_db_url}");
            match Sqlite::create_database(&sqlite_db_url).await {
                Ok(()) => logging::log!("Database created successfully!"),
                Err(err) => {
                    logging::error!("[ERROR] Failed to create database: {err}");
                    return Err(err);
                }
            }
        }

        let db = SqlitePool::connect(&sqlite_db_url).await?;

        let migrations = current_dir()?.join("migrations");
        logging::log!("Applying database migrations from: {migrations:?} ...");
        Migrator::new(migrations).await?.run(&db).await?;

        logging::log!("Database migrations completed successfully!");
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
                    node.merge_onto(&mut node_info, true);
                    retrieved_nodes.insert(node.node_id, node_info);
                }
            }
            Err(err) => {
                logging::error!("[ERROR] Database query error while retrieving nodes: {err}")
            }
        }

        retrieved_nodes
    }

    // Retrieve node metadata from local cache DB
    pub async fn get_node_metadata(&self, info: &mut NodeInstanceInfo, get_status: bool) {
        let db_lock = self.db.lock().await;
        match sqlx::query_as::<_, CachedNodeMetadata>(
            "SELECT * FROM nodes WHERE node_id LIKE ? || '%'",
        )
        .bind(info.node_id.clone())
        .fetch_one(&*db_lock)
        .await
        {
            Ok(node) => node.merge_onto(info, get_status),
            Err(err) => logging::error!(
                "[ERROR] Database query error while retrieving node metadata: {err}"
            ),
        }
    }

    // Check the node is not part of a batch, i.e. is not in Locked state
    pub async fn check_node_is_not_batched(
        &self,
        node_id: &NodeId,
    ) -> Result<NodeInstanceInfo, ServerFnError> {
        let mut node_info = NodeInstanceInfo::new(node_id.clone());
        self.get_node_metadata(&mut node_info, true).await;
        if node_info.is_status_locked {
            return Err(ServerFnError::new(
                "Node is part of a running/scheduled batch".to_string(),
            ));
        }
        Ok(node_info)
    }

    // Retrieve node binary version from local cache DB
    pub async fn get_node_bin_version(&self, node_id: &str) -> Option<String> {
        let db_lock = self.db.lock().await;
        match sqlx::query("SELECT bin_version FROM nodes WHERE node_id LIKE ? || '%'")
            .bind(node_id)
            .fetch_one(&*db_lock)
            .await
        {
            Ok(r) => {
                let v: String = r.get("bin_version");
                if v.is_empty() { None } else { Some(v) }
            }
            Err(err) => {
                logging::error!(
                    "[ERROR] Database query error while retrieving node binary version: {err}"
                );
                None
            }
        }
    }

    // Retrieve the list of nodes which have a binary version not matching the provided version
    pub async fn get_outdated_nodes_list(
        &self,
        version: &Version,
    ) -> Result<Vec<(NodeId, Version)>, DbError> {
        let db_lock = self.db.lock().await;
        let data = sqlx::query(
            "SELECT node_id, bin_version FROM nodes WHERE status = ? AND bin_version != ?",
        )
        .bind(json!(NodeStatus::Active).to_string())
        .bind(version.to_string())
        .fetch_all(&*db_lock)
        .await?;

        let version = data
            .iter()
            .filter_map(|v| {
                let current = v
                    .get::<String, _>("bin_version")
                    .parse()
                    // if we cannot parse it, let's assume it as outdated
                    // and return it with v0.0.0, so it can be corrected
                    // by the caller if desirable (e.g. by upgrading it).
                    .unwrap_or(Version::new(0, 0, 0));

                if &current < version {
                    Some((v.get("node_id"), current))
                } else {
                    None
                }
            })
            .collect();

        Ok(version)
    }

    // Insert node metadata onto local cache DB
    pub async fn insert_node_metadata(&self, info: &NodeInstanceInfo) {
        let query_str = "INSERT OR REPLACE INTO nodes (\
                node_id, created, status_changed, status, \
                is_status_locked, is_status_unknown, \
                ip_addr, port, metrics_port, rewards_addr, \
                home_network, upnp, node_logs, \
                records, connected_peers, kbuckets_peers, \
                data_dir_path \
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            .to_string();

        let db_lock = self.db.lock().await;
        match sqlx::query(&query_str)
            .bind(info.node_id.clone())
            .bind(info.created.to_string())
            .bind(info.status_changed.to_string())
            .bind(json!(info.status).to_string())
            .bind(info.is_status_locked)
            .bind(info.is_status_unknown)
            .bind(info.node_ip.map_or("".to_string(), |v| v.to_string()))
            .bind(info.port)
            .bind(info.metrics_port)
            .bind(info.rewards_addr.clone())
            .bind(info.home_network)
            .bind(info.upnp)
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
            .bind(
                info.data_dir_path
                    .clone()
                    .map_or("".to_string(), |v| v.display().to_string()),
            )
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                logging::error!("[ERROR] Database insert error while storing node metadata: {err}")
            }
        }
    }

    // Update node metadata
    pub async fn update_node_metadata(&self, info: &NodeInstanceInfo, update_status: bool) {
        let mut updates = Vec::new();
        let mut params = Vec::new();

        if update_status {
            updates.push("status=?");
            params.push(json!(info.status).to_string());
        }

        if info.status_changed > 0 {
            updates.push("status_changed=?");
            params.push(info.status_changed.to_string());
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
        if let Some(data_dir_path) = &info.data_dir_path {
            updates.push("data_dir_path=?");
            params.push(data_dir_path.clone().display().to_string());
        }

        if updates.is_empty() {
            return; // no updates to make
        }

        let query_str = format!(
            "UPDATE nodes SET {} WHERE node_id LIKE ? || '%'",
            updates.join(", ")
        );
        params.push(info.node_id.clone());

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
            Err(err) => {
                logging::error!("[ERROR] Database update error while updating node metadata: {err}")
            }
        }
    }

    // Remove node metadata from local cache DB
    pub async fn delete_node_metadata(&self, node_id: &str) {
        let db_lock = self.db.lock().await;
        match sqlx::query("DELETE FROM nodes WHERE node_id LIKE ? || '%'")
            .bind(node_id)
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                logging::error!("[ERROR] Database delete error while removing node metadata: {err}")
            }
        }
    }

    // Update node metadata onto local cache DB by specifying specific fields and new values
    async fn update_node_metadata_fields(&self, node_id: &str, fields_values: &[(&str, &str)]) {
        let (updates, mut params) =
            fields_values
                .iter()
                .fold((vec![], vec![]), |(mut u, mut p), (field, param)| {
                    u.push(format!("{field}=?"));
                    p.push(*param);
                    (u, p)
                });
        params.push(node_id);

        let query_str = format!(
            "UPDATE nodes SET {} WHERE node_id LIKE ? || '%'",
            updates.join(", ")
        );

        let mut query = sqlx::query(&query_str);
        for p in params {
            query = query.bind(p);
        }

        let db_lock = self.db.lock().await;
        match query.execute(&*db_lock).await {
            Ok(_) => {}
            Err(err) => {
                logging::error!("[ERROR] Database error while updating node record fields: {err}")
            }
        }
    }

    // Convenient method to update node status field
    pub async fn update_node_status(&self, node_id: &str, status: &NodeStatus) {
        self.update_node_metadata_fields(node_id, &[("status", &json!(status).to_string())])
            .await
    }

    // Convenient method to lock node status field and status-changed timestamp
    pub async fn set_node_status_to_locked(&self, node_id: &str) {
        self.update_node_metadata_fields(node_id, &[("is_status_locked", "1")])
            .await
    }

    // Convenient method to unlock node status and set it to its previous status
    pub async fn unlock_node_status(&self, node_id: &str) {
        self.update_node_metadata_fields(node_id, &[("is_status_locked", "0")])
            .await
    }

    // Convenient method to update node balance field
    pub async fn update_node_balance(&self, node_id: &str, balance: &str) {
        self.update_node_metadata_fields(node_id, &[("balance", balance)])
            .await
    }

    // Explicit method to update node PID so the caller is aware the PID
    // will be changed, otherwise it could cause problems if updated from the incorrect flow.
    pub async fn update_node_pid(&self, node_id: &str, pid: NodePid) {
        self.update_node_metadata_fields(node_id, &[("pid", &pid.to_string())])
            .await
    }

    // Retrieve node metrics from local cache DB
    pub async fn get_node_metrics(&self, node_id: NodeId, since: Option<i64>) -> Metrics {
        let db_lock = self.db.lock().await;
        let mut node_metrics = Metrics::new();

        match sqlx::query(
            "SELECT * FROM nodes_metrics WHERE node_id LIKE ? || '%' AND timestamp > ? ORDER BY timestamp",
        )
        .bind(node_id.clone())
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
            Err(err) => logging::error!("[ERROR] Database query error while retrieving node metrics: {err}"),
        }

        node_metrics
    }

    // Store node metrics onto local cache DB
    pub async fn store_node_metrics(
        &self,
        node_id: NodeId,
        metrics: impl IntoIterator<Item = &NodeMetric>,
    ) {
        let metrics = metrics.into_iter().collect::<Vec<_>>();
        if metrics.is_empty() {
            return;
        }

        let mut query_builder =
            QueryBuilder::new("INSERT INTO nodes_metrics (node_id, timestamp, key, value) ");

        query_builder.push_values(metrics, |mut b, metric| {
            b.push_bind(node_id.clone())
                .push_bind(metric.timestamp)
                .push_bind(metric.key.clone())
                .push_bind(metric.value.clone());
        });

        let db_lock = self.db.lock().await;
        match query_builder.build().execute(&*db_lock).await {
            Ok(_) => {}
            Err(err) => {
                logging::error!("[ERROR] Database insert error while storing node metrics: {err}.")
            }
        }
    }

    // Remove node metrics from local cache DB
    pub async fn delete_node_metrics(&self, node_id: &str) {
        let db_lock = self.db.lock().await;
        match sqlx::query("DELETE FROM nodes_metrics WHERE node_id LIKE ? || '%'")
            .bind(node_id)
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                logging::error!("[ERROR] Database delete error while removing node metrics: {err}")
            }
        }
    }

    // Remove metrics for a node so there are no more than max_size records
    pub async fn remove_oldest_metrics(&self, node_id: NodeId, max_size: usize) {
        let db_lock = self.db.lock().await;
        match sqlx::query(
            "DELETE FROM nodes_metrics WHERE \
                node_id LIKE ? || '%' AND timestamp <= \
                    (SELECT DISTINCT timestamp \
                        FROM nodes_metrics \
                        WHERE node_id LIKE ? || '%' \
                        ORDER BY timestamp DESC \
                        LIMIT 1 OFFSET ? \
                    )",
        )
        .bind(node_id.clone())
        .bind(node_id)
        .bind(max_size as i64)
        .execute(&*db_lock)
        .await
        {
            Ok(res) => logging::log!("Removed {} metrics records", res.rows_affected()),
            Err(err) => {
                logging::error!("[ERROR] Database delete error while pruning old metrics: {err}")
            }
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
                logging::warn!(
                    "[WARN] Database query error while retrieving settings: {err}. We'll be using defaults."
                );
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
                logging::log!("Application settings updated successfully in database.");
                Ok(())
            }
            Err(err) => {
                logging::error!("[ERROR] Database error while updating settings: {err}");
                Err(err.into())
            }
        }
    }
}
