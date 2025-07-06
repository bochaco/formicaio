use super::{
    docker_msgs::*,
    node_instance::{NodeId, NodeInstanceInfo},
    server_api_types::NodeOpts,
};

use axum::body::Body;
use bytes::Bytes;
use futures_util::{Stream, StreamExt, pin_mut};
use http_body_util::BodyExt;
use hyper::{
    Method, Request, Response, StatusCode,
    body::Incoming,
    client::conn,
    header::{CONTENT_LENGTH, CONTENT_TYPE},
};
use hyper_util::rt::TokioIo;
use leptos::logging;
use serde::Serialize;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;
use tokio::{net::UnixStream, time::timeout};
use url::form_urlencoded;

// Label's key to set to each container created, so we can then use as
// filter when fetching the list of them.
const LABEL_KEY_VERSION: &str = "formica_version";
// Label's key to cache node's port number
pub const LABEL_KEY_NODE_PORT: &str = "node_port";
// Label's key to cache node's metrics port number
pub const LABEL_KEY_METRICS_PORT: &str = "metrics_port";
// Label's key to cache the rewards address set for the node
pub const LABEL_KEY_REWARDS_ADDR: &str = "rewards_addr";
// Label's key to cache the flag value set --home-network for the node
pub const LABEL_KEY_HOME_NETWORK_DISABLED: &str = "home_network_disabled";
// Label's key to cache the flag value set --no-upnp for the node
pub const LABEL_KEY_UPNP_DISABLED: &str = "upnp_disabled";
// Label's key to cache the value set to node logs for the node
pub const LABEL_KEY_NODE_LOGS_DISABLED: &str = "node_logs_disabled";

// Docker API base paths
const DOCKER_CONTAINERS_API: &str = "/containers";
const DOCKER_EXEC_API: &str = "/exec";
const DOCKER_IMAGES_API: &str = "/images";

// Env var name to set the path of the Docker socket.
const DOCKER_SOCKET_PATH: &str = "DOCKER_SOCKET_PATH";
// Default path for the Docker socket.
const DEFAULT_DOCKER_SOCKET_PATH: &str = "/var/run/docker.sock";

// Name and tag of the Docker image to use by default for each node instance
const DEFAULT_NODE_CONTAINER_IMAGE_NAME: &str = "bochaco/formica";
const DEFAULT_NODE_CONTAINER_IMAGE_TAG: &str = "latest";
// Env var names to set the name and tag of the Docker image to use for each node instance
const NODE_CONTAINER_IMAGE_NAME: &str = "NODE_CONTAINER_IMAGE_NAME";
const NODE_CONTAINER_IMAGE_TAG: &str = "NODE_CONTAINER_IMAGE_TAG";

// Number of seconds before timing out an attempt to upgrade the node binary.
pub const UPGRADE_NODE_BIN_TIMEOUT_SECS: u64 = 8 * 60; // 8 mins

#[derive(Debug, Error)]
pub enum DockerClientError {
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
    #[error(transparent)]
    HyperError(#[from] hyper::Error),
    #[error("Docker client error: {0}")]
    ClientError(String),
    #[error("Container not found with id: {0}")]
    CointainerNotFound(NodeId),
    #[error("Image not found locally")]
    ImageNotFound,
    #[error("Docker server error with code {0}: {1}")]
    DockerServerError(u16, String),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error("Value received couldn't be parsed as integer: '{0}'")]
    InvalidValue(String),
}

// Type of request supported by internal helpers herein.
#[derive(Clone)]
enum ReqMethod {
    Get,
    Post(String),
    Put(Vec<u8>),
    Delete,
}

impl ReqMethod {
    fn post<T: Serialize>(body: &T) -> Result<Self, DockerClientError> {
        Ok(Self::Post(serde_json::to_string(body)?))
    }

    fn post_empty_body() -> Self {
        Self::Post("".to_string())
    }

