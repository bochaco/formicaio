use super::node_instance::NodeInstanceInfo;

use leptos::*;
use serde::{Deserialize, Serialize};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::SqlitePool,
    FromRow, Sqlite,
};
use std::env::current_dir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    SqlxError(#[from] sqlx::Error),
}

// Sqlite DB URL to connect to with sqlx.
const SQLITE_DB_URL: &str = "sqlite:formicaio.db";

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
    pub chunks: String,
    pub connected_peers: String,
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
        if let Ok(v) = self.rewards.parse::<u64>() {
            info.rewards = Some(v);
        }
        if let Ok(v) = self.balance.parse::<u64>() {
            info.balance = Some(v);
        }
        if let Ok(v) = self.chunks.parse::<u64>() {
            info.chunks = Some(v);
        }
        if let Ok(v) = self.connected_peers.parse::<usize>() {
            info.connected_peers = Some(v);
        }
    }
}

// Create a connection to local Sqlite DB where nodes metadata is cached.
async fn db_conn() -> Result<SqlitePool, sqlx::Error> {
    if !Sqlite::database_exists(SQLITE_DB_URL)
        .await
        .unwrap_or(false)
    {
        logging::log!("Creating database {SQLITE_DB_URL}");
        match Sqlite::create_database(SQLITE_DB_URL).await {
            Ok(()) => {
                logging::log!("Created database successfully!");
                let db = SqlitePool::connect(SQLITE_DB_URL).await?;

                let migrations = current_dir()?.join("migrations");
                logging::log!("Applying database migration scripts from: {migrations:?} ...");
                Migrator::new(migrations).await?.run(&db).await?;

                logging::log!("Created 'nodes' table successfully!");
                Ok(db)
            }
            Err(err) => {
                logging::log!("Failed to create database: {err}");
                return Err(err.into());
            }
        }
    } else {
        // database already exists
        SqlitePool::connect(SQLITE_DB_URL).await
    }
}

// Retrieve node metadata from local cache DB
pub async fn db_get_node_metadata(info: &mut NodeInstanceInfo) -> Result<(), DbError> {
    let db = db_conn().await?;
    match sqlx::query_as::<_, CachedNodeMetadata>(
        "SELECT container_id, peer_id, bin_version, port, \
                rpc_api_port, rewards, balance, chunks, connected_peers \
            FROM nodes WHERE container_id=?",
    )
    .bind(info.container_id.clone())
    .fetch_all(&db)
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

    Ok(())
}

// Store node metadata (insert, or update if it exists) onto local cache DB
pub async fn db_store_node_metadata(info: &NodeInstanceInfo) -> Result<(), DbError> {
    let db = db_conn().await?;
    match sqlx::query(
        "INSERT OR REPLACE INTO nodes (\
                container_id, peer_id, bin_version, port, \
                rpc_api_port, rewards, balance, chunks, connected_peers \
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(info.container_id.clone())
    .bind(info.peer_id.clone())
    .bind(info.bin_version.clone().unwrap_or_default())
    .bind(info.port.clone())
    .bind(info.rpc_api_port.clone())
    .bind(info.rewards.map_or("".to_string(), |v| v.to_string()))
    .bind(info.balance.map_or("".to_string(), |v| v.to_string()))
    .bind(info.chunks.map_or("".to_string(), |v| v.to_string()))
    .bind(
        info.connected_peers
            .map_or("".to_string(), |v| v.to_string()),
    )
    .execute(&db)
    .await
    {
        Ok(_) => {}
        Err(err) => logging::log!("Sqlite query error: {err}"),
    }

    Ok(())
}

// Remove node metadata from local cache DB
pub async fn db_delete_node_metadata(container_id: &str) -> Result<(), DbError> {
    let db = db_conn().await?;
    match sqlx::query("DELETE FROM nodes WHERE container_id = ?")
        .bind(container_id)
        .execute(&db)
        .await
    {
        Ok(_) => {}
        Err(err) => logging::log!("Sqlite query error: {err}"),
    }

    Ok(())
}

// Update node metadata onto local cache DB by specifying specific field and new value
pub async fn db_update_node_metadata_field(
    container_id: &str,
    field: &str,
    value: &str,
) -> Result<(), DbError> {
    let db = db_conn().await?;
    match sqlx::query(&format!("UPDATE nodes SET {field}=? WHERE container_id=?"))
        .bind(value)
        .bind(container_id)
        .execute(&db)
        .await
    {
        Ok(_) => {}
        Err(err) => logging::log!("Sqlite query error: {err}"),
    }

    Ok(())
}
