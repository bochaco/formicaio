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
        let args = match parse_tool_args(&tool_call.function.arguments) {
            Ok(v) => v,
            Err(e) => return json_error(&format!("bad args: {e}")),
        };

        logging::log!("[Agent] Executing tool '{name}' with args: {args}");

        // Reject placeholder node IDs before they reach the node manager.
        // Small LLMs sometimes pass template strings like "[ID of each stopped node]"
        // when they haven't fetched real IDs yet. Return a clear error so the model
        // self-corrects and calls nodes_instances first.
        if let Some(node_id) = args.get("node_id").and_then(|v| v.as_str())
            && is_placeholder(node_id)
        {
            let err = json_error(&format!(
                "Invalid node_id '{node_id}': this looks like a placeholder, not a real node ID. \
                 Call nodes_instances first to obtain the actual node IDs, then call this tool \
                 again with a real ID from that result."
            ));
            logging::warn!("[Agent] Placeholder node_id rejected for tool '{name}': {node_id}");
            return err;
        }

        let result = match name.as_str() {
            "fetch_stats" => {
                let tool = FetchStats {};
                match tool.call_tool(&self.app_ctx).await {
                    Ok(r) => extract_text_or_ok(&r),
                    Err(e) => json_error(&e.to_string()),
                }
            }
            "nodes_instances" => {
                let tool = NodeInstances {};
                match tool.call_tool(&self.app_ctx, &self.node_manager).await {
                    Ok(r) => transform_nodes_for_llm(&extract_text_or_ok(&r)),
                    Err(e) => json_error(&e.to_string()),
                }
            }
            "create_node_instance" => match build_create_node_instance(&args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(r) => extract_text_or_ok(&r),
                    Err(e) => json_error(&e.to_string()),
                },
                Err(e) => json_error(&e),
            },
            "start_node_instance" => match serde_json::from_value::<StartNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => json_status("started"),
                    Err(e) => json_error(&e.to_string()),
                },
                Err(e) => json_error(&format!("bad args: {e}")),
            },
            "stop_node_instance" => match serde_json::from_value::<StopNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => json_status("stopped"),
                    Err(e) => json_error(&e.to_string()),
                },
                Err(e) => json_error(&format!("bad args: {e}")),
            },
            "delete_node_instance" => match serde_json::from_value::<DeleteNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => json_status("deleted"),
                    Err(e) => json_error(&e.to_string()),
                },
                Err(e) => json_error(&format!("bad args: {e}")),
            },
            "upgrade_node_instance" => match serde_json::from_value::<UpgradeNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => json_status("upgraded"),
                    Err(e) => json_error(&e.to_string()),
                },
                Err(e) => json_error(&format!("bad args: {e}")),
            },
            "recycle_node_instance" => match serde_json::from_value::<RecycleNodeInstance>(args) {
                Ok(tool) => match tool.call_tool(&self.node_manager).await {
                    Ok(_) => json_status("recycled"),
                    Err(e) => json_error(&e.to_string()),
                },
                Err(e) => json_error(&format!("bad args: {e}")),
            },
            unknown => {
                logging::warn!("[Agent] Unknown tool requested: {unknown}");
                json_error(&format!("Unknown tool: {unknown}"))
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

fn extract_text_or_ok(result: &rust_mcp_sdk::schema::CallToolResult) -> String {
    let text = result
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
        .join("");

    if text.is_empty() {
        json_status("ok")
    } else {
        text
    }
}

/// Converts the raw `nodes_instances` HashMap JSON (keyed by node ID, with complex Rust enum
/// status values) into a flat, LLM-friendly list. Small models struggle to extract map keys
/// and parse nested enum variants like `{"Inactive":{"Stopped":null}}`, so we normalize to
/// a simple array with explicit `node_id` and human-readable `status` fields.
fn transform_nodes_for_llm(raw: &str) -> String {
    let map: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(_) => return raw.to_string(),
    };

    if map.is_empty() {
        return "{\"node_count\": 0, \"nodes\": [], \"message\": \
                \"There are NO node instances in Formicaio. The instance list is genuinely empty.\"}"
            .to_string();
    }

    let nodes: Vec<serde_json::Value> = map
        .iter()
        .map(|(key, v)| {
            json!({
                "node_id": key,
                "status": node_status_string(v.get("status")),
                "peer_id": v.get("peer_id"),
                "port": v.get("port"),
                "connected_peers": v.get("connected_peers"),
                "records": v.get("records"),
                "balance": v.get("balance"),
                "bin_version": v.get("bin_version"),
            })
        })
        .collect();

    let count = nodes.len();
    serde_json::to_string(&json!({ "node_count": count, "nodes": nodes }))
        .unwrap_or_else(|_| raw.to_string())
}

/// Flattens a Rust-serialized NodeStatus enum value into a plain string.
/// Handles variants like `"Active"`, `{"Inactive": {"Stopped": null}}`,
/// `{"Inactive": {"StartFailed": "..."}}`, etc.
fn node_status_string(status: Option<&serde_json::Value>) -> String {
    let Some(v) = status else {
        return "Unknown".to_string();
    };
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(obj) = v.as_object()
        && let Some((variant, inner)) = obj.iter().next()
    {
        if variant == "Inactive" {
            let reason = match inner {
                v if v.is_object() => v
                    .as_object()
                    .and_then(|o| o.keys().next().map(|k| k.as_str().to_string()))
                    .unwrap_or_else(|| "Unknown".to_string()),
                v if v.is_string() => v.as_str().unwrap_or("Unknown").to_string(),
                _ => "Unknown".to_string(),
            };
            return format!("Inactive/{reason}");
        }
        return variant.clone();
    }
    v.to_string()
}

/// Returns true if `s` looks like a template placeholder rather than a real node ID.
/// Catches patterns like "[node_id]", "[ID of stopped node]", "<node_id>", "node-id", etc.
fn is_placeholder(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    (s.starts_with('[') && s.ends_with(']'))
        || (s.starts_with('<') && s.ends_with('>'))
        || s.contains("placeholder")
        || s.contains("node_id")
        || s.contains("each node")
        || s.contains("stopped node")
        || s.contains("inactive node")
}

fn build_create_node_instance(args: &Value) -> Result<CreateNodeInstance, String> {
    let port = parse_u16_arg(args, "port")?;
    let metrics_port = parse_u16_arg(args, "metrics_port")?;

    Ok(CreateNodeInstance {
        node_ip: args["node_ip"]
            .as_str()
            .ok_or("missing node_ip")?
            .to_string(),
        port,
        metrics_port,
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

fn parse_tool_args(arguments: &str) -> Result<Value, String> {
    let args: Value = serde_json::from_str(arguments).map_err(|e| e.to_string())?;
    if !args.is_object() {
        return Err("arguments must be a JSON object".to_string());
    }
    Ok(args)
}

fn parse_u16_arg(args: &Value, name: &str) -> Result<u16, String> {
    let value = args[name]
        .as_u64()
        .ok_or_else(|| format!("missing {name}"))?;
    u16::try_from(value).map_err(|_| format!("{name} must be <= {}", u16::MAX))
}

fn json_error(msg: &str) -> String {
    serde_json::to_string(&json!({ "error": msg }))
        .unwrap_or_else(|_| "{\"error\":\"internal serialization error\"}".to_string())
}

fn json_status(status: &str) -> String {
    serde_json::to_string(&json!({ "status": status }))
        .unwrap_or_else(|_| "{\"status\":\"unknown\"}".to_string())
}
