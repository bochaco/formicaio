use super::{
    app::ImmutableNodeStatus,
    node_instance::{InactiveReason, NodeId, NodeInstanceInfo, NodePid},
};

use ant_releases::{
    get_running_platform, AntReleaseRepoActions, AntReleaseRepository, ArchiveType, ReleaseType,
};
use bytes::Bytes;
use futures_util::Stream;
use leptos::logging;
use libp2p_identity::{Keypair, PeerId};
use local_ip_address::list_afinet_netifas;
use semver::Version;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::Arc,
    time::Duration,
};
use sysinfo::{ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System};
use thiserror::Error;
use tokio::{
    fs::{create_dir_all, metadata, remove_dir_all, remove_file, File},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, SeekFrom},
    sync::Mutex,
    time::sleep,
};

// Name of the node binary used to launch new nodes processes
#[cfg(windows)]
const NODE_BIN_NAME: &str = "antnode.exe";
#[cfg(not(windows))]
const NODE_BIN_NAME: &str = "antnode";

const DEFAULT_EVM_NETWORK: &str = "evm-arbitrum-one";
const ROOT_DIR: &str = "NODE_MGR_ROOT_DIR";
const DEFAULT_ROOT_FOLDER: &str = "formicaio_data";
const DEFAULT_NODE_DATA_FOLDER: &str = "node_data";
const DEFAULT_LOGS_FOLDER: &str = "logs";
const DEFAULT_BOOTSTRAP_CACHE_FOLDER: &str = "bootstrap_cache";

// Consts used to download node binary
const ANTNODE_S3_BASE_URL: &str = "https://antnode.s3.eu-west-2.amazonaws.com";
const GITHUB_API_URL: &str = "https://api.github.com";

#[derive(Debug, Error)]
pub enum NodeManagerError {
    #[error("Failed to spawn a new node: {0}")]
    CannotSpawnNode(String),
    #[error("Node bin version not found at {0:?}")]
    NodeBinVersionNotFound(PathBuf),
    #[error("Missing '{0}' information to spawn node")]
    SpawnNodeMissingParam(String),
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
    #[error(transparent)]
    PeerIdError(#[from] libp2p_identity::DecodingError),
    #[error(transparent)]
    NodeBinInstallError(#[from] ant_releases::Error),
    #[error(transparent)]
    NodeBinVersionError(#[from] semver::Error),
}

// Execution and management of nodes as native OS processes
#[derive(Clone, Debug)]
pub struct NodeManager {
    root_dir: PathBuf,
    system: Arc<Mutex<System>>,
    nodes: Arc<Mutex<HashMap<NodeId, Child>>>,
    node_status_locked: ImmutableNodeStatus,
}

impl NodeManager {
    pub async fn new(node_status_locked: ImmutableNodeStatus) -> Result<Self, NodeManagerError> {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            panic!("This OS isn't supported by our 'sysinfo' dependency which manages the nodes as native processes.");
        }

        let root_dir = match env::var(ROOT_DIR) {
            Ok(v) => Path::new(&v).to_path_buf(),
            Err(_) => env::current_dir()?.join(DEFAULT_ROOT_FOLDER),
        }
        .to_path_buf();

        logging::log!("Node manager instantiated with root dir:: {root_dir:?}");
        create_dir_all(&root_dir).await?;

        let system = Arc::new(Mutex::new(System::new()));
        let nodes = Arc::new(Mutex::new(HashMap::default()));

        Ok(Self {
            root_dir,
            system,
            nodes,
            node_status_locked,
        })
    }

    // Create directory to hold node's data and cloned node binary
    pub async fn new_node(&self, node_info: &NodeInstanceInfo) -> Result<(), NodeManagerError> {
        let node_id = &node_info.node_id;
        let node_bin_path = self.root_dir.join(NODE_BIN_NAME);
        let new_node_data_dir = self.root_dir.join(DEFAULT_NODE_DATA_FOLDER).join(node_id);

        create_dir_all(&new_node_data_dir).await?;

        let mut source_file = File::open(node_bin_path).await?;
        let destination_path = new_node_data_dir.join(NODE_BIN_NAME);
        let mut destination_file = File::create(&destination_path).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = tokio::fs::metadata(&destination_path).await?.permissions();
            permissions.set_mode(0o755); // Set permissions to rwxr-xr-x (owner can read/write/execute, group and others can read/execute)
            tokio::fs::set_permissions(destination_path, permissions).await?;
        }
        let mut buffer = Vec::new();
        source_file.read_to_end(&mut buffer).await?;
        destination_file.write_all(&buffer).await?;

        Ok(())
    }

    // Spawn the node as a new process using its own directory and cloned node binary
    pub async fn spawn_new_node(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<NodePid, NodeManagerError> {
        let node_id = &node_info.node_id;
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
        let upnp = node_info.upnp;
        let node_logs = node_info.node_logs;

        let node_data_dir = self.root_dir.join(DEFAULT_NODE_DATA_FOLDER).join(node_id);
        let node_bin_path = node_data_dir.join(NODE_BIN_NAME);
        let bootstrap_cache_dir = self.root_dir.join(DEFAULT_BOOTSTRAP_CACHE_FOLDER);
        let log_output_dir = self
            .root_dir
            .join(DEFAULT_NODE_DATA_FOLDER)
            .join(node_id)
            .join(DEFAULT_LOGS_FOLDER);

        // if node dir and binary don't exist we create them
        if let Err(err) = metadata(&node_bin_path).await {
            if err.kind() == std::io::ErrorKind::NotFound {
                self.new_node(node_info).await?;
            } else {
                return Err(err.into());
            }
        }

        let mut args = if home_network {
            vec!["--relay".to_string()]
        } else {
            vec![]
        };

        if !upnp {
            args.push("--no-upnp".to_string());
        }

        args.push("--port".to_string());
        args.push(port.to_string());

        args.push("--metrics-server-port".to_string());
        args.push(metrics_port.to_string());

        args.push("--root-dir".to_string());
        args.push(node_data_dir.display().to_string());

        args.push("--log-output-dest".to_string());
        if node_logs {
            args.push(log_output_dir.display().to_string());
        } else {
            // untill the node binary supports this feature,
            // we just send it to stdout, which we in turn send it to 'null'
            args.push("stdout".to_string());
        }

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

        logging::log!("Spawning new node {node_id} with cmd: {command:?}");
        // Run the node
        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                logging::log!("Node process for {node_id} spawned with PID: {pid}");
                self.nodes.lock().await.insert(node_id.to_string(), child);
                node_info.pid = Some(pid);
                // let's delay it for a moment so it generates the peer id
                sleep(Duration::from_secs(2)).await;
                match self
                    .get_node_version_and_peer_id(&node_info.node_id, !home_network)
                    .await
                {
                    Ok((bin_version, peer_id, ips)) => {
                        node_info.bin_version = bin_version;
                        node_info.peer_id = peer_id;
                        node_info.ips = ips;
                    }
                    Err(err) => {
                        logging::error!("Failed to obtain node bin version and peer id for node {node_id}: {err:?}")
                    }
                }

                Ok(pid)
            }
            Err(err) => {
                logging::error!("Failed to spawn new node {node_id}: {err:?}");
                Err(NodeManagerError::CannotSpawnNode(err.to_string()))
            }
        }
    }

