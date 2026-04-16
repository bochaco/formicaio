use crate::{
    bg_tasks::ImmutableNodeStatus,
    types::{InactiveReason, NodeId, NodeInstanceInfo, NodePid},
};

use bytes::Bytes;
use flate2::read::GzDecoder;
use futures_util::Stream;
use leptos::logging;
use local_ip_address::list_afinet_netifas;
use saorsa_core::identity::NodeIdentity;
use semver::Version;
use serde_json;
use std::{
    collections::{HashMap, hash_map::Entry},
    env,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::Arc,
    time::Duration,
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};
use thiserror::Error;
use tokio::{
    fs::{File, create_dir_all, metadata, remove_dir_all, remove_file},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, SeekFrom},
    sync::RwLock,
    time::sleep,
};
use walkdir::WalkDir;

// Name of the node binary used to launch new nodes processes
#[cfg(windows)]
const NODE_BIN_NAME: &str = "ant-node.exe";
#[cfg(not(windows))]
const NODE_BIN_NAME: &str = "ant-node";

const DEFAULT_EVM_NETWORK: &str = "arbitrum-one";
const NODE_MGR_ROOT_DIR: &str = "NODE_MGR_ROOT_DIR";
const DEFAULT_ROOT_FOLDER: &str = "formicaio_data";
const DEFAULT_NODE_DATA_FOLDER: &str = "node_data";
const DEFAULT_LOGS_FOLDER: &str = "logs";
const DEFAULT_BOOTSTRAP_CACHE_FOLDER: &str = "bootstrap_cache";
const NODE_IDENTITY_KEY_FILE: &str = "node_identity.key";
const NODE_LOG_FILENAME_PREFIX: &str = "ant-node.";

// Consts used to download node binary from GitHub releases
const ANT_NODE_GITHUB_REPO: &str = "WithAutonomi/ant-node";
const GITHUB_API_URL: &str = "https://api.github.com";

#[derive(Debug, Error)]
pub enum NativeNodesError {
    #[error("Failed to spawn a new node process: {0}")]
    CannotSpawnNode(String),
    #[error("Failed to get node binary version at path {0:?}: {1}")]
    NodeBinVersionFailure(PathBuf, String),
    #[error("Node binary version not reported by executable at path: {0:?}")]
    NodeBinVersionNotFound(PathBuf),
    #[error("Missing required parameter '{0}' to spawn node.")]
    SpawnNodeMissingParam(String),
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
    #[error("Failed to load node identity: {0}")]
    NodeIdentityError(String),
    #[error("Failed to download node binary: {0}")]
    NodeBinDownloadError(String),
    #[error("No supported platform found for current architecture")]
    UnsupportedPlatform,
    #[error(transparent)]
    NodeBinVersionError(#[from] semver::Error),
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}

#[derive(Debug)]
enum NodeProcess {
    Spawned(Child),
    ProcessFound(Pid),
}

impl NodeProcess {
    fn pid(&self) -> u32 {
        match self {
            NodeProcess::Spawned(child) => child.id(),
            NodeProcess::ProcessFound(pid) => pid.as_u32(),
        }
    }
}

// Determine the platform-specific archive name for downloading ant-node
fn get_platform_archive_name() -> Result<String, NativeNodesError> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let name = match (os, arch) {
        ("linux", "x86_64") => "ant-node-cli-linux-x64.tar.gz",
        ("linux", "aarch64") => "ant-node-cli-linux-arm64.tar.gz",
        ("macos", "x86_64") => "ant-node-cli-macos-x64.tar.gz",
        ("macos", "aarch64") => "ant-node-cli-macos-arm64.tar.gz",
        ("windows", "x86_64") => "ant-node-cli-windows-x64.zip",
        _ => return Err(NativeNodesError::UnsupportedPlatform),
    };
    Ok(name.to_string())
}

