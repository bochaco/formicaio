use super::llm_client::{FunctionDefinition, LlmToolCall, ToolDefinition};
use crate::{app_context::AppContext, bg_tasks::mcp_tools::*, node_mgr::NodeManager};

use leptos::logging;
use serde_json::{Value, json};

/// Dispatches LLM tool calls directly to the existing `mcp_tools` functions,
/// avoiding any HTTP round-trip through the external MCP server.
pub struct ToolExecutor {
    app_ctx: AppContext,
    node_manager: NodeManager,
}

impl ToolExecutor {
    pub fn new(app_ctx: AppContext, node_manager: NodeManager) -> Self {
        Self {
            app_ctx,
            node_manager,
        }
    }

    /// Execute a tool call requested by the LLM. Returns a JSON string describing
    /// the result (suitable for injection as a `tool` role message).
    pub async fn execute(&self, tool_call: &LlmToolCall) -> String {
        let name = &tool_call.function.name;
        let args: Value = serde_json::from_str(&tool_call.function.arguments).unwrap_or(json!({}));

        logging::log!("[Agent] Executing tool '{name}' with args: {args}");

        let result = match name.as_str() {
            "fetch_stats" => {
                let tool = FetchStats {};
                match tool.call_tool(&self.app_ctx).await {
                    Ok(r) => extract_text(&r),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                }
            }
            "nodes_instances" => {
                let tool = NodeInstances {};
                match tool.call_tool(&self.app_ctx, &self.node_manager).await {
                    Ok(r) => {
                        let text = extract_text(&r);
                        // When the node map is empty it serialises to "{}".
                        // Make it unambiguous so the LLM doesn't hallucinate nodes.
                        if text.trim() == "{}" || text.trim() == "[]" {
                            "{\"node_count\": 0, \"nodes\": {}, \"message\": \
                             \"There are NO node instances in Formicaio. \
                             The instance list is genuinely empty.\"}"
                                .to_string()
                        } else {
                            // Prepend a count so the LLM knows how many nodes to expect
                            if let Ok(map) = serde_json::from_str::<
                                serde_json::Map<String, serde_json::Value>,
                            >(&text)
                            {
                                let count = map.len();
                                format!("{{\"node_count\": {count}, \"nodes\": {text}}}")
                            } else {
                                text
                            }
                        }
                    }
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                }
            }
            "create_node_instance" => match build_create_node_instance(&args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(r) => extract_text(&r),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                },
                Err(e) => format!("{{\"error\":\"{e}\"}}"),
            },
            "start_node_instance" => match serde_json::from_value::<StartNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => "{\"status\":\"started\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                },
                Err(e) => format!("{{\"error\":\"bad args: {e}\"}}"),
            },
            "stop_node_instance" => match serde_json::from_value::<StopNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => "{\"status\":\"stopped\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                },
                Err(e) => format!("{{\"error\":\"bad args: {e}\"}}"),
            },
            "delete_node_instance" => match serde_json::from_value::<DeleteNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => "{\"status\":\"deleted\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                },
                Err(e) => format!("{{\"error\":\"bad args: {e}\"}}"),
            },
            "upgrade_node_instance" => match serde_json::from_value::<UpgradeNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => "{\"status\":\"upgraded\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                },
                Err(e) => format!("{{\"error\":\"bad args: {e}\"}}"),
            },
            "recycle_node_instance" => match serde_json::from_value::<RecycleNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => "{\"status\":\"recycled\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{e}\"}}"),
                },
                Err(e) => format!("{{\"error\":\"bad args: {e}\"}}"),
            },
            unknown => {
                logging::warn!("[Agent] Unknown tool requested: {unknown}");
                format!("{{\"error\":\"Unknown tool: {unknown}\"}}")
            }
        };

        logging::log!("[Agent] Tool '{name}' result: {result}");
        result
    }

    /// Returns the OpenAI-format tool definitions to pass in each chat request.
    pub fn tool_definitions() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "fetch_stats".to_string(),
                    description: "Return up-to-date aggregated statistics for all Formicaio nodes \
                         (total nodes, active nodes, total balance, stored records, \
                          estimated network size, connected peers, disk usage)."
                        .to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "nodes_instances".to_string(),
                    description: "Retrieve the list of all node instances and their current state \
                         (status, peers, records, balance, version, IP/port, disk usage)."
                        .to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "start_node_instance".to_string(),
                    description: "Start a stopped node instance by its ID.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "node_id": { "type": "string", "description": "The node ID to start" }
                        },
                        "required": ["node_id"]
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "stop_node_instance".to_string(),
                    description: "Stop a running node instance by its ID.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "node_id": { "type": "string", "description": "The node ID to stop" }
                        },
                        "required": ["node_id"]
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "recycle_node_instance".to_string(),
                    description: "Recycle a node instance (restart with a new peer ID). \
                         Useful to recover shunned or poorly-connected nodes."
                        .to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "node_id": { "type": "string", "description": "The node ID to recycle" }
                        },
                        "required": ["node_id"]
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "upgrade_node_instance".to_string(),
                    description: "Upgrade a node instance to the latest available binary version."
                        .to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "node_id": { "type": "string", "description": "The node ID to upgrade" }
                        },
                        "required": ["node_id"]
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "delete_node_instance".to_string(),
                    description: "Permanently delete a node instance and remove all its data. \
                         This action is irreversible."
                        .to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "node_id": { "type": "string", "description": "The node ID to delete" }
                        },
                        "required": ["node_id"]
                    }),
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_node_instance".to_string(),
                    description: "Create and optionally start a new node instance.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "node_ip": { "type": "string", "description": "Listening IP address (e.g. '0.0.0.0')" },
                            "port": { "type": "integer", "description": "Main TCP port" },
                            "metrics_port": { "type": "integer", "description": "Metrics TCP port" },
                            "rewards_addr": { "type": "string", "description": "Hex-encoded rewards address" },
                            "upnp": { "type": "boolean", "description": "Enable UPnP" },
                            "reachability_check": { "type": "boolean", "description": "Enable reachability check" },
                            "node_logs": { "type": "boolean", "description": "Enable node logs" },
                            "auto_start": { "type": "boolean", "description": "Auto-start after creation" },
                            "data_dir_path": { "type": "string", "description": "Custom data directory path" }
                        },
                        "required": ["node_ip", "port", "metrics_port", "rewards_addr", "upnp", "reachability_check", "node_logs", "auto_start", "data_dir_path"]
                    }),
                },
            },
        ]
    }

    /// Restricted tool set for the autonomous monitoring loop.
    /// Only read-only and safe-start tools are allowed — destructive operations
    /// (delete, recycle, upgrade, create) must not be available to the autonomous agent.
    pub fn autonomous_tool_definitions() -> Vec<ToolDefinition> {
        Self::tool_definitions()
            .into_iter()
            .filter(|t| {
                matches!(
                    t.function.name.as_str(),
                    "fetch_stats" | "nodes_instances" | "start_node_instance"
                )
            })
            .collect()
    }
}

fn extract_text(result: &rust_mcp_sdk::schema::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| {
            if let rust_mcp_sdk::schema::ContentBlock::TextContent(t) = c {
                Some(t.text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn build_create_node_instance(args: &Value) -> Result<CreateNodeInstance, String> {
    Ok(CreateNodeInstance {
        node_ip: args["node_ip"]
            .as_str()
            .ok_or("missing node_ip")?
            .to_string(),
        port: args["port"].as_u64().ok_or("missing port")? as u16,
        metrics_port: args["metrics_port"]
            .as_u64()
            .ok_or("missing metrics_port")? as u16,
        rewards_addr: args["rewards_addr"]
            .as_str()
            .ok_or("missing rewards_addr")?
            .to_string(),
        upnp: args["upnp"].as_bool().unwrap_or(false),
        reachability_check: args["reachability_check"].as_bool().unwrap_or(true),
        node_logs: args["node_logs"].as_bool().unwrap_or(false),
        auto_start: args["auto_start"].as_bool().unwrap_or(true),
        data_dir_path: args["data_dir_path"].as_str().unwrap_or("").to_string(),
    })
}