    // Retrieve list of nodes with up to date status.
    pub async fn get_nodes_list(
        &self,
        mut nodes_info: HashMap<NodeId, NodeInstanceInfo>,
    ) -> Result<Vec<NodeInstanceInfo>, NodeManagerError> {
        // first update processes information of our `System` struct
        let mut sys = self.system.lock().await;
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        let mut nodes_list = vec![];

        for process in sys
            .processes_by_exact_name(NODE_BIN_NAME.as_ref())
            // filter out threads
            .filter(|p| p.thread_kind().is_none())
        {
            let pid = process.pid().as_u32();
            let info = nodes_info.iter().find_map(|(_, n)| {
                if n.pid == Some(pid) {
                    Some(n.clone())
                } else {
                    None
                }
            });

            // TODO: what if there are active PIDs not found in DB...
            // ...populate them in DB so the user can see/delete them...?
            // ...killing them is not an option as we could have just lost track of it...?

            if let Some(mut node_info) = info {
                nodes_info.remove(&node_info.node_id);

                if process.status() != ProcessStatus::Zombie {
                    node_info.set_status_active();
                    nodes_list.push(node_info);
                    continue;
                }

                // we won't try to kill a zombie if its status is locked
                if self
                    .node_status_locked
                    .is_still_locked(&node_info.node_id)
                    .await
                {
                    nodes_list.push(node_info);
                    continue;
                }

                // we need to kill/wait for child zombie
                let id = self
                    .nodes
                    .lock()
                    .await
                    .iter()
                    .find(|(_, c)| c.id() == pid)
                    .map(|(id, _)| id.clone());
                let reason = if let Some(node_id) = id {
                    let status = if let Some(exit_status) = self.kill_node(&node_id).await {
                        exit_status.to_string()
                    } else {
                        "none".to_string()
                    };
                    logging::log!(
                        "Process with PID {pid} exited (node id: {node_id}) with status: {status}"
                    );
                    InactiveReason::Exited(status.to_string())
                } else {
                    logging::warn!("Zombie process detected with pid {pid}: {process:?}");
                    InactiveReason::Exited("zombie".to_string())
                };

                node_info.set_status_inactive(reason);
                nodes_list.push(node_info);
            }
        }

        // let's now go through the remaining nodes which are effectively inactive
        for (_, mut node_info) in nodes_info {
            if !node_info.status.is_inactive() {
                node_info.set_status_inactive(InactiveReason::Unknown)
            }

            nodes_list.push(node_info);
        }

        Ok(nodes_list)
    }

