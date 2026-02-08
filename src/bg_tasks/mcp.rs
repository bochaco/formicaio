use super::mcp_tools::*;
use crate::{app_context::AppContext, node_mgr::NodeManager};

use async_trait::async_trait;
use leptos::logging;
use rust_mcp_sdk::{
    McpServer, ToMcpServerHandler,
    event_store::InMemoryEventStore,
    mcp_icon,
    mcp_server::{HyperServerOptions, ServerHandler, hyper_server},
    schema::{
        CallToolRequestParams, CallToolResult, Implementation, InitializeResult,
        LATEST_PROTOCOL_VERSION, ListToolsResult, PaginatedRequestParams, RpcError,
        ServerCapabilities, ServerCapabilitiesTools, schema_utils::CallToolError,
    },
};
use std::{net::SocketAddr, sync::Arc, time::Duration};

pub struct McpServerHandler {
    app_ctx: AppContext,
    node_manager: NodeManager,
}

#[async_trait]
impl ServerHandler for McpServerHandler {
    // Handle ListToolsRequest, return list of available tools as ListToolsResult
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: FormicaioTools::tools(),
        })
    }

    /// Handles incoming CallToolRequest and processes it using the appropriate tool.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        // Attempt to convert request parameters into FormicaioTools enum
        let tool_params: FormicaioTools =
            FormicaioTools::try_from(params).map_err(CallToolError::new)?;

        // Match the tool variant and execute its corresponding logic
        match tool_params {
            FormicaioTools::FetchStats(tool) => tool.call_tool(&self.app_ctx).await,
            FormicaioTools::NodeInstances(tool) => {
                tool.call_tool(&self.app_ctx, &self.node_manager).await
            }
            FormicaioTools::CreateNodeInstance(tool) => tool.call_tool(&self.node_manager).await,
            FormicaioTools::StartNodeInstance(tool) => tool.call_tool(&self.node_manager).await,
            FormicaioTools::StopNodeInstance(tool) => tool.call_tool(&self.node_manager).await,
            FormicaioTools::DeleteNodeInstance(tool) => tool.call_tool(&self.node_manager).await,
            FormicaioTools::UpgradeNodeInstance(tool) => tool.call_tool(&self.node_manager).await,
            FormicaioTools::RecycleNodeInstance(tool) => tool.call_tool(&self.node_manager).await,
        }
    }
}

// Kick off the MCP server to listen on the given address and port.
pub fn start_mcp_server(addr: SocketAddr, app_ctx: AppContext, node_manager: NodeManager) {
    // Define server details and capabilities
    let server_details = InitializeResult {
        // server name and version
        server_info: Implementation {
            name: "Formicaio MCP Server SSE".to_string(),
            version: "0.1.0".to_string(),
            title: Some("Formicaio MCP Server SSE".to_string()),
            description: Some("Formicaio MCP Server SSE".into()),
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/bochaco/formicaio/refs/heads/main/public/formicaio.svg",
                mime_type = "image/svg",
                sizes = ["128x128"],
                theme = "dark"
            )],
            website_url: Some("https://github.com/bochaco/formicaio".into()),
        },
        capabilities: ServerCapabilities {
            // indicates that server support mcp tools
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default() // Using default values for other fields
        },
        meta: None,
        instructions: Some("Formicaio MCP Server - Use 'ListTools' to discover available node management tools. Connect via HTTP SSE or standard MCP protocols.".to_string()),
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
    };

    // instantiate our custom handler for handling MCP messages
    let handler = McpServerHandler {
        app_ctx: app_ctx.clone(),
        node_manager,
    };

    // instantiate HyperServer, providing `server_details`, `handler` and HyperServerOptions
    let server = hyper_server::create_server(
        server_details,
        handler.to_mcp_server_handler(),
        HyperServerOptions {
            host: addr.ip().to_string(),
            port: addr.port(),
            sse_support: false,
            ping_interval: Duration::from_secs(5),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            ..Default::default()
        },
    );

    // Start the server
    tokio::spawn(async move {
        let server_info = server
            .server_info(None)
            .await
            .unwrap_or_else(|err| err.to_string());
        logging::log!("{server_info}");
        if let Some(info) = server_info.find("http://") {
            *app_ctx.mcp_status.write().await = Some(server_info[info..].to_string());
        } else {
            *app_ctx.mcp_status.write().await = Some("unknown".to_string());
        }

        if let Err(err) = server.start().await {
            logging::error!("Failed to start MCP server: {err:?}");
            *app_ctx.mcp_status.write().await = None;
        }
    });
}
