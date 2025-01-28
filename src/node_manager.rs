use super::node_instance::{NodeId, NodeInstanceInfo, NodePid};

use leptos::logging;
use std::{
    collections::{HashMap, HashSet},
    env,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::Arc,
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use thiserror::Error;
use tokio::sync::Mutex;

// FIXME!!!: these two binaries need to be installed when app is first started.
// Name of the node binary used to launch new nodes processes
const NODE_BIN_NAME: &str = "antnode";
// Name of the binary used to upgrade the node binary
const INSTALLER_BIN_NAME: &str = "antup";

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
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
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
                match self
                    .get_node_version_and_peer_id(&node_info.container_id, !home_network)
                    .await
                {
                    Ok((bin_version, peer_id, ips)) => {
                        node_info.bin_version = bin_version;
                        node_info.peer_id = peer_id;
                        node_info.ips = ips;
                    }
                    Err(err) => {
                        logging::error!("Failed to obtain node bin version and peer id: {err:?}")
                    }
                }

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
            logging::log!(
                ">>> Process: {:?} (PID: {}) {}",
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

    // Retrieve version of the node binary and its peer id
    async fn get_node_version_and_peer_id(
        &self,
        id: &NodeId,
        get_ips: bool,
    ) -> Result<(Option<String>, Option<String>, Option<String>), NodeManagerError> {
        /* TODO:
        let cmd = "/app/antnode --version | grep -oE 'Autonomi Node v[0-9]+\\.[0-9]+\\.[0-9]+.*$'"
            .to_string();
        let (_, resp_str) = self
            .exec_in_container(id, cmd, "get node bin version")
            .await?;

        let version = resp_str
            .strip_prefix("Autonomi Node v")
            .map(|v| v.replace(['\n', '\r'], ""));
        logging::log!("Node bin version in container {id}: {version:?}");

        let cmd = "cat node_data/secret-key | od -A n -t x1 | tr -d ' \n'".to_string();
        let (_, resp_str) = self.exec_in_container(id, cmd, "get node peer id").await?;

        let peer_id = if let Ok(keypair) =
            libp2p_identity::Keypair::ed25519_from_bytes(hex::decode(resp_str).unwrap_or_default())
        {
            Some(libp2p_identity::PeerId::from(keypair.public()).to_string())
        } else {
            None
        };
        logging::log!("Node peer id in container {id}: {peer_id:?}");
        */

        //let ips = if get_ips {
        //    let cmd = "hostname -I | sed 's/^[ \t]*//;s/[ \t]*$//;s/ /, /g'".to_string();
        //    let (_, ips) = self
        //        .exec_in_container(id, cmd, "get node network IPs")
        //        .await?;
        //    logging::log!("Node IPs in container {id}: {ips}");
        //    Some(ips)
        //} else {
        //    None
        //};

        //Ok((version, peer_id, ips))

        Ok((
            Some("0.1.1".to_string()),
            Some("12D3KooWQCWUNjFmA5Azkp3jSxBRf796x3cyKF6V7FviUBCxXp31".to_string()),
            None,
        ))
    }

    // Upgrade the binary of given node
    pub async fn upgrade_node(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<(), NodeManagerError> {
        logging::log!("[UPGRADE] UPGRADE node ...");

        // restart container to run with new node version
        let _res = self.kill_node(&node_info.container_id).await;
        self.spawn_new_node(node_info).await?;

        Ok(())
    }

    // Upgrade the binary of the node binary used for new nodes to be spawned
    pub async fn upgrade_node_binary(&self) -> Result<(), NodeManagerError> {
        let installer_bin_path = self.root_dir.join(INSTALLER_BIN_NAME);
        let node_bin_path = self.root_dir.clone();
        let _output = self.exec_cmd(
            Command::new(installer_bin_path.display().to_string())
                .arg("node")
                .arg("-p")
                .arg(node_bin_path.display().to_string()),
            "upgrade node binary",
        )?;

        Ok(())
    }

    // Clears the node's PeerId and restarts it
    pub async fn regenerate_peer_id_in_container(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<(), NodeManagerError> {
        logging::log!("[RECYCLE] Recycling node by clearing its peer-id ...");

        // we remove 'secret-key' file so the node will re-generate it when restarted.
        let file_path = self
            .root_dir
            .join(DEFAULT_NODE_DATA_FOLDER)
            .join(node_info.container_id.clone())
            .join("secret-key");
        let _output = self.exec_cmd(
            Command::new("rm")
                .arg("-f")
                .arg(file_path.display().to_string()),
            "regenerate node's peer-id",
        )?;

        // restart node to obtain a new peer-id
        let _res = self.kill_node(&node_info.container_id).await;
        self.spawn_new_node(node_info).await?;

        logging::log!("Finished recycling node: {}", node_info.container_id);

        Ok(())
    }

    // Helper to execute a cmd
    fn exec_cmd(&self, cmd: &mut Command, desc: &str) -> Result<Output, NodeManagerError> {
        let output = cmd.output()?;
        if !output.status.success() {
            logging::error!("Failed to execute command to {desc}: {output:?}");
        }
        Ok(output)
    }
}
