use super::node_instance::{NodeId, NodeInstanceInfo, NodePid};

use leptos::logging;
use std::{
    collections::{HashMap, HashSet},
    env,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use thiserror::Error;
use tokio::sync::Mutex;

// Name of the node binary use to launch new nodes processes
const NODE_BIN_NAME: &str = "antnode";

const DEFAULT_EVM_NETWORK: &str = "evm-arbitrum-sepolia";
const ROOT_DIR: &str = "NODE_MGR_ROOT_DIR";
const DEFAULT_ROOT_FOLDER: &str = "formicaio_data";
const DEFAULT_NODE_DATA_FOLDER: &str = "node_data";
const DEFAULT_LOGS_FOLDER: &str = "logs";
const DEFAULT_BOOTSTRAP_CACHE_FOLDER: &str = "bootstrap_cache";

#[derive(Debug, Error)]
pub enum NodeManagerError {
    #[error("Failed to create a new node: {0}")]
    CannotCreateNode(String),
    #[error("Node not found with id: {0}")]
    NodeNotFound(NodeId),
    #[error("Missing '{0}' information to spawn node")]
    SpawnNodeMissingParam(String),
    /*
        #[error(transparent)]
        StdIoError(#[from] std::io::Error),
        #[error("System info error with code {0}: {1}")]
        SystemInfoError(u16, String),
    */
}

// Execution and management of nodes as native OS processes
#[derive(Clone, Debug)]
pub struct NodeManager {
    root_dir: PathBuf,
    system: Arc<Mutex<System>>,
    nodes: Arc<Mutex<HashMap<NodeId, Child>>>,
}

impl Default for NodeManager {
    fn default() -> Self {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            panic!("This OS isn't supported by our 'sysinfo' dependency which manages the nodes as native processes.");
        }

        let root_dir = match env::var(ROOT_DIR) {
            Ok(v) => Path::new(&v).to_path_buf(),
            Err(_) => env::current_dir().unwrap().join(DEFAULT_ROOT_FOLDER),
        }
        .to_path_buf();

        logging::log!("Node manager instantiated with root dir:: {root_dir:?}");

        let system = Arc::new(Mutex::new(System::new()));
        let nodes = Arc::new(Mutex::new(HashMap::default()));

        Self {
            root_dir,
            system,
            nodes,
        }
    }
}

impl NodeManager {
    pub async fn spawn_new_node(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<(), NodeManagerError> {
        let node_id = &node_info.container_id;
        let port = node_info
            .port
            .ok_or(NodeManagerError::SpawnNodeMissingParam(
                "port number".to_string(),
            ))?;
        let metrics_port =
            node_info
                .metrics_port
                .ok_or(NodeManagerError::SpawnNodeMissingParam(
                    "metrics port number".to_string(),
                ))?;
        let rewards_address =
            node_info
                .rewards_addr
                .clone()
                .ok_or(NodeManagerError::SpawnNodeMissingParam(
                    "rewards address".to_string(),
                ))?;
        let home_network = node_info.home_network;

        let node_bin_path = self.root_dir.join(NODE_BIN_NAME);
        let node_data_dir = self.root_dir.join(DEFAULT_NODE_DATA_FOLDER).join(node_id);
        let bootstrap_cache_dir = self.root_dir.join(DEFAULT_BOOTSTRAP_CACHE_FOLDER);
        let log_output_dir = self
            .root_dir
            .join(DEFAULT_NODE_DATA_FOLDER)
            .join(node_id)
            .join(DEFAULT_LOGS_FOLDER);

        // TODO:
        // "if [ -e '/app/node_data/secret-key-recycle' ]; then rm -f /app/node_data/secret-key*; fi \

        let mut args = if home_network {
            vec!["--home-network".to_string()]
        } else {
            vec![]
        };

        args.push("--port".to_string());
        args.push(port.to_string());

        args.push("--metrics-server-port".to_string());
        args.push(metrics_port.to_string());

        args.push("--root-dir".to_string());
        args.push(node_data_dir.display().to_string());

        args.push("--log-output-dest".to_string());
        args.push(log_output_dir.display().to_string());

        args.push("--bootstrap-cache-dir".to_string());
        args.push(bootstrap_cache_dir.display().to_string());

        args.push("--rewards-address".to_string());
        args.push(rewards_address.to_string());

        args.push(DEFAULT_EVM_NETWORK.to_string());

        let mut command = Command::new(node_bin_path);
        command.args(args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.current_dir(&self.root_dir);

        logging::log!(">>> RUNNING: {command:?}");
        // Run the node
        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                logging::log!("Node process spawned with PID: {pid}");
                self.nodes.lock().await.insert(node_id.to_string(), child);
                node_info.pid = Some(pid);
                Ok(())
            }
            Err(err) => {
                logging::error!("Failed to create new node: {err:?}");
                Err(NodeManagerError::CannotCreateNode(err.to_string()))
            }
        }
    }

    pub async fn get_active_nodes_list(&self) -> Result<HashSet<NodePid>, NodeManagerError> {
        // first update processes information of our `System` struct
        let mut sys = self.system.lock().await;
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        // Display processes ID, name na disk usage:
        let mut pids = HashSet::default();
        for process in sys
            .processes_by_exact_name(NODE_BIN_NAME.as_ref())
            // filter out threads
            .filter(|p| p.thread_kind().is_none())
        {
            println!(
                "Process: {:?} (PID: {}) {}",
                process.name(),
                process.pid(),
                process.status()
            );
            pids.insert(process.pid().as_u32());
        }

        Ok(pids)
    }

    pub async fn kill_node(&self, node_id: &NodeId) -> Result<(), NodeManagerError> {
        let mut child = self
            .nodes
            .lock()
            .await
            .remove(node_id)
            .ok_or(NodeManagerError::NodeNotFound(node_id.clone()))?;

        match child.kill() {
            Ok(()) => {
                logging::log!(">>> KILLED!!");
            }
            Err(err) => {
                logging::warn!(">>> child couldn't be killed: {err:?}");
            }
        }

        let output = child.wait_with_output();
        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    logging::log!(">>> Output: {}", stdout);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    logging::error!(">>> Error: {}", stderr);
                }
            }
            Err(err) => {
                logging::warn!(">>> Failed to wait on child: {err:?}");
            }
        }

        Ok(())
    }
}