    // Kill node's process
    pub async fn kill_node(&self, node_id: &NodeId) -> Option<ExitStatus> {
        let mut child = match self.nodes.lock().await.remove(node_id) {
            Some(child) => child,
            None => {
                logging::error!("Failed to kill node, node not found with id: {node_id}");
                return None;
            }
        };

        if let Err(err) = child.kill() {
            logging::warn!("Failed to kill process for node {node_id}: {err:?}");
        } else {
            logging::log!("Process of node {node_id} was killed.");
        }

        match child.wait_with_output() {
            Ok(output) if output.status.success() || output.status.code().is_none() => {
                Some(output.status)
            }
            Ok(output) => {
                let status = output.status;
                logging::warn!("Killed process for node {node_id}: {status}");
                Some(status)
            }
            Err(err) => {
                logging::warn!("Error when checking killed process for node {node_id}: {err:?}");
                None
            }
        }
    }

    // Remove node's data dir
    pub async fn remove_node_dir(&self, node_id: &NodeId) {
        let node_data_dir = self.root_dir.join(DEFAULT_NODE_DATA_FOLDER).join(node_id);
        if let Err(err) = remove_dir_all(&node_data_dir).await {
            logging::warn!("Failed to remove node's dir {node_data_dir:?}: {err:?}");
        }
    }

