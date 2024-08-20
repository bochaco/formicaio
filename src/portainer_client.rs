use leptos::*;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

const PORTAINER_API_BASE_URL: &str = "http://127.0.0.1:9000/api/endpoints/";
const PORTAINER_API_BASE_PATH: &str = "/docker/containers";

// FIXME: this has to be set once at start time of the app, once it logs into Portainer server
const ENV_ID: u64 = 4;
const TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpZCI6MSwidXNlcm5hbWUiOiJhZG1pbiIsInJvbGUiOjEsInNjb3BlIjoiZGVmYXVsdCIsImZvcmNlQ2hhbmdlUGFzc3dvcmQiOmZhbHNlLCJleHAiOjE3MjQxOTA3NTIsImlhdCI6MTcyNDE2MTk1Mn0.IrzLV8CAargtSshd3xNPnzJPiP_l6e682WElUagWfeM";

// Name of the Docker image to use for each node instance
const NODE_CONTAINER_IMAGE_NAME: &str = "nginx:latest";

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
    pub ExposedPorts: Option<ExposedPorts>,
    pub HostConfig: Option<HostConfigCreate>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerCreateSuccess {
    pub Id: ContainerId,
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

// Query the Portainer server to return the info of the contianer matching the given id
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
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_API_BASE_PATH}/json");

    logging::log!("Sending Portainer query to get LIST of containers: {url}");
    let query = &[
        ("all", "true"),
        ("filters", &serde_json::to_string(filters)?),
    ];
    let resp = Client::new()
        .get(&url)
        .bearer_auth(TOKEN)
        .query(query)
        .send()
        .await?;

    // return error is status code is not 200 OK
    match resp.status() {
        StatusCode::OK => {
            let containers = resp.json::<Vec<Container>>().await?;
            logging::log!("{containers:#?}");
            Ok(containers)
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!(">> ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to DELETE a contianer matching the given id
pub async fn delete_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_API_BASE_PATH}/{id}");

    logging::log!("Sending Portainer request to DELETE containers: {url}");
    let query = &[("force", "true")];
    let resp = Client::new()
        .delete(&url)
        .bearer_auth(TOKEN)
        .query(query)
        .send()
        .await?;

    // return error is status code is not 204 NO CONTENT
    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!(">> ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to START a contianer matching the given id
pub async fn start_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_API_BASE_PATH}/{id}/start");

    logging::log!("Sending Portainer request to START a container: {url}");
    let resp = Client::new().post(&url).bearer_auth(TOKEN).send().await?;

    // return error is status code is not 204 NO CONTENT
    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!(">> ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to STOP a contianer matching the given id
pub async fn stop_container_with(id: &ContainerId) -> Result<(), PortainerError> {
    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_API_BASE_PATH}/{id}/stop");

    logging::log!("Sending Portainer request to STOP a container: {url}");
    let resp = Client::new().post(&url).bearer_auth(TOKEN).send().await?;

    // return error is status code is not 204 NO CONTENT
    match resp.status() {
        StatusCode::NO_CONTENT => Ok(()),
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!(">> ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}

// Request the Portainer server to create a new node container, returning the container info.
pub async fn create_new_container() -> Result<ContainerId, PortainerError> {
    let container_create_req = ContainerCreate {
        Image: "nginx:latest".to_string(),
        Labels: None,
        ExposedPorts: Some(
            vec![("80/tcp".to_string(), HashMap::default())]
                .into_iter()
                .collect::<ExposedPorts>(),
        ),
        HostConfig: Some(HostConfigCreate {
            NetworkMode: None,
            PublishAllPorts: Some(false),
            PortBindings: Some(
                vec![(
                    "80/tcp".to_string(),
                    vec![PortBinding {
                        HostIp: None,
                        HostPort: "8080".to_string(),
                    }],
                )]
                .into_iter()
                .collect::<PortBindings>(),
            ),
        }),
    };

    let url = format!("{PORTAINER_API_BASE_URL}{ENV_ID}{PORTAINER_API_BASE_PATH}/create");

    let random_name = hex::encode(rand::random::<[u8; 10]>().to_vec());
    logging::log!(
        "Sending Portainer request to CREATE a new container (named: {random_name}): {url}"
    );
    let resp = Client::new()
        .post(&url)
        .bearer_auth(TOKEN)
        .query(&[("name", random_name)])
        .json(&container_create_req)
        .send()
        .await?;

    // return error is status code is not 200 OK
    match resp.status() {
        StatusCode::OK => {
            let container = resp.json::<ContainerCreateSuccess>().await?;
            logging::log!("{container:#?}");
            Ok(container.Id)
        }
        other => {
            let msg = resp.json::<ServerErrorMessage>().await?;
            logging::log!(">> ERROR: {other:?} - {}", msg.message);
            Err(PortainerError::PortainerServerError(msg.message))
        }
    }
}
