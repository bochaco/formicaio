use crate::{
    app_context::AppContext, node_mgr::NodeManager, server_api::parse_and_validate_addr,
    types::NodeOpts,
};

use rust_mcp_sdk::{
    macros::{JsonSchema, mcp_tool},
    schema::{CallToolResult, TextContent, schema_utils::CallToolError},
    tool_box,
};
use std::{net::IpAddr, path::PathBuf, str::FromStr};

// Serialise object and return a result ready to return as the tool response
fn serialise_to_tool_response<T: serde::Serialize>(
    object: &T,
) -> Result<CallToolResult, CallToolError> {
    match serde_json::to_string(object) {
        Ok(str) => Ok(CallToolResult::text_content(vec![TextContent::from(str)])),
        Err(err) => Err(CallToolError::from_message(err.to_string())),
    }
}

#[mcp_tool(
    name = "fetch_stats",
    description = "Return up to date aggregated nodes stats."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct FetchStats {}
impl FetchStats {
    pub async fn call_tool(&self, app_ctx: &AppContext) -> Result<CallToolResult, CallToolError> {
        let stats = app_ctx.stats.read().await.clone();
        serialise_to_tool_response(&stats)
    }
}

#[mcp_tool(
    name = "nodes_instances",
    description = "Retrieve the list of node instances and their configured options."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct NodeInstances {}
impl NodeInstances {
    pub async fn call_tool(
        &self,
        app_ctx: &AppContext,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        match node_manager
            .filtered_nodes_list(None, app_ctx.nodes_metrics.clone())
            .await
        {
            Ok(nodes) => serialise_to_tool_response(&nodes),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

#[mcp_tool(
    name = "create_node_instance",
    description = "Create a new node instance."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct CreateNodeInstance {
    /// Listening IP address set by the user for the node (IPv4 or IPv6, including special values like `0.0.0.0` or `::`)
    pub node_ip: String,
    /// TCP port used by the node for main operations
    pub port: u16,
    /// TCP port used by the node for metrics reporting
    pub metrics_port: u16,
    /// Hex-encoded rewards address for the node
    pub rewards_addr: String,
    /// Whether UPnP is enabled for this node
    pub upnp: bool,
    /// Whether reachability check is enabled for this node
    pub reachability_check: bool,
    /// Whether node logs are enabled for this node
    pub node_logs: bool,
    /// Whether to automatically start the node after creation
    pub auto_start: bool,
    /// Custom data directory path for this node instance
    pub data_dir_path: String,
}
impl CreateNodeInstance {
    pub async fn call_tool(
        &self,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        // validate rewards address before proceeding
        if let Err(err) = parse_and_validate_addr(&self.rewards_addr) {
            return Err(CallToolError::from_message(err.to_string()));
        }

        let node_opts = NodeOpts {
            node_ip: IpAddr::from_str(&self.node_ip)
                .map_err(|err| CallToolError::from_message(err.to_string()))?,
            port: self.port,
            metrics_port: self.metrics_port,
            rewards_addr: self.rewards_addr.clone(),
            upnp: self.upnp,
            reachability_check: self.reachability_check,
            node_logs: self.node_logs,
            auto_start: self.auto_start,
            data_dir_path: PathBuf::from(&self.data_dir_path),
        };

        match node_manager.create_node_instance(node_opts).await {
            Ok(info) => serialise_to_tool_response(&info),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

#[mcp_tool(
    name = "start_node_instance",
    description = "Start node instance with given Id."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct StartNodeInstance {
    node_id: String,
}
impl StartNodeInstance {
    pub async fn call_tool(
        &self,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        match node_manager
            .start_node_instance(self.node_id.clone().into())
            .await
        {
            Ok(()) => Ok(CallToolResult::text_content(vec![])),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

#[mcp_tool(
    name = "stop_node_instance",
    description = "Stop node instance with given Id."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct StopNodeInstance {
    node_id: String,
}
impl StopNodeInstance {
    pub async fn call_tool(
        &self,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        match node_manager
            .stop_node_instance(self.node_id.clone().into())
            .await
        {
            Ok(()) => Ok(CallToolResult::text_content(vec![])),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

#[mcp_tool(
    name = "delete_node_instance",
    description = "Delete node instance with given Id."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct DeleteNodeInstance {
    node_id: String,
}
impl DeleteNodeInstance {
    pub async fn call_tool(
        &self,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        match node_manager
            .delete_node_instance(self.node_id.clone().into())
            .await
        {
            Ok(()) => Ok(CallToolResult::text_content(vec![])),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

#[mcp_tool(
    name = "upgrade_node_instance",
    description = "Upgrade node instance with given Id."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct UpgradeNodeInstance {
    node_id: String,
}
impl UpgradeNodeInstance {
    pub async fn call_tool(
        &self,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        match node_manager
            .upgrade_node_instance(&self.node_id.clone().into())
            .await
        {
            Ok(()) => Ok(CallToolResult::text_content(vec![])),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

#[mcp_tool(
    name = "recycle_node_instance",
    description = "Recycle node instance with given Id."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct RecycleNodeInstance {
    node_id: String,
}
impl RecycleNodeInstance {
    pub async fn call_tool(
        &self,
        node_manager: &NodeManager,
    ) -> Result<CallToolResult, CallToolError> {
        match node_manager
            .recycle_node_instance(self.node_id.clone().into())
            .await
        {
            Ok(()) => Ok(CallToolResult::text_content(vec![])),
            Err(err) => Err(CallToolError::from_message(err.to_string())),
        }
    }
}

// Generates an enum named FormicaioTools, list of available tools.
tool_box!(
    FormicaioTools,
    [
        FetchStats,
        NodeInstances,
        CreateNodeInstance,
        StartNodeInstance,
        StopNodeInstance,
        DeleteNodeInstance,
        UpgradeNodeInstance,
        RecycleNodeInstance
    ]
);
