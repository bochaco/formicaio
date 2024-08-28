use bytes::Bytes;
use futures_util::Stream;
use leptos::*;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

const PORTAINER_API_BASE_URL: &str = "http://127.0.0.1:9000/api/endpoints/";
const PORTAINER_CONTAINER_API: &str = "/docker/containers";
const PORTAINER_EXEC_API: &str = "/docker/exec";

// FIXME: this has to be set once at start time of the app, once it logs into Portainer server
const ENV_ID: u64 = 4;
const TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpZCI6MSwidXNlcm5hbWUiOiJhZG1pbiIsInJvbGUiOjEsInNjb3BlIjoiZGVmYXVsdCIsImZvcmNlQ2hhbmdlUGFzc3dvcmQiOmZhbHNlLCJleHAiOjE3MjQ4Nzg4ODQsImlhdCI6MTcyNDg1MDA4NH0.KJi5vLUaW78Xh8-OTp1vju0oXBNB9N7JuLDHGFflUl4";

// Name of the Docker image to use for each node instance
const NODE_CONTAINER_IMAGE_NAME: &str = "formica";

// Hex-encoded container id
pub type ContainerId = String;

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Container {
    pub Id: ContainerId,
    pub Created: u64,
    pub Ports: Vec<Port>,
    pub State: ContainerState,
    pub Status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Port {
    pub IP: Option<String>,
    pub PrivatePort: u64,
    pub PublicPort: Option<u64>,
    pub Type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ContainerState {
    created,
    restarting,
    running,
    removing,
    paused,
    exited,
    dead,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct PortBinding {
    pub HostIp: Option<String>,
    pub HostPort: String,
}

pub type PortBindings = HashMap<String, Vec<PortBinding>>;

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct HostConfigCreate {
    pub NetworkMode: Option<String>,
    pub PublishAllPorts: Option<bool>,
    pub PortBindings: Option<PortBindings>,
}

pub type ExposedPorts = HashMap<String, HashMap<i32, i32>>;

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerCreate {
    pub Image: String,
    pub Labels: Option<HashMap<String, String>>,
    pub Env: Option<Vec<String>>,
    pub ExposedPorts: Option<ExposedPorts>,
    pub HostConfig: Option<HostConfigCreate>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerExec {
    pub AttachStdin: Option<bool>,
    pub AttachStdout: Option<bool>,
    pub AttachStderr: Option<bool>,
    pub Cmd: Option<Vec<String>>,
    pub Tty: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerExecStart {
    pub Detach: Option<bool>,
    pub Tty: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerCreateExecSuccess {
    pub Id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerExecJson {
    pub Running: bool,
    pub ExitCode: u8,
}

#[derive(Debug, Error)]
pub enum PortainerError {
    #[error("Container not found with id: {0}")]
    CointainerNotFound(ContainerId),
    #[error("Portainer server error: {0}")]
    PortainerServerError(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerErrorMessage {
    message: String,
}

// Query the Portainer server to return the info of the container matching the given id
pub async fn get_container_info(id: &ContainerId) -> Result<Container, PortainerError> {
    let mut filters = HashMap::default();
    filters.insert("id".to_string(), vec![id.clone()]);
    let containers = list_containers(&filters).await?;
    containers
        .into_iter()
        .next()
        .ok_or(PortainerError::CointainerNotFound(id.clone()))
}

// Query the Portainer server to return the list of ALL existing containers.
pub async fn get_containers_list() -> Result<Vec<Container>, PortainerError> {
    let mut filters = HashMap::default();
    filters.insert(
        "ancestor".to_string(),
        vec![NODE_CONTAINER_IMAGE_NAME.to_string()],
    );
    list_containers(&filters).await
}

// Query the Portainer server to return a LIST of existing containers using the given filter.
async fn list_containers(
    filters: &HashMap<String, Vec<String>>,
) -> Result<Vec<Container>, PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/json");
    logging::log!("Sending Portainer query to get LIST of containers: {url} ...");
    let query = &[
        ("all", "true"),
        ("filters", &serde_json::to_string(filters)?),
    ];
    let resp = get_request(&url, query).await?;

    match resp.status() {
        StatusCode::OK => {
            let containers = resp.json::<Vec<Container>>().await?;
            //logging::log!("{containers:#?}");
            Ok(containers)
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to DELETE a container matching the given id
pub async fn delete_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/{id}");
    logging::log!("Sending Portainer request to DELETE containers: {url} ...");
    let query = &[("force", "true")];
    let resp = Client::new()
        .delete(&url)
        .bearer_auth(TOKEN)
        .query(query)
        .send()
        .await?;

    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to START a container matching the given id
pub async fn start_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/{id}/start");
    logging::log!("Sending Portainer request to START a container: {url} ...");
    let resp = Client::new().post(&url).bearer_auth(TOKEN).send().await?;

    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to STOP a container matching the given id
pub async fn stop_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/{id}/stop");
    logging::log!("Sending Portainer request to STOP a container: {url} ...");
    let resp = post_request(&url, &[], &()).await?;

    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to CREATE a new node container, returning the container info.
pub async fn create_new_container(
    port: u16,
    rpc_api_port: u16,
) -> Result<ContainerId, PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/create");
    let mapped_ports = vec![port, rpc_api_port];
    let container_create_req = ContainerCreate {
        Image: NODE_CONTAINER_IMAGE_NAME.to_string(),
        Labels: None,
        Env: Some(vec![
            format!("NODE_PORT={port}"),
            format!("RPC_PORT={rpc_api_port}"),
        ]),
        ExposedPorts: Some(
            mapped_ports
                .iter()
                .map(|p| (format!("{p}/tcp"), HashMap::default()))
                .collect::<ExposedPorts>(),
        ),
        HostConfig: Some(HostConfigCreate {
            NetworkMode: None,
            PublishAllPorts: Some(false),
            PortBindings: Some(
                mapped_ports
                    .iter()
                    .map(|p| {
                        (
                            format!("{p}/tcp"),
                            vec![PortBinding {
                                HostIp: None,
                                HostPort: p.to_string(),
                            }],
                        )
                    })
                    .into_iter()
                    .collect::<PortBindings>(),
            ),
        }),
    };

    let random_name = hex::encode(rand::random::<[u8; 10]>().to_vec());
    logging::log!(
        "Sending Portainer request to CREATE a new container (named: {random_name}): {url} ..."
    );
    let resp = post_request(&url, &[("name", &random_name)], &container_create_req).await?;

    match resp.status() {
        StatusCode::OK => {
            let container = resp.json::<ContainerCreateExecSuccess>().await?;
            logging::log!("Container created successfully: {container:#?}");
            Ok(container.Id)
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to return a node container logs stream.
pub async fn get_container_logs_stream(
    container_id: &ContainerId,
) -> Result<impl Stream<Item = reqwest::Result<Bytes>>, PortainerError> {
    let url =
        format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/{container_id}/logs");
    logging::log!("Sending Portainer query to get container LOGS stream: {url} ...");
    let query = &[
        ("stdout", "true"),
        ("stderr", "true"),
        ("follow", "true"),
        ("tail", "20"),
    ];
    let resp = get_request(&url, query).await?;

    match resp.status() {
        StatusCode::OK => Ok(resp.bytes_stream()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to UPGRADE the node binary within a container matching the given id
pub async fn upgrade_node_in_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/{id}/exec");
    logging::log!("Sending Portainer request to UPGRADE node within a container: {url} ...");
    let exec_cmd = ContainerExec {
        AttachStdin: Some(false),
        AttachStdout: Some(true),
        AttachStderr: Some(true),
        Cmd: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "./safeup node -n -p /app".to_string(),
        ]),
        Tty: Some(false),
    };
    let resp = post_request(&url, &[], &exec_cmd).await?;
    let exec_id = match resp.status() {
        StatusCode::CREATED => {
            let exec_result = resp.json::<ContainerCreateExecSuccess>().await?;
            logging::log!("Container node upgrade cmd created successfully: {exec_result:#?}");
            exec_result.Id
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            return Err(PortainerError::PortainerServerError(msg.message));
        }
    };

    // let's now start the exec cmd created
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_EXEC_API}/{exec_id}/start");
    let opts = ContainerExecStart {
        Detach: Some(false),
        Tty: Some(true),
    };
    let resp = post_request(&url, &[], &opts).await?;
    match resp.status() {
        StatusCode::OK => {
            logging::log!("Node upgrade process finished in container: {id}");
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            return Err(PortainerError::PortainerServerError(msg.message));
        }
    }

    // let's check its exit code
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_EXEC_API}/{exec_id}/json");
    let resp = get_request(&url, &[]).await?;
    match resp.status() {
        StatusCode::OK => {
            let exec = resp.json::<ContainerExecJson>().await?;
            logging::log!("Container exec: {exec:#?}");
            if exec.ExitCode != 0 {
                // TODO: update UI
                logging::log!("Failed to upgrade node, exit code: {}", exec.ExitCode);
                return Ok(());
            }
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            return Err(PortainerError::PortainerServerError(msg.message));
        }
    }

    // restart container to run with new node version
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_CONTAINER_API}/{id}/restart");
    logging::log!("Sending Portainer request to RESTART a container: {url} ...");
    let resp = post_request(&url, &[], &()).await?;
    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!("ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Send GET request to Portainer server
async fn get_request(url: &str, query: &[(&str, &str)]) -> Result<Response, PortainerError> {
    let resp = Client::new()
        .get(url)
        .bearer_auth(TOKEN)
        .query(query)
        .send()
        .await?;

    match resp.status() {
        StatusCode::UNAUTHORIZED => {
            logging::log!("We need to log in to Portainer server.");
            // TODO: log into server
            Ok(resp)
        }
        _other => Ok(resp),
    }
}

// Send POST request to Portainer server
async fn post_request<T: Serialize + ?Sized>(
    url: &str,
    query: &[(&str, &str)],
    body: &T,
) -> Result<Response, PortainerError> {
    let resp = Client::new()
        .post(url)
        .bearer_auth(TOKEN)
        .query(query)
        .json(body)
        .send()
        .await?;

    match resp.status() {
        StatusCode::UNAUTHORIZED => {
            logging::log!("We need to log in to Portainer server.");
            // TODO: log into server
            Ok(resp)
        }
        _other => Ok(resp),
    }
}