// Extract NODE_BIN_NAME from a .tar.gz or .zip archive to the destination path
fn extract_binary_from_archive(
    archive_path: &Path,
    bin_name: &str,
    dest_path: &Path,
) -> Result<(), NativeNodesError> {
    let archive_str = archive_path.to_string_lossy();
    if archive_str.ends_with(".tar.gz") {
        let file = std::fs::File::open(archive_path)?;
        let gz = GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            if path.file_name().map_or(false, |n| n == bin_name) {
                entry.unpack(dest_path)?;
                return Ok(());
            }
        }
        Err(NativeNodesError::NodeBinDownloadError(format!(
            "Binary '{bin_name}' not found in archive"
        )))
    } else {
        // .zip (Windows)
        let file = std::fs::File::open(archive_path)?;
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|e| NativeNodesError::NodeBinDownloadError(e.to_string()))?;
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| NativeNodesError::NodeBinDownloadError(e.to_string()))?;
            if entry.name().ends_with(bin_name) {
                let mut out = std::fs::File::create(dest_path)?;
                std::io::copy(&mut entry, &mut out)?;
                return Ok(());
            }
        }
        Err(NativeNodesError::NodeBinDownloadError(format!(
            "Binary '{bin_name}' not found in zip archive"
        )))
    }
}

// Execution and management of nodes as native OS processes
#[derive(Clone, Debug)]
pub struct NativeNodes {
    root_dir: PathBuf,
    system: Arc<RwLock<System>>,
    nodes: Arc<RwLock<HashMap<NodeId, NodeProcess>>>,
    node_status_locked: ImmutableNodeStatus,
}

impl NativeNodes {
    pub async fn new(
        node_status_locked: ImmutableNodeStatus,
        data_dir_path: Option<PathBuf>,
        initial_pids: impl Iterator<Item = (NodeId, u32)>,
    ) -> Result<Self, NativeNodesError> {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            panic!(
                "Unsupported operating system: The 'sysinfo' dependency required for native process management is not supported on this platform. Please use Docker mode instead."
            );
        }

        let root_dir = if let Some(path) = data_dir_path {
            path
        } else {
            match std::env::var(NODE_MGR_ROOT_DIR) {
                Ok(v) => Path::new(&v).to_path_buf(),
                Err(_) => env::current_dir()?.join(DEFAULT_ROOT_FOLDER),
            }
            .to_path_buf()
        };

        logging::log!("[NodeMgr] Node manager initialized with root directory: {root_dir:?}");
        create_dir_all(&root_dir).await?;

        let system = Arc::new(RwLock::new(System::new()));
        let nodes = initial_pids
            .map(|(node_id, pid)| (node_id, NodeProcess::ProcessFound(Pid::from_u32(pid))))
            .collect();