    // Send request to Docker server
    async fn try_send_request(
        &self,
        base_url: &str,
        query_params: &[(&str, &str)],
        socket_path: &Path,
    ) -> Result<Response<Incoming>, DockerClientError> {
        let unix_stream = UnixStream::connect(socket_path).await.map_err(|err| {
            DockerClientError::ClientError(format!(
                "Failed to connect to Docker socket at {socket_path:?}: {err:?}"
            ))
        })?;
        let io = TokioIo::new(unix_stream);
        let (mut docker_reqs_sender, connection) = conn::http1::handshake(io).await?;
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                logging::log!("Error when connecting to Docker: {err:?}");
            }
        });

        // Construct the query string using url::form_urlencoded
        let query_string = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query_params)
            .finish();

        // Construct the full URL with query parameters
        let full_url = format!("{base_url}?{query_string}");

        let req_builder = Request::builder()
            .uri(full_url)
            // Host added just because http1 requires it
            .header("Host", "localhost");

        let req = match self {
            ReqMethod::Post(body_str) => req_builder
                .method(Method::POST)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body_str.clone()))?,
            ReqMethod::Put(bytes) => req_builder
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(CONTENT_LENGTH, bytes.len())
                .method(Method::PUT)
                .body(Body::from(bytes.clone()))?,
            ReqMethod::Get => req_builder.method(Method::GET).body(Body::from(()))?,
            ReqMethod::Delete => req_builder.method(Method::DELETE).body(Body::from(()))?,
        };

        let resp = docker_reqs_sender.send_request(req).await?;

        match resp.status() {
            StatusCode::NO_CONTENT | StatusCode::CREATED | StatusCode::OK => Ok(resp),
            StatusCode::NOT_FOUND => {
                let resp_bytes = get_response_bytes(resp).await?;
                let msg: ServerErrorMessage = serde_json::from_slice(&resp_bytes)?;
                // unfortunatelly the API returns different error msgs
                // instead of different error codes to handle them
                if msg.message.starts_with("No such image") {
                    Err(DockerClientError::ImageNotFound)
                } else {
                    Err(DockerClientError::DockerServerError(
                        StatusCode::NOT_FOUND.into(),
                        msg.message,
                    ))
                }
            }
            other => {
                let resp_bytes = get_response_bytes(resp).await?;
                let msg = match serde_json::from_slice::<ServerErrorMessage>(&resp_bytes) {
                    Ok(msg) => msg.message,
                    Err(_) => String::from_utf8_lossy(&resp_bytes).to_string(),
                };
                Err(DockerClientError::DockerServerError(other.into(), msg))
            }
        }
    }
}

// Client to send requests to a Docker server's API
#[derive(Clone, Debug)]
pub struct DockerClient {
    socket_path: PathBuf,
    node_image_name: String,
    node_image_tag: String,
}

impl DockerClient {
    // Instantiate a Docker client,
    pub async fn new() -> Result<Self, DockerClientError> {
        let socket_path = match env::var(DOCKER_SOCKET_PATH) {
            Ok(v) => Path::new(&v).to_path_buf(),
            Err(_) => Path::new(DEFAULT_DOCKER_SOCKET_PATH).to_path_buf(),
        };
        logging::log!("Docker socket path: {socket_path:?}");

        let node_image_name = match env::var(NODE_CONTAINER_IMAGE_NAME) {
            Ok(v) => v.to_string(),
            Err(_) => DEFAULT_NODE_CONTAINER_IMAGE_NAME.to_string(),
        };
        let node_image_tag = match env::var(NODE_CONTAINER_IMAGE_TAG) {
            Ok(v) => v.to_string(),
            Err(_) => DEFAULT_NODE_CONTAINER_IMAGE_TAG.to_string(),
        };
        logging::log!("Using formica node image: {node_image_name}:{node_image_tag}");

        Ok(Self {
            socket_path,
            node_image_tag,
            node_image_name,
        })
    }

    // Query the Docker server to return the info of the container matching the given id
    pub async fn get_container_info(
        &self,
        id: &NodeId,
    ) -> Result<NodeInstanceInfo, DockerClientError> {
        let mut filters = HashMap::default();
        filters.insert("id".to_string(), vec![id.clone()]);
        let containers = self.list_containers(&filters, true).await?;
        containers
            .into_iter()
            .next()
            .ok_or(DockerClientError::CointainerNotFound(id.clone()))
    }

    // Query the Docker server to return the list of ALL existing containers,
    // unless 'all' argument is set to false in which case only running containers are returned.
    pub async fn get_containers_list(&self) -> Result<Vec<NodeInstanceInfo>, DockerClientError> {
        let mut filters = HashMap::default();
        filters.insert("label".to_string(), vec![LABEL_KEY_VERSION.to_string()]);
        self.list_containers(&filters, true).await
    }

