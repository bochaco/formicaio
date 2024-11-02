use super::node_instance::NodeInstanceInfo;

use leptos::*;
use serde::{Deserialize, Serialize};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::SqlitePool,
    FromRow, Sqlite,
};
use std::{
    env::{self, current_dir},
    path::Path,
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
    pub rewards: String, // TODO: currently unused, remove it
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
        if let Ok(v) = self.balance.parse::<u64>() {
            info.balance = Some(v);
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
    pub async fn get_node_metadata(&self, info: &mut NodeInstanceInfo) -> Result<(), DbError> {
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

        Ok(())
    }

    // Store node metadata (insert, or update if it exists) onto local cache DB
    pub async fn store_node_metadata(&self, info: &NodeInstanceInfo) -> Result<(), DbError> {
        let db_lock = self.db.lock().await;
        let bind_peer_and_bin = info.peer_id.is_some() && info.bin_version.is_some();
        let query_str = format!(
            "INSERT OR REPLACE INTO nodes (\
                container_id,{} port, \
                rpc_api_port, balance, records, \
                connected_peers, kbuckets_peers \
            ) VALUES (?,{} ?, ?, ?, ?, ?, ?)",
            if bind_peer_and_bin {
                "peer_id, bin_version,"
            } else {
                ""
            },
            if bind_peer_and_bin { "?, ?," } else { "" }
        );

        let mut query = sqlx::query(&query_str).bind(info.container_id.clone());

        if bind_peer_and_bin {
            query = query
                .bind(info.peer_id.clone())
                .bind(info.bin_version.clone().unwrap_or_default());
        }

        match query
            .bind(info.port.clone())
            .bind(info.rpc_api_port.clone())
            .bind(info.balance.map_or("".to_string(), |v| v.to_string()))
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
            Err(err) => logging::log!("Sqlite query error: {err}"),
        }

        Ok(())
    }

    // Remove node metadata from local cache DB
    pub async fn delete_node_metadata(&self, container_id: &str) -> Result<(), DbError> {
        let db_lock = self.db.lock().await;
        match sqlx::query("DELETE FROM nodes WHERE container_id = ?")
            .bind(container_id)
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite query error: {err}"),
        }

        Ok(())
    }

    // Update node metadata onto local cache DB by specifying specific field and new value
    pub async fn update_node_metadata_field(
        &self,
        container_id: &str,
        field: &str,
        value: &str,
    ) -> Result<(), DbError> {
        let db_lock = self.db.lock().await;
        match sqlx::query(&format!("UPDATE nodes SET {field}=? WHERE container_id=?"))
            .bind(value)
            .bind(container_id)
            .execute(&*db_lock)
            .await
        {
            Ok(_) => {}
            Err(err) => logging::log!("Sqlite query error: {err}"),
        }

        Ok(())
    }
}