        Ok(Self {
            root_dir,
            system,
            nodes: Arc::new(RwLock::new(nodes)),
            node_status_locked,
        })
    }

    // Create directory to hold node's data and cloned node binary
    pub async fn new_node(&self, node_info: &NodeInstanceInfo) -> Result<(), NativeNodesError> {
        let node_bin_path = self.root_dir.join(NODE_BIN_NAME);
        let new_node_data_dir = self.get_node_data_dir(node_info, true);

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
    ) -> Result<NodePid, NativeNodesError> {
        let node_id = &node_info.node_id;
        let port = node_info
            .port
            .ok_or(NativeNodesError::SpawnNodeMissingParam(
                "port number".to_string(),
            ))?;
        let metrics_port =
            node_info
                .metrics_port
                .ok_or(NativeNodesError::SpawnNodeMissingParam(
                    "metrics port number".to_string(),
                ))?;
        let rewards_address =
            node_info
                .rewards_addr
                .clone()
                .ok_or(NativeNodesError::SpawnNodeMissingParam(
                    "rewards address".to_string(),
                ))?;

        let node_data_dir = self.get_node_data_dir(node_info, true);
        let node_bin_path = node_data_dir.join(NODE_BIN_NAME);
        let bootstrap_cache_dir = self.root_dir.join(DEFAULT_BOOTSTRAP_CACHE_FOLDER);
        let log_output_dir = node_data_dir.join(DEFAULT_LOGS_FOLDER);

        // if node dir and binary don't exist we create them
        if let Err(err) = metadata(&node_bin_path).await {
            if err.kind() == std::io::ErrorKind::NotFound {
                self.new_node(node_info).await?;
            } else {
                return Err(err.into());
            }
        }

        let mut args = vec![
            "--port".to_string(),
            port.to_string(),
            "--metrics-port".to_string(),
            metrics_port.to_string(),
            "--root-dir".to_string(),
            node_data_dir.display().to_string(),
            "--enable-logging".to_string(),
        ];

        if node_info.ipv4_only {
            args.push("--ipv4-only".to_string());
        }

        if node_info.node_logs {
            args.push("--log-dir".to_string());
            args.push(log_output_dir.display().to_string());
        }

        args.push("--bootstrap-cache-dir".to_string());
        args.push(bootstrap_cache_dir.display().to_string());

        args.push("--rewards-address".to_string());
        let rewards_address_str = rewards_address.to_string();
        let rewards_address_str = if rewards_address_str.starts_with("0x") {
            rewards_address_str
        } else {
            format!("0x{rewards_address_str}")
        };
        args.push(rewards_address_str);

        args.push("--evm-network".to_string());
        args.push(DEFAULT_EVM_NETWORK.to_string());

        let mut command = Command::new(node_bin_path);
        command.args(args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.current_dir(&self.root_dir);

        logging::log!("[NodeMgr] Spawning new node process {node_id} with command: {command:?}");
        // Run the node
        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                logging::log!("[NodeMgr] Node process for {node_id} spawned with PID: {pid}");
                self.nodes
                    .write()
                    .await
                    .insert(node_id.clone(), NodeProcess::Spawned(child));
                node_info.pid = Some(pid);
                // let's delay it for a moment so it generates the peer id
                sleep(Duration::from_secs(2)).await;
                match self.get_node_version_and_peer_id(node_info).await {
                    Ok((bin_version, peer_id, ips)) => {
                        node_info.bin_version = bin_version;
                        node_info.peer_id = peer_id;
                        node_info.ips = ips;
                    }
                    Err(err) => {
                        logging::error!(
                            "[ERROR][NodeMgr] Failed to obtain node bin version and peer id for node {node_id}: {err:?}"
                        )
                    }
                }

                Ok(pid)
            }
            Err(err) => {
                logging::error!("[ERROR][NodeMgr] Failed to spawn new node {node_id}: {err:?}");
                Err(NativeNodesError::CannotSpawnNode(err.to_string()))
            }
        }
    }

    // Retrieve list of nodes with up to date status.
    pub async fn get_nodes_list(
        &self,
        mut nodes_info: HashMap<NodeId, NodeInstanceInfo>,
    ) -> Result<(Vec<NodeInstanceInfo>, Vec<(NodeId, u32, String, String)>), NativeNodesError> {
        // first update processes information of our `System` struct
        let sys = {
            let mut sys = self.system.write().await;
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
            );
            sys
        };

        let mut nodes_list = vec![];
        let mut new_pids = vec![];

        for process in sys
            .processes_by_exact_name(NODE_BIN_NAME.as_ref())
            // filter out threads
            .filter(|p| p.thread_kind().is_none())
        {
            let pid = process.pid().as_u32();
            let mut info = nodes_info.iter().find_map(|(_, n)| {
                if n.pid == Some(pid) {
                    Some(n.clone())
                } else {
                    None
                }
            });

            if info.is_none()
                && let Some(exec_path) = process.exe()
            {
                // There is an active PID not found in our DB/list,
                // let's try to match it using its execution path
                info = self
                    .start_tracking_found_process(
                        exec_path,
                        process.pid(),
                        &nodes_info,
                        &mut new_pids,
                    )
                    .await;
            }

            if let Some(mut node_info) = info {
                let node_id = &node_info.node_id;
                nodes_info.remove(node_id);
                if process.status() != ProcessStatus::Zombie {
                    node_info.set_status_active();
                    nodes_list.push(node_info);
                    continue;
                }

                // we won't try to kill a zombie if its status is locked
                if self.node_status_locked.is_still_locked(node_id).await {
                    nodes_list.push(node_info);
                    continue;
                }

                // we need to kill/wait for child zombie
                let id = self
                    .nodes
                    .read()
                    .await
                    .iter()
                    .find(|(_, c)| c.pid() == pid)
                    .map(|(id, _)| id.clone());
                let reason = if let Some(node_id) = id {
                    let status = if let Some(exit_status) = self.kill_node(&node_id).await {
                        exit_status.to_string()
                    } else {
                        "none".to_string()
                    };
                    logging::log!(
                        "[NodeMgr] Process with PID {pid} exited (node id: {node_id}) with status: {status}"
                    );
                    InactiveReason::Exited(status.to_string())
                } else {
                    logging::warn!(
                        "[WARN][NodeMgr] Zombie process detected with pid {pid}: {process:?}"
                    );
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

        Ok((nodes_list, new_pids))
    }

    // Kill node's process
    pub async fn kill_node(&self, node_id: &NodeId) -> Option<ExitStatus> {
        let process = self.nodes.write().await.remove(node_id);
        match process {
            Some(node_process) => self.kill_node_process(node_id, node_process).await,
            None => {
                logging::error!(
                    "[ERROR][NodeMgr] Failed to kill node process: Node {node_id} not found in managed processes"
                );
                None
            }
        }
    }

    // Helper to kill node's process, either spawned or found
    async fn kill_node_process(
        &self,
        node_id: &NodeId,
        node_process: NodeProcess,
    ) -> Option<ExitStatus> {
        match node_process {
            NodeProcess::Spawned(mut child) => {
                if let Err(err) = child.kill() {
                    logging::warn!(
                        "[WARN][NodeMgr] Failed to kill process for node {node_id}: {err:?}"
                    );
                } else {
                    logging::log!("[NodeMgr] Successfully terminated node process {node_id}");
                }

                match child.wait_with_output() {
                    Ok(output) if output.status.success() || output.status.code().is_none() => {
                        Some(output.status)
                    }
                    Ok(output) => {
                        let status = output.status;
                        logging::warn!(
                            "[WARN][NodeMgr] Killed process for node {node_id}: {status}"
                        );
                        Some(status)
                    }
                    Err(err) => {
                        logging::warn!(
                            "[WARN][NodeMgr] Error when checking killed process for node {node_id}: {err:?}"
                        );
                        None
                    }
                }
            }
            NodeProcess::ProcessFound(pid) => {
                let sys = self.system.read().await;
                if let Some(process) = sys.process(pid) {
                    match process.kill_and_wait() {
                        Err(err) => {
                            logging::warn!(
                                "[WARN][NodeMgr] Failed to kill process for node {node_id}: {err:?}"
                            );
                            None
                        }
                        Ok(status) => {
                            logging::log!(
                                "[NodeMgr] Successfully terminated node process {node_id}"
                            );
                            if let Some(s) = status {
                                logging::warn!(
                                    "[WARN][NodeMgr] Killed process for node {node_id}: {s}"
                                );
                                Some(s)
                            } else {
                                logging::warn!("[WARN][NodeMgr] Killed process for node {node_id}");
                                None
                            }
                        }
                    }
                } else {
                    logging::warn!(
                        "[WARN[[NodeMgr] Failed to kill node process: Process with PID {pid} not found"
                    );
                    None
                }
            }
        }
    }

    // Helper which tries to match a found process with no known node info
    async fn start_tracking_found_process(
        &self,
        exec_path: &Path,
        pid: Pid,
        nodes_info: &HashMap<NodeId, NodeInstanceInfo>,
        new_pids: &mut Vec<(NodeId, u32, String, String)>,
    ) -> Option<NodeInstanceInfo> {
        for (node_id, node_info) in nodes_info.iter() {
            let node_path = self.get_node_data_dir(node_info, true).join(NODE_BIN_NAME);
            if exec_path == node_path {
                let (bin_version, peer_id) = if let Ok((bin_version, peer_id, _)) =
                    self.get_node_version_and_peer_id(node_info).await
                {
                    (bin_version, peer_id)
                } else {
                    (node_info.bin_version.clone(), node_info.peer_id.clone())
                };

                // we will consider it only if the pid is different than the one we had for same node_id,
                // it could be that it was just added to the list in a concurrent operation
                let new_pid = pid.as_u32();
                match self.nodes.write().await.entry(node_id.clone()) {
                    Entry::Occupied(mut e) => {
                        let old_pid = e.get().pid();
                        if old_pid == new_pid {
                            // it must have been that it was just added in a
                            // concurrent operation, ...let's just ignore this process then
                            return None;
                        } else {
                            let old_node_process = e.insert(NodeProcess::ProcessFound(pid));
                            let status = if let Some(exit_status) =
                                self.kill_node_process(node_id, old_node_process).await
                            {
                                exit_status.to_string()
                            } else {
                                "none".to_string()
                            };
                            logging::log!(
                                "[NodeMgr] Process with PID {old_pid} which restarted itself (node id: {node_id}) exited with status: {status}",
                            );
                        }
                    }
                    Entry::Vacant(e) => {
                        e.insert(NodeProcess::ProcessFound(pid));
                    }
                }

                logging::log!(
                    "[NodeMgr] Node {node_id} has a new PID {pid} after restarting itself"
                );

                new_pids.push((
                    node_id.clone(),
                    new_pid,
                    bin_version.clone().unwrap_or_default(),
                    peer_id.clone().unwrap_or_default(),
                ));

                let updated_info = NodeInstanceInfo {
                    pid: Some(new_pid),
                    bin_version,
                    peer_id,
                    ..node_info.clone()
                };

                return Some(updated_info);
            }
        }

        None
    }

    // Remove node's data dir
    pub async fn remove_node_dir(&self, node_info: &NodeInstanceInfo) {
        let node_data_dir = self.get_node_data_dir(node_info, true);
        if let Err(err) = remove_dir_all(&node_data_dir).await {
            logging::warn!(
                "[WARN][NodeMgr] Failed to remove node's dir {node_data_dir:?}: {err:?}"
            );
        }
    }

    // Helper to get node data dir based on node-mgr root dir and node custom data dir if set
    fn get_node_data_dir(&self, node_info: &NodeInstanceInfo, include_node_id: bool) -> PathBuf {
        let data_dir = if let Some(custom_path) = &node_info.data_dir_path {
            if custom_path.display().to_string().is_empty() {
                self.root_dir.join(DEFAULT_NODE_DATA_FOLDER)
            } else if custom_path.is_absolute() {
                custom_path.to_path_buf()
            } else {
                self.root_dir.join(custom_path)
            }
        } else {
            self.root_dir.join(DEFAULT_NODE_DATA_FOLDER)
        };

        if include_node_id {
            data_dir.join(node_info.node_id.to_string())
        } else {
            data_dir
        }
    }

    // Get disk used by node in bytes, plus its base data dir
    pub fn get_used_disk_space(&self, node_info: &NodeInstanceInfo) -> (u64, PathBuf) {
        let mut total_size = 0u64;
        let base_data_dir = self.get_node_data_dir(node_info, false);
        let node_data_dir = base_data_dir.join(node_info.node_id.to_string());

        for entry in WalkDir::new(&node_data_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                total_size += meta.len();
            }
        }

        (total_size, base_data_dir)
    }

    // Retrieve version of the node binary and its peer id
    async fn get_node_version_and_peer_id(
        &self,
        node_info: &NodeInstanceInfo,
    ) -> Result<(Option<String>, Option<String>, Option<String>), NativeNodesError> {
        let id = &node_info.node_id;
        let only_ipv4 = node_info.ipv4_only;

        let version = match self.read_node_version(Some(node_info)).await {
            Ok(version) => Some(version.to_string()),
            Err(err) => {
                logging::error!(
                    "[ERROR][NodeMgr] Failed to retrieve binary version for node {id}: {err:?}"
                );
                None
            }
        };
        logging::log!("[NodeMgr] Node {id} binary version: {version:?}");

        let peer_id = match self.helper_read_peer_id(node_info).await {
            Ok(peer_id) => Some(peer_id),
            Err(err) => {
                logging::error!(
                    "[ERROR][NodeMgr] Failed to retrieve Peer ID for node {id}: {err:?}"
                );
                None
            }
        };
        logging::log!("[NodeMgr] Node {id} Peer ID: {peer_id:?}");

        let ips = match list_afinet_netifas() {
            Ok(network_interfaces) => {
                let ips = network_interfaces
                    .into_iter()
                    .filter(|(_, ip)| if only_ipv4 { ip.is_ipv4() } else { true })
                    .map(|(_, ip)| ip.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                logging::log!(
                    "[NodeMgr] IP{} addresses in host: {ips}",
                    if only_ipv4 { "v4" } else { " v4/v6" }
                );
                Some(ips)
            }
            Err(err) => {
                logging::error!("[ERROR][NodeMgr] Failed to retrieve node IPs for {id}: {err:?}");
                None
            }
        };

        Ok((version, peer_id, ips))
    }

    // Try to retrieve the version of existing instance's node binary.
    // If node id is otherwise not provided, it will then try
    // to retrieve the version of the master node binary.
    pub async fn read_node_version(
        &self,
        node_info: Option<&NodeInstanceInfo>,
    ) -> Result<Version, NativeNodesError> {
        let node_bin_path = if let Some(info) = node_info {
            let node_data_dir = self.get_node_data_dir(info, true);
            node_data_dir.join(NODE_BIN_NAME)
        } else {
            self.root_dir.join(NODE_BIN_NAME)
        };

        let mut cmd = Command::new(&node_bin_path);
        cmd.arg("--version");
        let output = self
            .exec_cmd(&mut cmd, "get node bin version")
            .map_err(|err| {
                NativeNodesError::NodeBinVersionFailure(node_bin_path.clone(), err.to_string())
            })?;

        let lines = output
            .stdout
            .split(|&byte| byte == b'\n')
            .collect::<Vec<_>>();
        if let Some(line) = lines.first() {
            let version_str = String::from_utf8_lossy(line)
                .to_string()
                .strip_prefix("ant-node ")
                .unwrap_or_default()
                .to_string();

            Ok(version_str.parse()?)
        } else {
            Err(NativeNodesError::NodeBinVersionNotFound(node_bin_path))
        }
    }

    async fn helper_read_peer_id(
        &self,
        node_info: &NodeInstanceInfo,
    ) -> Result<String, NativeNodesError> {
        let node_data_dir = self.get_node_data_dir(node_info, true);
        let key_file_path = node_data_dir.join(NODE_IDENTITY_KEY_FILE);
        let identity = NodeIdentity::load_from_file(&key_file_path)
            .await
            .map_err(|e| NativeNodesError::NodeIdentityError(e.to_string()))?;
        Ok(identity.peer_id().to_string())
    }

    // Upgrade the binary of given node
    pub async fn upgrade_node(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<NodePid, NativeNodesError> {
        logging::log!(
            "[NodeMgr] Starting UPGRADE process for node {} ...",
            node_info.node_id
        );

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
    ) -> Result<Version, NativeNodesError> {
        let client = reqwest::Client::builder().user_agent("formicaio").build()?;

        let version_to_download = match version {
            Some(v) => v.clone(),
            None => {
                let url = format!("{GITHUB_API_URL}/repos/{ANT_NODE_GITHUB_REPO}/releases/latest");
                let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
                let tag = resp["tag_name"]
                    .as_str()
                    .ok_or_else(|| {
                        NativeNodesError::NodeBinDownloadError(
                            "Missing tag_name in GitHub release response".to_string(),
                        )
                    })?
                    .trim_start_matches('v');
                tag.parse()?
            }
        };

        // we upgrade only if existing node binary is not the latest version
        if let Ok(version) = self.read_node_version(None).await
            && version == version_to_download
        {
            logging::log!(
                "[NodeMgr] Master node binary is already up to date (version v{version})"
            );
            return Ok(version);
        }

        logging::log!(
            "[NodeMgr] Downloading node binary version v{version_to_download} from repository ..."
        );

        let archive_name = get_platform_archive_name()?;
        let download_url = format!(
            "https://github.com/{ANT_NODE_GITHUB_REPO}/releases/download/v{version_to_download}/{archive_name}"
        );

        logging::log!("[NodeMgr] Downloading from: {download_url}");

        let response = client.get(&download_url).send().await?;
        if !response.status().is_success() {
            return Err(NativeNodesError::NodeBinDownloadError(format!(
                "HTTP {} when downloading {download_url}",
                response.status()
            )));
        }

        let archive_bytes = response.bytes().await?;
        let archive_path = self.root_dir.join(&archive_name);
        tokio::fs::write(&archive_path, &archive_bytes).await?;

        // Extract the binary from the archive
        let bin_path = self.root_dir.join(NODE_BIN_NAME);
        extract_binary_from_archive(&archive_path, NODE_BIN_NAME, &bin_path)?;

        remove_file(archive_path).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = tokio::fs::metadata(&bin_path).await?.permissions();
            permissions.set_mode(0o755);
            tokio::fs::set_permissions(&bin_path, permissions).await?;
        }

        logging::log!(
            "[NodeMgr] Node binary v{version_to_download} downloaded successfully at: {bin_path:?}"
        );

        Ok(version_to_download)
    }

    // Clears the node's PeerId and restarts it
    pub async fn regenerate_peer_id(
        &self,
        node_info: &mut NodeInstanceInfo,
    ) -> Result<NodePid, NativeNodesError> {
        logging::log!(
            "[NodeMgr] Starting RECYCLING process for node {} by clearing its peer-id ...",
            node_info.node_id
        );

        // we remove 'node_identity.key' file so the node will re-generate it when restarted.
        let node_data_dir = self.get_node_data_dir(node_info, true);
        let file_path = node_data_dir.join(NODE_IDENTITY_KEY_FILE);
        remove_file(file_path).await?;

        // restart node to obtain a new peer-id
        let _res = self.kill_node(&node_info.node_id).await;
        let pid = self.spawn_new_node(node_info).await?;

        logging::log!(
            "[NodeMgr] Finished recycling node {}, new PID: {pid}",
            node_info.node_id
        );

        Ok(pid)
    }

    // Find the most recent log file in the given logs directory.
    // Log files are named {NODE_LOG_FILENAME_PREFIX}.YYYY-MM-DD.log and rotate daily.
    async fn find_latest_log_file(logs_dir: &Path) -> Option<PathBuf> {
        let mut read_dir = tokio::fs::read_dir(logs_dir).await.ok()?;
        let mut latest: Option<(String, PathBuf)> = None;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(NODE_LOG_FILENAME_PREFIX) && name_str.ends_with(".log") {
                let date_part = &name_str[NODE_LOG_FILENAME_PREFIX.len()..name_str.len() - 4];
                match &latest {
                    None => latest = Some((date_part.to_string(), entry.path())),
                    Some((cur_date, _)) if date_part > cur_date.as_str() => {
                        latest = Some((date_part.to_string(), entry.path()))
                    }
                    _ => {}
                }
            }
        }
        latest.map(|(_, path)| path)
    }

    // Return a node logs stream.
    pub async fn get_node_logs_stream(
        &self,
        node_info: &NodeInstanceInfo,
    ) -> Result<impl Stream<Item = Result<Bytes, NativeNodesError>> + use<>, NativeNodesError> {
        logging::log!(
            "[NodeMgr] Starting LOG stream for node {} ...",
            node_info.node_id
        );

        let node_data_dir = self.get_node_data_dir(node_info, true);
        let logs_dir = node_data_dir.join(DEFAULT_LOGS_FOLDER);
        let log_file_path = Self::find_latest_log_file(&logs_dir).await.ok_or_else(|| {
            NativeNodesError::StdIoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("No log file found in {logs_dir:?}"),
            ))
        })?;

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
    fn exec_cmd(&self, cmd: &mut Command, description: &str) -> Result<Output, NativeNodesError> {
        let output = cmd.output()?;
        if !output.status.success() {
            logging::error!(
                "[ERROR][NodeMgr] Command execution failed to {description}: {output:?}"
            );
        }
        Ok(output)
    }
}
