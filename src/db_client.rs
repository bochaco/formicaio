use super::{
    metrics::{Metrics, NodeMetric},
    node_instance::{ContainerId, NodeInstanceInfo},
};

use alloy::primitives::U256;
use leptos::*;
use serde::{Deserialize, Serialize};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::SqlitePool,
    FromRow, QueryBuilder, Row, Sqlite,
};
use std::{
    env::{self, current_dir},
    path::Path,
    str::FromStr,
    sync::Arc,
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

// Struct stored on the DB caching nodes metadata.
#[derive(Clone, Debug, Deserialize, FromRow, Serialize)]
pub struct CachedNodeMetadata {
    pub container_id: String,
    pub peer_id: String,
    pub bin_version: String,
    pub port: u16,
    pub rpc_api_port: u16,
    pub rewards: String,
    pub balance: String,
    pub records: String,
    pub connected_peers: String,
    pub kbuckets_peers: String,
}

impl CachedNodeMetadata {
    // Update the node info with data obtained from DB, but only those
    // fields with non zero/empty values; zero/empty value means it was unknown when stored.
    pub fn merge_onto(&self, info: &mut NodeInstanceInfo) {
        if !self.peer_id.is_empty() {
            info.peer_id = Some(self.peer_id.clone());
        }
        if !self.bin_version.is_empty() {
            info.bin_version = Some(self.bin_version.clone());
        }
        if self.port > 0 {
            info.port = Some(self.port);
        }
        if self.rpc_api_port > 0 {
            info.rpc_api_port = Some(self.rpc_api_port);
        }
        if !self.rewards.is_empty() {
            if let Ok(v) = U256::from_str(&self.rewards) {
                info.rewards = Some(v.into());
            }
        }
        if !self.balance.is_empty() {
            if let Ok(v) = U256::from_str(&self.balance) {
                info.balance = Some(v.into());
            }
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
                    return Err(err.into());
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

    // Insert node metadata onto local cache DB
    pub async fn insert_node_metadata(&self, info: &NodeInstanceInfo) {
        let db_lock = self.db.lock().await;
        let query_str = format!(
            "INSERT OR REPLACE INTO nodes (\
                container_id, port, \
                rpc_api_port, records, \
                connected_peers, kbuckets_peers \
            ) VALUES (?, ?, ?, ?, ?, ?)"
        );

        match sqlx::query(&query_str)
            .bind(info.container_id.clone())
            .bind(info.port.clone())
            .bind(info.rpc_api_port.clone())
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
    pub async fn update_node_metadata(&self, info: &NodeInstanceInfo) {
        let db_lock = self.db.lock().await;

        let mut updates = Vec::new();
        let mut params = Vec::new();

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

        match query.execute(&*db_lock).await {
            Ok(_) => {}
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
        let db_lock = self.db.lock().await;
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

        match query.execute(&*db_lock).await {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite update query error: {err}"),
        }
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
        let db_lock = self.db.lock().await;
        let mut query_builder =
            QueryBuilder::new("INSERT INTO nodes_metrics (container_id, timestamp, key, value) ");

        query_builder.push_values(metrics, |mut b, metric| {
            b.push_bind(container_id.clone())
                .push_bind(metric.timestamp)
                .push_bind(metric.key.clone())
                .push_bind(metric.value.clone());
        });

        match query_builder.build().execute(&*db_lock).await {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite insert query error: {err}"),
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
}
