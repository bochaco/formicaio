use crate::types::{InactiveReason, NodeId, NodeInstanceInfo};

#[cfg(feature = "ssr")]
use crate::{
    docker_client::{
        LABEL_KEY_HOME_NETWORK_DISABLED, LABEL_KEY_METRICS_PORT, LABEL_KEY_NODE_LOGS_DISABLED,
        LABEL_KEY_NODE_PORT, LABEL_KEY_REWARDS_ADDR, LABEL_KEY_UPNP_DISABLED,
    },
    types::NodeStatus,
};

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
};

const DEFAULT_NODE_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Container {
    pub Id: NodeId,
    pub Created: u64,
    pub Ports: Vec<Port>,
    pub State: ContainerState,
    pub Status: String,
    pub Labels: HashMap<String, String>,
    pub NetworkSettings: Networks,
}

// some helper methods to extract values from it
impl Container {
    pub fn port(&self) -> Option<u16> {
        self.Labels
            .get(LABEL_KEY_NODE_PORT)
            .map(|v| v.parse::<u16>().unwrap_or_default())
    }
    pub fn metrics_port(&self) -> Option<u16> {
        self.Labels
            .get(LABEL_KEY_METRICS_PORT)
            .map(|v| v.parse::<u16>().unwrap_or_default())
    }
    pub fn node_ip(&self) -> Option<String> {
        self.NetworkSettings.Networks.get("bridge").and_then(|n| {
            if n.IPAddress.is_empty() {
                None
            } else {
                Some(n.IPAddress.clone())
            }
        })
    }
}

impl From<Container> for NodeInstanceInfo {
    fn from(val: Container) -> Self {
        Self {
            node_id: val.Id.clone(),
            created: val.Created,
            status: NodeStatus::from(&val.State),
            status_info: if val.State == ContainerState::created {
                "".to_string()
            } else {
                val.Status.clone()
            },
            port: val.port(),
            metrics_port: val.metrics_port(),
            node_ip: Some(
                val.node_ip()
                    .map_or(DEFAULT_NODE_IP, |v| v.parse().unwrap_or(DEFAULT_NODE_IP)),
            ),
            rewards_addr: val.Labels.get(LABEL_KEY_REWARDS_ADDR).cloned(),
            home_network: !val.Labels.contains_key(LABEL_KEY_HOME_NETWORK_DISABLED),
            upnp: !val.Labels.contains_key(LABEL_KEY_UPNP_DISABLED),
            node_logs: !val.Labels.contains_key(LABEL_KEY_NODE_LOGS_DISABLED),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Networks {
    pub Networks: HashMap<String, Network>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Network {
    pub IPAddress: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Port {
    pub IP: Option<String>,
    pub PrivatePort: u64,
    pub PublicPort: Option<u64>,
    pub Type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

#[cfg(feature = "ssr")]
impl From<&ContainerState> for NodeStatus {
    fn from(item: &ContainerState) -> NodeStatus {
        match item {
            ContainerState::created => NodeStatus::Inactive(InactiveReason::Created),
            ContainerState::restarting => NodeStatus::Restarting,
            ContainerState::running => NodeStatus::Active,
            ContainerState::removing => NodeStatus::Removing,
            ContainerState::paused | ContainerState::exited | ContainerState::dead => {
                NodeStatus::Inactive(InactiveReason::Stopped)
            }
        }
    }
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
    pub StopSignal: Option<String>,
    pub StopTimeout: Option<i64>,
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

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ContainerUpdate {
    pub RestartPolicy: Option<RestartPolicy>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct RestartPolicy {
    pub Name: String,
    pub MaximumRetryCount: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerErrorMessage {
    pub message: String,
}
