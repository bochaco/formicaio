use super::metadata_db::DbClient;

use bytes::Bytes;
use dyn_fmt::AsStrFormatExt;
use futures_util::Stream;
use leptos::*;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;

const PORTAINER_API_BASE_URL: &str = "http://127.0.0.1:9000/api/endpoints";
const PORTAINER_CONTAINER_API: &str = "/docker/containers";
const PORTAINER_EXEC_API: &str = "/docker/exec";

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
pub struct ContainerCreateEnvSuccess {
    pub Id: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerExecJson {
    pub Running: bool,
    pub ExitCode: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerErrorMessage {
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct PortainerAuthRequest {
    Username: Option<String>,
    Password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PortainerAuthResponse {
    jwt: String,
}

#[derive(Debug, Error)]
pub enum PortainerError {
    #[error("Unauthorised access to Portainer server")]
    PortainerUnauthorised,
    #[error("Portainer environment id invalid: {0}")]
    PortainerEnvInvalid(String),
    #[error("Container not found with id: {0}")]
    CointainerNotFound(ContainerId),
    #[error("Portainer server error: {0}")]
    PortainerServerError(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

// Client to send requests to a Portainer server's API
#[derive(Clone, Debug)]
pub struct PortainerClient {
    token: Arc<Mutex<String>>,
    portainer_env_id: Arc<Mutex<String>>,
    reqwest_client: Client,
    db_client: DbClient,
}

// TODO: read these values from env vars
const PORTAINER_USERNAME: &str = "admin";
const PORTAINER_PASSWORD: &str = "adminpassword";

impl PortainerClient {
    // Instantiate a Portainer client,
    // automatically authorising itself to the server.
    // TODO: if the server is not initialised yet with an admin account,
    // we need to create it here.
    pub async fn new(db_client: DbClient) -> Result<Self, PortainerError> {
        let reqwest_client = Client::new();
        let portainer_env_id = Arc::new(Mutex::new(db_client.get_portainer_env_id().await));
        let client = Self {
            token: Arc::new(Mutex::new("".to_string())),
            portainer_env_id,
            reqwest_client,
            db_client,
        };
        client.login().await?;

        Ok(client)
    }

    // Log into Portainer server, caching the new token obtained on the local DB.
    async fn login(&self) -> Result<(), PortainerError> {
        logging::log!("Logging into Portainer server...");
        let url = "http://127.0.0.1:9000/api/auth";
        let body = PortainerAuthRequest {
            Username: Some(PORTAINER_USERNAME.to_string()),
            Password: Some(PORTAINER_PASSWORD.to_string()),
        };
        let resp = self.reqwest_client.post(url).json(&body).send().await?;
        match resp.status() {
            StatusCode::OK => {
                let auth = resp.json::<PortainerAuthResponse>().await?;
                *self.token.lock().await = auth.jwt;
                Ok(())
            }
            other => {
                let msg = resp.json::<ServerErrorMessage>().await?;
                logging::log!("ERROR: {other:?} - {}", msg.message);
                Err(PortainerError::PortainerServerError(msg.message))
            }
        }
    }

    // Get the currently used Portainer environment Id.
    async fn portainer_env_id(&self) -> String {
        self.portainer_env_id.lock().await.clone()
    }

    // Query the Portainer server to return the info of the container matching the given id
    pub async fn get_container_info(&self, id: &ContainerId) -> Result<Container, PortainerError> {
        let mut filters = HashMap::default();
        filters.insert("id".to_string(), vec![id.clone()]);
        let containers = self.list_containers(&filters).await?;
        containers
            .into_iter()
            .next()
            .ok_or(PortainerError::CointainerNotFound(id.clone()))
    }

    // Query the Portainer server to return the list of ALL existing containers.
    pub async fn get_containers_list(&self) -> Result<Vec<Container>, PortainerError> {
        let mut filters = HashMap::default();
        filters.insert(
            "ancestor".to_string(),
            vec![NODE_CONTAINER_IMAGE_NAME.to_string()],
        );
        self.list_containers(&filters).await
    }

    // Query the Portainer server to return a LIST of existing containers using the given filter.
    async fn list_containers(
        &self,
        filters: &HashMap<String, Vec<String>>,
    ) -> Result<Vec<Container>, PortainerError> {
        let url = format!("{PORTAINER_API_BASE_URL}/{{}}{PORTAINER_CONTAINER_API}/json",);
        logging::log!("Sending Portainer query to get LIST of containers: {url} ...");
        let query = &[
            ("all", "true"),
            ("filters", &serde_json::to_string(filters)?),
        ];
        let resp = self.get_request(&url, query).await?;

        match resp.status() {
            StatusCode::OK => {
                let containers = resp.json::<Vec<Container>>().await?;
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
    pub async fn delete_container_with(&self, id: &ContainerId) -> Result<(), PortainerError> {
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/{id}",
            self.portainer_env_id().await
        );
        logging::log!("Sending Portainer request to DELETE containers: {url} ...");
        let query = &[("force", "true")];
        let token = self.token.lock().await;
        let resp = Client::new()
            .delete(&url)
            .bearer_auth(&token)
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
    pub async fn start_container_with(&self, id: &ContainerId) -> Result<(), PortainerError> {
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/{id}/start",
            self.portainer_env_id().await
        );
        logging::log!("Sending Portainer request to START a container: {url} ...");
        let resp = self.post_request(&url, &[], &()).await?;

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
    pub async fn stop_container_with(&self, id: &ContainerId) -> Result<(), PortainerError> {
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/{id}/stop",
            self.portainer_env_id().await
        );
        logging::log!("Sending Portainer request to STOP a container: {url} ...");
        let resp = self.post_request(&url, &[], &()).await?;

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
        &self,
        port: u16,
        rpc_api_port: u16,
    ) -> Result<ContainerId, PortainerError> {
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/create",
            self.portainer_env_id().await
        );
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
        let resp = self
            .post_request(&url, &[("name", &random_name)], &container_create_req)
            .await?;

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
        &self,
        container_id: &ContainerId,
    ) -> Result<impl Stream<Item = reqwest::Result<Bytes>>, PortainerError> {
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/{container_id}/logs",
            self.portainer_env_id().await
        );
        logging::log!("Sending Portainer query to get container LOGS stream: {url} ...");
        let query = &[
            ("stdout", "true"),
            ("stderr", "true"),
            ("follow", "true"),
            ("tail", "20"),
        ];
        let resp = self.get_request(&url, query).await?;

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
    pub async fn upgrade_node_in_container_with(
        &self,
        id: &ContainerId,
    ) -> Result<(), PortainerError> {
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/{id}/exec",
            self.portainer_env_id().await
        );
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
        let resp = self.post_request(&url, &[], &exec_cmd).await?;
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
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_EXEC_API}/{exec_id}/start",
            self.portainer_env_id().await
        );
        let opts = ContainerExecStart {
            Detach: Some(false),
            Tty: Some(true),
        };
        let resp = self.post_request(&url, &[], &opts).await?;
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
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_EXEC_API}/{exec_id}/json",
            self.portainer_env_id().await
        );
        let resp = self.get_request(&url, &[]).await?;
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
        let url = format!(
            "{PORTAINER_API_BASE_URL}/{}{PORTAINER_CONTAINER_API}/{id}/restart",
            self.portainer_env_id().await
        );
        logging::log!("Sending Portainer request to RESTART a container: {url} ...");
        let resp = self.post_request(&url, &[], &()).await?;
        match resp.status() {
            StatusCode::NO_CONTENT => Ok(()),
            other => {
                let msg = resp.json::<ServerErrorMessage>().await?;
                logging::log!("ERROR: {other:?} - {}", msg.message);
                Err(PortainerError::PortainerServerError(msg.message))
            }
        }
    }

    // Send GET request to Portainer server, creating a new env if necessary
    async fn get_request(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Response, PortainerError> {
        match self.try_get_request(url, query).await {
            Err(PortainerError::PortainerUnauthorised) => {
                logging::log!("We need to log in to Portainer server.");
                // let's log in and obtain a new token before retrying
                self.login().await?;
                self.try_get_request(url, query).await
            }
            Err(PortainerError::PortainerEnvInvalid(id)) => {
                logging::log!("We need to create a new Portainer environment since current it's invalid: {id}.");
                // let's create a new env before retrying
                self.new_environment().await?;
                self.try_get_request(url, query).await
            }
            other => other,
        }
    }

    // Send POST request to Portainer server, creating a new env if necessary
    async fn post_request<T: Serialize + ?Sized>(
        &self,
        url: &str,
        query: &[(&str, &str)],
        body: &T,
    ) -> Result<Response, PortainerError> {
        match self.try_post_request(url, query, body).await {
            Err(PortainerError::PortainerUnauthorised) => {
                logging::log!("We need to log in to Portainer server.");
                self.login().await?;
                self.try_post_request(url, query, body).await
            }
            Err(PortainerError::PortainerEnvInvalid(id)) => {
                logging::log!("We need to create a new Portainer environment since current it's invalid: {id}.");
                // let's create a new env before retrying
                self.new_environment().await?;
                self.try_post_request(url, query, body).await
            }
            other => other,
        }
    }

    // Creates a new Portainer environment and set the new env id.
    async fn new_environment(&self) -> Result<(), PortainerError> {
        logging::log!(
            "Sending Portainer request to CREATE ENVIRONMENT: {PORTAINER_API_BASE_URL} ..."
        );
        let env_name = format!("env-name-{}", hex::encode(rand::random::<[u8; 10]>()));
        let query = &[("Name", env_name.as_str()), ("EndpointCreationType", "1")];
        let resp = self
            .try_post_request(PORTAINER_API_BASE_URL, query, &())
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let env = resp.json::<ContainerCreateEnvSuccess>().await?;
                let id = env.Id.to_string();
                logging::log!("New Portainer env ID: {id}");
                self.db_client.update_portainer_env_id(id.clone()).await;
                *self.portainer_env_id.lock().await = id.clone();
                Ok(())
            }
            other => {
                let msg = resp.json::<ServerErrorMessage>().await?;
                logging::log!("ERROR: {other:?} - {}", msg.message);
                Err(PortainerError::PortainerServerError(msg.message))
            }
        }
    }

    // Send GET request to Portainer server
    async fn try_get_request(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Response, PortainerError> {
        let env_id = self.portainer_env_id().await;
        let url_with_env_id = url.format(&[env_id.clone()]);
        let token = self.token.lock().await.clone();
        let resp = self
            .reqwest_client
            .get(url_with_env_id)
            .bearer_auth(&token)
            .query(query)
            .send()
            .await?;

        match resp.status() {
            StatusCode::UNAUTHORIZED => Err(PortainerError::PortainerUnauthorised),
            StatusCode::NOT_FOUND => Err(PortainerError::PortainerEnvInvalid(env_id)),
            _other => Ok(resp),
        }
    }

    // Send POST request to Portainer server
    async fn try_post_request<T: Serialize + ?Sized>(
        &self,
        url: &str,
        query: &[(&str, &str)],
        body: &T,
    ) -> Result<Response, PortainerError> {
        let env_id = self.portainer_env_id().await;
        let url_with_env_id = url.format(&[env_id.clone()]);
        let token = self.token.lock().await.clone();
        let resp = self
            .reqwest_client
            .post(url_with_env_id)
            .bearer_auth(&token)
            .query(query)
            .json(body)
            .send()
            .await?;

        match resp.status() {
            StatusCode::UNAUTHORIZED => Err(PortainerError::PortainerUnauthorised),
            StatusCode::NOT_FOUND => Err(PortainerError::PortainerEnvInvalid(env_id)),
            _other => Ok(resp),
        }
    }
}