    // Retrieve version of the node binary and its peer id
    async fn get_node_version_and_peer_id(
        &self,
        id: &NodeId,
        get_ips: bool,
    ) -> Result<(Option<String>, Option<String>, Option<String>), NodeManagerError> {
        let version = match self.helper_read_node_version(Some(id)).await {
            Ok(version) => Some(version.to_string()),
            Err(err) => {
                logging::log!("Failed to retrieve binary version of node {id}: {err:?}");
                None
            }
        };
        logging::log!("Node binary version in node {id}: {version:?}");

        let peer_id = match self.helper_read_peer_id(id).await {
            Ok(peer_id) => Some(peer_id),
            Err(err) => {
                logging::log!("Failed to retrieve PeerId of node {id}: {err:?}");
                None
            }
        };
        logging::log!("Peer id in node {id}: {peer_id:?}");

        let ips = if get_ips {
            match list_afinet_netifas() {
                Ok(network_interfaces) => {
                    let ips = network_interfaces
                        .into_iter()
                        .filter(|(_, ip)| ip.is_ipv4())
                        .map(|(_, ip)| ip.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    logging::log!("IPv4 addresses in host: {ips}");
                    Some(ips)
                }
                Err(err) => {
                    logging::log!("Failed to retrieve node IPs for {id}: {err:?}");
                    None
                }
            }
        } else {
            None
        };

        Ok((version, peer_id, ips))
    }

    // Try to retrieve the version of existing instance's node binary.
    // If node id is otherwise not provided, it will then try
    // to retrieve the version of the master node binary.
    async fn helper_read_node_version(
        &self,
        id: Option<&NodeId>,
    ) -> Result<Version, NodeManagerError> {
        let node_bin_path = if let Some(id) = id {
            self.root_dir
                .join(DEFAULT_NODE_DATA_FOLDER)
                .join(id)
                .join(NODE_BIN_NAME)
        } else {
            self.root_dir.join(NODE_BIN_NAME)
        };

        let mut cmd = Command::new(&node_bin_path);
        cmd.arg("--version");
        let output = self.exec_cmd(&mut cmd, "get node bin version")?;

        let lines = output
            .stdout
            .split(|&byte| byte == b'\n')
            .collect::<Vec<_>>();
        if let Some(line) = lines.first() {
            let version_str = String::from_utf8_lossy(line)
                .to_string()
                .strip_prefix("Autonomi Node v")
                .unwrap_or_default()
                .to_string();

            Ok(version_str.parse()?)
        } else {
            Err(NodeManagerError::NodeBinVersionNotFound(node_bin_path))
        }
    }

    async fn helper_read_peer_id(&self, id: &NodeId) -> Result<String, NodeManagerError> {
        let sk_file_path = self
            .root_dir
            .join(DEFAULT_NODE_DATA_FOLDER)
            .join(id)
            .join("secret-key");
        let mut file = File::open(sk_file_path).await?;
        let mut key_str = Vec::new();
        file.read_to_end(&mut key_str).await?;

        let keypair = Keypair::ed25519_from_bytes(key_str)?;
        Ok(PeerId::from(keypair.public()).to_string())
    }

    // Upgrade the binary of given node
    pub async fn upgrade_node(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<NodePid, NodeManagerError> {
        logging::log!("[UPGRADE] Upgrading node {} ...", node_info.node_id);

        // restart node to run with new node version
        let _res = self.kill_node(&node_info.node_id).await;
        // copy the node binary so it uses the latest version available
        self.new_node(node_info).await?;
        // let's delay it for a moment so it closes files descriptors
        sleep(Duration::from_secs(4)).await;

        let pid = self.spawn_new_node(node_info).await?;

        Ok(pid)
    }

    // Download/upgrade the master node binary which is used for new nodes to be spawned.
    // If no version is provided, it will upgrade only if existing node binary is not the latest version.
    pub async fn upgrade_master_node_binary(
        &self,
        version: Option<&Version>,
    ) -> Result<Version, NodeManagerError> {
        let release_repo = AntReleaseRepository {
            github_api_base_url: GITHUB_API_URL.to_string(),
            nat_detection_base_url: "".to_string(),
            node_launchpad_base_url: "".to_string(),
            ant_base_url: "".to_string(),
            antnode_base_url: ANTNODE_S3_BASE_URL.to_string(),
            antctl_base_url: "".to_string(),
            antnode_rpc_client_base_url: "".to_string(),
        };
        let release_type = ReleaseType::AntNode;

        let version_to_download = match version {
            Some(v) => v.clone(),
            None => release_repo.get_latest_version(&release_type).await?,
        };

        // we upgrade only if existing node binary is not the latest version
        if let Ok(version) = self.helper_read_node_version(None).await {
            if version == version_to_download {
                logging::log!("Master node binary already matches version v{version}.");
                return Ok(version);
            }
        }

        logging::log!("Downloading node binary v{version_to_download} ...");
        let platform = get_running_platform()?;

        let archive_path = release_repo
            .download_release_from_s3(
                &release_type,
                &version_to_download,
                &platform,
                &ArchiveType::TarGz,
                &self.root_dir,
                &|_, _| {},
            )
            .await?;

        let bin_path = release_repo.extract_release_archive(&archive_path, &self.root_dir)?;

        remove_file(archive_path).await?;

        logging::log!("Node binary v{version_to_download} downloaded successfully and unpacked at: {bin_path:?}");

        Ok(version_to_download)
    }

    // Clears the node's PeerId and restarts it
    pub async fn regenerate_peer_id(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<NodePid, NodeManagerError> {
        logging::log!(
            "[RECYCLE] Recycling node {} by clearing its peer-id ...",
            node_info.node_id
        );

        // we remove 'secret-key' file so the node will re-generate it when restarted.
        let file_path = self
            .root_dir
            .join(DEFAULT_NODE_DATA_FOLDER)
            .join(node_info.node_id.clone())
            .join("secret-key");
        remove_file(file_path).await?;

        // restart node to obtain a new peer-id
        let _res = self.kill_node(&node_info.node_id).await;
        let pid = self.spawn_new_node(node_info).await?;

        logging::log!(
            "Finished recycling node {}, new PID: {pid}",
            node_info.node_id
        );

        Ok(pid)
    }

    // Return a node logs stream.
    pub async fn get_node_logs_stream(
        &self,
        id: &NodeId,
    ) -> Result<impl Stream<Item = Result<Bytes, NodeManagerError>>, NodeManagerError> {
        logging::log!("[LOGS] Get LOGS stream from node {id} ...");
        let log_file_path = self
            .root_dir
            .join(DEFAULT_NODE_DATA_FOLDER)
            .join(id)
            .join(DEFAULT_LOGS_FOLDER)
            .join("antnode.log");

        let mut file = File::open(log_file_path).await?;
        let file_length = file.metadata().await?.len();
        if file_length > 1024 {
            file.seek(SeekFrom::Start(file_length - 2048u64)).await?;
        }
        let mut reader = BufReader::new(file);
        let mut max_iter = 180;
        Ok(async_stream::stream! {
            loop {
                let mut chunk = vec![0; 1024]; // Read in 1024-byte chunks.
                let bytes_read = reader.read(&mut chunk).await?;
                if bytes_read == 0 {
                    sleep(Duration::from_secs(1)).await;
                    if max_iter == 0 {
                        // we have this limit to make sure we don't leak any thread
                        // since there seems to be cases in ARM platforms...
                        break;
                    }
                    max_iter -= 1;
                    continue;
                }

                yield Ok(Bytes::from(chunk));
                max_iter = 180;
            }
        })
    }

    // Helper to execute a cmd
    fn exec_cmd(&self, cmd: &mut Command, description: &str) -> Result<Output, NodeManagerError> {
        let output = cmd.output()?;
        if !output.status.success() {
            logging::error!("Failed to execute command to {description}: {output:?}");
        }
        Ok(output)
    }
}