    // Query the Docker server to return a LIST of existing containers using the given filter.
    async fn list_containers(
        &self,
        filters: &HashMap<String, Vec<String>>,
        all: bool,
    ) -> Result<Vec<NodeInstanceInfo>, DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/json");
        let all_str = all.to_string();
        let query = &[
            ("all", all_str.as_str()),
            ("filters", &serde_json::to_string(filters)?),
        ];
        let resp_bytes = self.send_request(ReqMethod::Get, &url, query).await?;
        let containers: Vec<Container> = serde_json::from_slice(&resp_bytes)?;
        let nodes = containers.into_iter().map(|c| c.into()).collect();
        Ok(nodes)
    }

    // Request the Docker server to DELETE a container matching the given id
    pub async fn delete_container(&self, id: &NodeId) -> Result<(), DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/{id}");
        logging::log!("[DELETE] Sending Docker request to DELETE containers: {url} ...");
        let query = &[("force", "true")];
        self.send_request(ReqMethod::Delete, &url, query).await?;
        Ok(())
    }

    // Request the Docker server to START a container matching the given id
    pub async fn start_container(
        &self,
        id: &NodeId,
        get_ips: bool,
    ) -> Result<(Option<String>, Option<String>, Option<String>), DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/{id}/start");
        logging::log!("[START] Sending Docker request to START a container: {url} ...");
        self.send_request(ReqMethod::post_empty_body(), &url, &[])
            .await?;

        let url = format!("{DOCKER_CONTAINERS_API}/{id}/update");
        logging::log!(
            "Sending Docker request to UPDATE the restart policy of a container: {url} ..."
        );
        let container_update_req = ContainerUpdate {
            RestartPolicy: Some(RestartPolicy {
                Name: "on-failure".to_string(),
                MaximumRetryCount: Some(5),
            }),
        };
        self.send_request(ReqMethod::post(&container_update_req)?, &url, &[])
            .await?;

        // let's try to retrieve new version
        self.get_node_version_and_peer_id(id, get_ips).await
    }

    // Request the Docker server to STOP a container matching the given id
    pub async fn stop_container(&self, id: &NodeId) -> Result<(), DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/{id}/stop");
        logging::log!("[STOP] Sending Docker request to STOP a container: {url} ...");
        self.send_request(ReqMethod::post_empty_body(), &url, &[])
            .await?;

        Ok(())
    }

    // Request the Docker server to CREATE a new node container, returning the container info.
    pub async fn create_new_container(
        &self,
        node_opts: NodeOpts,
    ) -> Result<NodeId, DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/create");
        let mapped_ports = [(node_opts.port, "udp"), (node_opts.metrics_port, "tcp")];

        let mut labels = vec![
            (LABEL_KEY_VERSION.to_string(), self.node_image_tag.clone()),
            (LABEL_KEY_NODE_PORT.to_string(), node_opts.port.to_string()),
            (
                LABEL_KEY_METRICS_PORT.to_string(),
                node_opts.metrics_port.to_string(),
            ),
        ];
        let mut env_vars = vec![
            format!("NODE_PORT={}", node_opts.port),
            format!("METRICS_PORT={}", node_opts.metrics_port),
        ];
        if !node_opts.rewards_addr.is_empty() {
            env_vars.push(format!(
                "REWARDS_ADDR_ARG=--rewards-address {}",
                node_opts.rewards_addr
            ));
            labels.push((
                LABEL_KEY_REWARDS_ADDR.to_string(),
                node_opts.rewards_addr.clone(),
            ));
        }
        if !node_opts.home_network {
            env_vars.push("HOME_NETWORK_ARG=".to_string());
            labels.push((
                LABEL_KEY_HOME_NETWORK_DISABLED.to_string(),
                "true".to_string(),
            ));
        }
        env_vars.push(format!("IP_ARG=--ip {}", node_opts.node_ip));
        if !node_opts.upnp {
            labels.push((LABEL_KEY_UPNP_DISABLED.to_string(), "true".to_string()));
            env_vars.push("UPNP_ARG=--no-upnp".to_string());
        }
        if !node_opts.node_logs {
            env_vars.push("NODE_LOGS_ARG=".to_string());
            labels.push((LABEL_KEY_NODE_LOGS_DISABLED.to_string(), "true".to_string()));
        }

        let container_create_req = ContainerCreate {
            Image: format!("{}:{}", self.node_image_name, self.node_image_tag),
            // we use a label so we can then filter them when fetching a list of containers
            Labels: Some(labels.into_iter().collect()),
            Env: Some(env_vars),
            ExposedPorts: Some(
                mapped_ports
                    .iter()
                    .map(|(port, proto)| (format!("{port}/{proto}"), HashMap::default()))
                    .collect::<ExposedPorts>(),
            ),
            StopSignal: Some("SIGINT".to_string()),
            StopTimeout: Some(5),
            HostConfig: Some(HostConfigCreate {
                NetworkMode: Some("host".to_string()),
                PublishAllPorts: Some(false),
                PortBindings: Some(
                    mapped_ports
                        .iter()
                        .map(|(port, proto)| {
                            (
                                format!("{port}/{proto}"),
                                vec![PortBinding {
                                    HostIp: Some("0.0.0.0".to_string()),
                                    HostPort: port.to_string(),
                                }],
                            )
                        })
                        .collect::<PortBindings>(),
                ),
            }),
        };

        let random_name = hex::encode(rand::random::<[u8; 10]>());
        logging::log!(
            "[CREATE] Sending Docker request to CREATE a new container (named: {random_name}): {url} ..."
        );
        let resp_bytes = self
            .send_request(
                ReqMethod::post(&container_create_req)?,
                &url,
                &[("name", &random_name)],
            )
            .await?;
        let container: ContainerCreateExecSuccess = serde_json::from_slice(&resp_bytes)?;
        logging::log!("Container created successfully: {container:#?}");

        Ok(container.Id)
    }

    // Request the Docker server to return a node container logs stream.
    pub async fn get_container_logs_stream(
        &self,
        id: &NodeId,
    ) -> Result<impl Stream<Item = Result<Bytes, DockerClientError>> + use<>, DockerClientError>
    {
        let url = format!("{DOCKER_CONTAINERS_API}/{id}/exec");
        logging::log!("[LOGS] Sending Docker query to get container LOGS stream: {url} ...");
        let exec_cmd = ContainerExec {
            AttachStdin: Some(false),
            AttachStdout: Some(true),
            AttachStderr: Some(true),
            Cmd: Some(vec![
                "sh".to_string(),
                "-c".to_string(),
                "tail -s 7 -f /app/node_data/logs/antnode.log".to_string(),
            ]),
            Tty: Some(false),
        };
        let resp_bytes = self
            .send_request(ReqMethod::post(&exec_cmd)?, &url, &[])
            .await?;
        let exec_result: ContainerCreateExecSuccess = serde_json::from_slice(&resp_bytes)?;
        logging::log!("Cmd to stream logs created successfully: {exec_result:#?}");
        let exec_id = exec_result.Id;

        // let's now start the exec cmd created
        let url = format!("{DOCKER_EXEC_API}/{exec_id}/start");
        let opts = ContainerExecStart {
            Detach: Some(false),
            Tty: Some(true),
        };

        self.send_request_and_return_stream(ReqMethod::post(&opts)?, &url, &[])
            .await
    }

    // Request the Docker server to UPGRADE the node binary within a container matching the given id
    pub async fn upgrade_node_in_container(
        &self,
        id: &NodeId,
        get_ips: bool,
    ) -> Result<(Option<String>, Option<String>), DockerClientError> {
        logging::log!("[UPGRADE] Sending Docker request to UPGRADE node within a container...");

        let cmd = "./antup node -n -p /app".to_string();
        let exec_cmd = self.exec_in_container(id, cmd, "upgrade node binary");
        let timeout_duration = Duration::from_secs(UPGRADE_NODE_BIN_TIMEOUT_SECS);
        match timeout(timeout_duration, exec_cmd).await {
            Err(_) => logging::log!(
                "Timeout ({timeout_duration:?}) while upgrading node binary. Let's restart it anyways..."
            ),
            Ok(resp) => {
                let (exec_id, _) = resp?;
                logging::log!("Node upgrade process finished in container: {id}");

                // let's check its exit code
                let url = format!("{DOCKER_EXEC_API}/{exec_id}/json");
                let resp_bytes = self.send_request(ReqMethod::Get, &url, &[]).await?;
                let exec: ContainerExecJson = serde_json::from_slice(&resp_bytes)?;
                logging::log!("Container exec: {exec:#?}");
                if exec.ExitCode != 0 {
                    let error_msg = format!("Failed to upgrade node, exit code: {}", exec.ExitCode);
                    logging::log!("{error_msg}");
                    return Err(DockerClientError::DockerServerError(
                        exec.ExitCode.into(),
                        error_msg,
                    ));
                }
            }
        }

        // let's try to retrieve new version, forget it if there is any error
        let (new_version, _, ips) = self
            .get_node_version_and_peer_id(id, get_ips)
            .await
            .unwrap_or_default();

        // restart container to run with new node version
        self.restart_container(id).await?;

        Ok((new_version, ips))
    }

    // Retrieve version of the node binary and its peer id
    pub async fn get_node_version_and_peer_id(
        &self,
        id: &NodeId,
        get_ips: bool,
    ) -> Result<(Option<String>, Option<String>, Option<String>), DockerClientError> {
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

        let ips = if get_ips {
            let cmd = "ip -4 addr show | awk '/inet / {print $2}' | cut -d/ -f1 | paste -sd ' '"
                .to_string();
            let (_, ips) = self
                .exec_in_container(id, cmd, "get node network IPs")
                .await?;
            let ips = ips.trim().replace(' ', ", ");
            logging::log!("Node IPs in container {id}: {ips}");
            Some(ips)
        } else {
            None
        };

        Ok((version, peer_id, ips))
    }

    // Clears the node's PeerId within the containver and restarts it
    pub async fn regenerate_peer_id_in_container(
        &self,
        id: &NodeId,
        get_ips: bool,
    ) -> Result<(Option<String>, Option<String>, Option<String>), DockerClientError> {
        logging::log!("[RECYCLE] Recycling container by clearing node's peer-id ...");

        // we write an empty file at '/app/node_data/secret-key-recycle' so the container removes
        // existing secret-key file before invoking the node binary upon restarting the container.
        let url = format!("{DOCKER_CONTAINERS_API}/{id}/archive");
        let query = &[("path", "/app/node_data")];
        let empty_file_tar_bzip2 = vec![
            66, 90, 104, 57, 49, 65, 89, 38, 83, 89, 124, 173, 200, 126, 0, 0, 128, 125, 128, 192,
            128, 2, 0, 64, 2, 127, 128, 0, 1, 122, 76, 158, 32, 16, 8, 32, 0, 116, 26, 9, 54, 166,
            129, 161, 233, 1, 166, 130, 74, 105, 169, 160, 0, 208, 0, 222, 116, 252, 36, 17, 116,
            33, 23, 57, 11, 41, 139, 149, 193, 36, 13, 6, 229, 187, 245, 242, 36, 41, 201, 48, 51,
            18, 125, 214, 6, 50, 131, 36, 93, 82, 1, 224, 204, 194, 33, 71, 99, 52, 125, 132, 249,
            20, 30, 214, 130, 181, 30, 25, 84, 94, 171, 107, 97, 43, 37, 211, 34, 32, 63, 23, 114,
            69, 56, 80, 144, 124, 173, 200, 126,
        ];
        self.send_request(ReqMethod::Put(empty_file_tar_bzip2), &url, query)
            .await?;

        // restart container to obtain a new peer-id
        self.restart_container(id).await?;

        logging::log!("Finished recycling node container: {id}");

        self.get_node_version_and_peer_id(id, get_ips).await
    }

    // Restart the container wich has given id
    async fn restart_container(&self, id: &NodeId) -> Result<(), DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/{id}/restart");
        logging::log!("[RESTART] Sending Docker request to RESTART a container: {url} ...");
        self.send_request(ReqMethod::post_empty_body(), &url, &[])
            .await?;
        Ok(())
    }

    // Helper to execute a cmd in a given container
    async fn exec_in_container(
        &self,
        id: &NodeId,
        cmd: String,
        cmd_desc: &str,
    ) -> Result<(String, String), DockerClientError> {
        let url = format!("{DOCKER_CONTAINERS_API}/{id}/exec");
        let exec_cmd = ContainerExec {
            AttachStdin: Some(false),
            AttachStdout: Some(true),
            AttachStderr: Some(false),
            Cmd: Some(vec!["sh".to_string(), "-c".to_string(), cmd]),
            Tty: Some(false),
        };
        let resp_bytes = self
            .send_request(ReqMethod::post(&exec_cmd)?, &url, &[])
            .await?;
        let exec_result: ContainerCreateExecSuccess = serde_json::from_slice(&resp_bytes)?;
        logging::log!("Cmd to {cmd_desc} created successfully: {exec_result:#?}");
        let exec_id = exec_result.Id;
        // let's now start the exec cmd created
        let url = format!("{DOCKER_EXEC_API}/{exec_id}/start");
        let opts = ContainerExecStart {
            Detach: Some(false),
            Tty: Some(true),
        };
        let resp_bytes = self
            .send_request(ReqMethod::post(&opts)?, &url, &[])
            .await?;
        let resp_str = String::from_utf8_lossy(&resp_bytes).to_string();

        Ok((exec_id, resp_str))
    }

    // Send request to Docker server, pulling the formica image
    // if necessary before retrying.
    async fn send_request(
        &self,
        method: ReqMethod,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<u8>, DockerClientError> {
        let resp = match method.try_send_request(url, query, &self.socket_path).await {
            Err(DockerClientError::ImageNotFound) => {
                logging::log!(
                    "We need to pull the formica image: {}.",
                    self.node_image_name
                );
                // let's pull the image before retrying
                self.pull_formica_image().await?;
                method.try_send_request(url, query, &self.socket_path).await
            }
            other => other,
        }?;

        get_response_bytes(resp).await
    }

    // Send request to Docker server, pulling the formica image
    // if necessary before retrying, and returning the response as a stream.
    async fn send_request_and_return_stream(
        &self,
        method: ReqMethod,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<impl Stream<Item = Result<Bytes, DockerClientError>> + use<>, DockerClientError>
    {
        let resp = match method.try_send_request(url, query, &self.socket_path).await {
            Err(DockerClientError::ImageNotFound) => {
                logging::log!(
                    "We need to pull the formica image: {} ...",
                    self.node_image_name
                );
                // let's pull the image before retrying
                self.pull_formica_image().await?;
                method.try_send_request(url, query, &self.socket_path).await
            }
            other => other,
        }?;

        Ok(resp_to_stream(resp))
    }

    // Pull the formica image.
    pub async fn pull_formica_image(&self) -> Result<(), DockerClientError> {
        let url = format!("{DOCKER_IMAGES_API}/create");
        logging::log!(
            "[PULL] Sending Docker request to PULL formica image: {}:{} ...",
            self.node_image_name,
            self.node_image_tag
        );
        let query = &[
            ("fromImage", self.node_image_name.as_str()),
            ("tag", self.node_image_tag.as_str()),
        ];
        let resp = ReqMethod::post_empty_body()
            .try_send_request(&url, query, &self.socket_path)
            .await?;

        // consume and await end of response stream, discarding the bytes
        get_response_bytes(resp).await?;

        // TODO: check if it succeeded and report error if it failed
        logging::log!(
            "Formica image {}:{} was successfully pulled!",
            self.node_image_name,
            self.node_image_tag
        );

        Ok(())
    }
}

// Convert a Response into a Stream of its body bytes.
fn resp_to_stream(
    mut resp: Response<Incoming>,
) -> impl Stream<Item = Result<Bytes, DockerClientError>> {
    async_stream::stream! {
        while let Some(next) = resp.frame().await {
            match next {
                Ok(frame) => {
                    for chunk in frame.data_ref().into_iter() {
                        yield Ok(chunk.clone());
                    }
                }
                Err(e) => {
                    yield Err(e.into());
                }
            }
        }
    }
}

// Consume all the bytes from the response body stream and return them.
async fn get_response_bytes(resp: Response<Incoming>) -> Result<Vec<u8>, DockerClientError> {
    let mut resp_bytes = vec![];
    let resp_stream = resp_to_stream(resp);
    pin_mut!(resp_stream); // needed for iteration
    while let Some(value) = resp_stream.next().await {
        resp_bytes.extend(value?);
    }

    Ok(resp_bytes)
}
