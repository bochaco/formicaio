mod llm_client;
mod tool_executor;

pub use llm_client::{LlmClient, LlmMessage, OpenAiCompatClient, StreamEvent, ToolDefinition};
pub use tool_executor::ToolExecutor;

use crate::{
    app_context::AppContext,
    node_mgr::NodeManager,
    types::{AgentEventType, AppSettings, ChatMessage, StreamChunk},
};

use bytes::Bytes;
use futures_util::StreamExt;
use leptos::logging;
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::sync::{RwLock, broadcast};

// ─── In-memory chat session store ─────────────────────────────────────────────

/// Maximum number of chat sessions kept in memory at once.
/// Oldest session is evicted when this limit is reached.
const MAX_SESSIONS: usize = 20;

/// Maximum number of (role, content) pairs stored per session.
/// Older entries are trimmed when this limit is exceeded.
const MAX_MESSAGES_PER_SESSION: usize = 100;

/// In-memory store of chat conversation histories, keyed by session ID.
/// Automatically evicts the oldest session when `MAX_SESSIONS` is reached.
#[derive(Debug, Default)]
struct ChatSessions {
    order: VecDeque<String>,
    map: HashMap<String, Vec<ChatMessage>>,
}

impl ChatSessions {
    fn get(&self, session_id: &str) -> Vec<ChatMessage> {
        self.map.get(session_id).cloned().unwrap_or_default()
    }

    fn upsert(&mut self, session_id: String, history: Vec<ChatMessage>) {
        if !self.map.contains_key(&session_id) {
            if self.order.len() >= MAX_SESSIONS
                && let Some(old_id) = self.order.pop_front()
            {
                self.map.remove(&old_id);
            }
            self.order.push_back(session_id.clone());
        }
        self.map.insert(session_id, history);
    }

    fn remove(&mut self, session_id: &str) {
        self.map.remove(session_id);
        self.order.retain(|id| id != session_id);
    }

    fn clear_all(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// Returns session IDs in most-recent-first order.
    fn list_ids(&self) -> Vec<String> {
        self.order.iter().rev().cloned().collect()
    }
}

// ─── Agent context ─────────────────────────────────────────────────────────────

/// Shared context for the local AI agent.
#[derive(Clone, Debug)]
pub struct AgentContext {
    pub settings: Arc<RwLock<AppSettings>>,
    pub autonomous_enabled: Arc<RwLock<bool>>,
    /// Channel used by server functions to notify the autonomous loop of setting changes.
    pub cmds_tx: broadcast::Sender<AgentCmd>,
    chat_sessions: Arc<RwLock<ChatSessions>>,
}

#[derive(Clone, Debug)]
pub enum AgentCmd {
    AutonomousModeToggled(bool),
    SettingsChanged(Box<AppSettings>),
}

impl AgentContext {
    pub fn new(settings: AppSettings) -> Self {
        let autonomous_enabled = settings.autonomous_enabled;
        let (cmds_tx, _) = broadcast::channel(16);
        Self {
            settings: Arc::new(RwLock::new(settings)),
            autonomous_enabled: Arc::new(RwLock::new(autonomous_enabled)),
            cmds_tx,
            chat_sessions: Arc::new(RwLock::new(ChatSessions::default())),
        }
    }

    pub async fn get_chat_history(&self, session_id: &str) -> Vec<ChatMessage> {
        self.chat_sessions.read().await.get(session_id)
    }

    pub async fn list_chat_sessions(&self) -> Vec<String> {
        self.chat_sessions.read().await.list_ids()
    }

    pub async fn clear_chat_history(&self, session_id: Option<&str>) {
        let mut sessions = self.chat_sessions.write().await;
        if let Some(sid) = session_id {
            sessions.remove(sid);
        } else {
            sessions.clear_all();
        }
    }
}

// ─── System prompt ────────────────────────────────────────────────────────────

fn build_system_prompt(custom_suffix: &str) -> String {
    let mut prompt = "You are the Formicaio AI Agent, an expert assistant for managing \
        nodes on the Autonomi p2p network. You help node operators monitor performance, \
        diagnose issues, and take management actions.\n\n\
        You have access to tools that connect directly to the live Formicaio instance:\n\
        - fetch_stats: aggregated statistics for all nodes\n\
        - nodes_instances: full list of all node instances with their current state\n\
        - start_node_instance / stop_node_instance / recycle_node_instance / upgrade_node_instance / delete_node_instance: manage individual nodes by ID\n\
        - create_node_instance: create a new node\n\n\
        CRITICAL RULES — you MUST follow these without exception:\n\
        1. NEVER invent, guess, or fabricate node IDs, node names, counts, balances, or any other data. \
           All node information MUST come from a tool call. If you have not called a tool yet, call it now.\n\
        2. Before answering ANY question about nodes or statistics, call the relevant tool first. \
           Do not rely on prior conversation history for live data — it may be stale.\n\
        3. Report tool results accurately. Do not add, remove, or modify nodes that were not in the tool response.\n\
        4. If a tool returns an error, tell the user exactly what the error was. Do not guess the answer.\n\
        5. An empty tool result (zero nodes, empty list, node_count 0) is VALID DATA. \
           It means Formicaio genuinely has no nodes. Tell the user there are no nodes. \
           NEVER invent nodes to fill an empty result.\n\
        6. Explain your reasoning before taking any destructive or irreversible action (delete, recycle).\n\
        7. Be concise and factual. Format numbers clearly. Do not pad responses with speculative commentary.\n\
        8. When asked to PERFORM AN ACTION (start, stop, restart, recycle, upgrade, delete nodes), \
           you MUST actually call the tool — do NOT just describe what you would do or explain the steps. \
           Call the tool NOW.\n\
        9. NEVER use placeholder text as a tool argument. Every argument passed to a tool MUST be a \
           real value obtained from a previous tool result in this conversation. For example, never \
           pass '[node_id]', '[ID of stopped node]', 'node-123', or any invented/guessed ID. \
           If you do not yet have the real node IDs, call nodes_instances first to obtain them.\n\
        10. For BATCH OPERATIONS (e.g. 'restart all stopped nodes', 'start all inactive nodes'): \
           Step 1 — call nodes_instances (no other tool calls yet). \
           Step 2 — read the result and identify each node that matches. \
           Step 3 — call the action tool (e.g. start_node_instance) ONCE per matching node, \
           using the exact node ID string from the nodes_instances result. \
           Make ONE action tool call per turn, then wait for its result, then proceed to the next node. \
           Do NOT batch or combine action calls — call them one at a time."
        .to_string();

    if !custom_suffix.is_empty() {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(custom_suffix);
    }
    prompt
}

// ─── Chat turn processor ──────────────────────────────────────────────────────

/// Process a single chat turn: stream tokens + tool calls to `tx`.
/// Loads prior conversation history from the in-memory session store by `session_id`,
/// and saves the updated history (user + assistant turn) back on completion.
pub async fn process_chat_turn(
    user_message: String,
    session_id: String,
    app_ctx: AppContext,
    node_manager: NodeManager,
    tx: tokio::sync::mpsc::Sender<Bytes>,
) {
    let settings = app_ctx.agent_ctx.settings.read().await.clone();

    // Load stored history for this session
    let history = app_ctx.agent_ctx.get_chat_history(&session_id).await;

    // Build messages for the LLM: system prompt + prior history (capped) + new user message
    let mut messages: Vec<LlmMessage> = vec![LlmMessage {
        role: "system".to_string(),
        content: build_system_prompt(&settings.system_prompt),
        tool_calls: None,
        tool_call_id: None,
    }];

    let max_ctx = settings.max_context_messages as usize;
    let history_window = if history.len() > max_ctx {
        &history[history.len() - max_ctx..]
    } else {
        &history
    };
    for msg in history_window {
        messages.push(LlmMessage {
            role: msg.role.as_str().to_string(),
            content: msg.content.clone(),
            tool_calls: None,
            tool_call_id: None,
        });
    }
    messages.push(LlmMessage::user(&user_message));

    let llm = OpenAiCompatClient::new(
        &settings.llm_base_url,
        &settings.llm_model,
        &settings.llm_api_key,
    );
    let tool_defs = ToolExecutor::tool_definitions();
    let executor = ToolExecutor::new(app_ctx.clone(), node_manager.clone());

    // Agentic loop: keep going until the LLM stops requesting tool calls
    let mut full_assistant_content = String::new();
    let mut all_tool_calls_display: Vec<(String, String, String)> = vec![];
    let mut pending_tool_calls: Vec<(usize, String, String, String)> = vec![]; // (idx, id, name, args)

    loop {
        let stream = match llm.chat_stream(messages.clone(), &tool_defs).await {
            Ok(s) => s,
            Err(e) => {
                send_chunk(
                    &tx,
                    &StreamChunk::Error {
                        message: e.to_string(),
                    },
                )
                .await;
                return;
            }
        };

        let mut current_text = String::new();
        pending_tool_calls.clear();

        futures_util::pin_mut!(stream);

        while let Some(event) = stream.next().await {
            match event {
                Err(e) => {
                    send_chunk(
                        &tx,
                        &StreamChunk::Error {
                            message: e.to_string(),
                        },
                    )
                    .await;
                    return;
                }
                Ok(StreamEvent::TextDelta(token)) => {
                    current_text.push_str(&token);
                    full_assistant_content.push_str(&token);
                    send_chunk(&tx, &StreamChunk::Token { content: token }).await;
                }
                Ok(StreamEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments_delta,
                }) => {
                    // Accumulate tool call parts by index
                    if let Some(entry) = pending_tool_calls
                        .iter_mut()
                        .find(|(i, _, _, _)| *i == index)
                    {
                        entry.3.push_str(&arguments_delta);
                        if let Some(n) = name {
                            entry.2 = n;
                        }
                    } else {
                        pending_tool_calls.push((
                            index,
                            id.unwrap_or_default(),
                            name.unwrap_or_default(),
                            arguments_delta,
                        ));
                    }
                }
                Ok(StreamEvent::Done) => {
                    break;
                }
            }
        }

        if pending_tool_calls.is_empty() {
            // No tool calls — the turn is complete
            break;
        }

        // There are tool calls: build assistant message with tool calls for context, then execute
        let tool_calls_json = serde_json::to_string(
            &pending_tool_calls
                .iter()
                .map(|(_, id, name, args)| {
                    json!({
                        "id": id,
                        "type": "function",
                        "function": {"name": name, "arguments": args}
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();

        // Append assistant turn to context for next iteration
        messages.push(LlmMessage {
            role: "assistant".to_string(),
            content: current_text.clone(),
            tool_calls: serde_json::from_str(&tool_calls_json).ok(),
            tool_call_id: None,
        });

        // Execute each tool call and inject results
        for (_, id, name, args_str) in &pending_tool_calls {
            let fake_call = llm_client::LlmToolCall {
                id: id.clone(),
                r#type: "function".to_string(),
                function: llm_client::LlmFunctionCall {
                    name: name.clone(),
                    arguments: args_str.clone(),
                },
            };

            send_chunk(
                &tx,
                &StreamChunk::ToolStart {
                    name: name.clone(),
                    input: args_str.clone(),
                },
            )
            .await;

            let result = executor.execute(&fake_call).await;

            send_chunk(
                &tx,
                &StreamChunk::ToolResult {
                    name: name.clone(),
                    result: result.clone(),
                },
            )
            .await;

            // Record for display in reloaded sessions
            all_tool_calls_display.push((name.clone(), args_str.clone(), result.clone()));

            // Append tool result to the conversation context
            messages.push(LlmMessage::tool_result(id, result));
        }

        // Loop back to get the LLM's next response given the tool results
    }

    // Persist the completed turn (user + assistant) to the in-memory session store
    if !full_assistant_content.is_empty() || !all_tool_calls_display.is_empty() {
        let mut updated_history = history;
        updated_history.push(ChatMessage::user(user_message));
        updated_history.push(ChatMessage::assistant(
            full_assistant_content,
            all_tool_calls_display,
        ));
        // Trim to the cap so a single session can't grow unboundedly
        if updated_history.len() > MAX_MESSAGES_PER_SESSION {
            let excess = updated_history.len() - MAX_MESSAGES_PER_SESSION;
            updated_history.drain(..excess);
        }
        app_ctx
            .agent_ctx
            .chat_sessions
            .write()
            .await
            .upsert(session_id, updated_history);
    }

    send_chunk(&tx, &StreamChunk::Done).await;
}

// ─── Autonomous monitoring loop ───────────────────────────────────────────────

pub async fn run_autonomous_loop(app_ctx: AppContext, node_manager: NodeManager) {
    let settings = app_ctx.agent_ctx.settings.read().await.clone();
    let mut check_interval = tokio::time::interval(tokio::time::Duration::from_secs(
        settings.autonomous_check_interval_secs,
    ));
    let mut cmds_rx = app_ctx.agent_ctx.cmds_tx.subscribe();

    logging::log!("[Agent] Autonomous monitoring loop started.");

    loop {
        tokio::select! {
            _ = check_interval.tick() => {
                let enabled = *app_ctx.agent_ctx.autonomous_enabled.read().await;
                if enabled {
                    run_monitoring_cycle(&app_ctx, &node_manager).await;
                }
            }
            cmd = cmds_rx.recv() => {
                match cmd {
                    Ok(AgentCmd::AutonomousModeToggled(enabled)) => {
                        *app_ctx.agent_ctx.autonomous_enabled.write().await = enabled;
                        logging::log!("[Agent] Autonomous mode toggled to: {enabled}");
                    }
                    Ok(AgentCmd::SettingsChanged(new_settings)) => {
                        let new_interval = tokio::time::Duration::from_secs(
                            new_settings.autonomous_check_interval_secs,
                        );
                        *app_ctx.agent_ctx.settings.write().await = *new_settings;
                        check_interval = tokio::time::interval(new_interval);
                        logging::log!("[Agent] Settings updated, interval changed to {new_interval:?}");
                    }
                    Err(_) => { /* sender dropped — continue */ }
                }
            }
        }
    }
}

async fn run_monitoring_cycle(app_ctx: &AppContext, node_manager: &NodeManager) {
    let settings = app_ctx.agent_ctx.settings.read().await.clone();
    logging::log!("[Agent] Running autonomous monitoring cycle...");

    let llm = OpenAiCompatClient::new(
        &settings.llm_base_url,
        &settings.llm_model,
        &settings.llm_api_key,
    );
    // Restrict to read-only + start_node_instance only.
    // Destructive tools (delete, recycle, upgrade, create) are intentionally excluded.
    let tool_defs = ToolExecutor::autonomous_tool_definitions();
    let executor = ToolExecutor::new(app_ctx.clone(), node_manager.clone());

    let system = "You are a Formicaio autonomous monitoring agent. \
        Analyse the current node states and take corrective action ONLY when necessary. \
        Restart nodes that are offline/inactive. \
        Do NOT take action if all nodes are healthy. \
        For each action taken, be minimal and targeted. \
        Respond with a brief plain-text summary after any tool calls."
        .to_string();

    let messages = vec![
        LlmMessage {
            role: "system".to_string(),
            content: system,
            tool_calls: None,
            tool_call_id: None,
        },
        LlmMessage::user(
            "Please check the health of all nodes and take corrective action only if needed.",
        ),
    ];

    let max_actions = settings.autonomous_max_actions_per_cycle as usize;
    let mut actions_taken: usize = 0;
    let mut current_messages = messages;

    loop {
        if actions_taken >= max_actions {
            logging::log!("[Agent] Max actions per cycle ({max_actions}) reached.");
            let _ = app_ctx
                .db_client
                .insert_agent_event(
                    &AgentEventType::Info,
                    &format!("Monitoring cycle: max actions ({max_actions}) reached"),
                )
                .await;
            break;
        }

        let stream = match llm.chat_stream(current_messages.clone(), &tool_defs).await {
            Ok(s) => s,
            Err(e) => {
                logging::error!("[Agent] Autonomous cycle LLM error: {e}");
                let _ = app_ctx
                    .db_client
                    .insert_agent_event(
                        &AgentEventType::Error,
                        &format!("LLM error during monitoring: {e}"),
                    )
                    .await;
                return;
            }
        };

        let mut text = String::new();
        // Store (index, id, name, args) — index matches the LLM's tool-call index,
        // not the vector position, so out-of-order deltas are accumulated correctly.
        let mut pending: Vec<(usize, String, String, String)> = vec![];

        futures_util::pin_mut!(stream);

        while let Some(event) = stream.next().await {
            match event {
                Ok(StreamEvent::TextDelta(t)) => text.push_str(&t),
                Ok(StreamEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments_delta,
                }) => {
                    if let Some(entry) = pending.iter_mut().find(|(i, _, _, _)| *i == index) {
                        entry.3.push_str(&arguments_delta);
                        if let Some(n) = name {
                            entry.2 = n;
                        }
                    } else {
                        pending.push((
                            index,
                            id.unwrap_or_default(),
                            name.unwrap_or_default(),
                            arguments_delta,
                        ));
                    }
                }
                Ok(StreamEvent::Done) => break,
                Err(e) => {
                    logging::error!("[Agent] Stream error in autonomous cycle: {e}");
                    break;
                }
            }
        }

        if !text.is_empty() {
            logging::log!("[Agent] Autonomous summary: {text}");
            let _ = app_ctx
                .db_client
                .insert_agent_event(&AgentEventType::Info, &text)
                .await;
        }

        if pending.is_empty() {
            break;
        }

        // Build assistant message with tool calls
        let tool_calls_value: Vec<serde_json::Value> = pending
            .iter()
            .map(|(_, id, name, args)| {
                json!({
                    "id": id, "type": "function",
                    "function": {"name": name, "arguments": args}
                })
            })
            .collect();

        current_messages.push(LlmMessage {
            role: "assistant".to_string(),
            content: text,
            tool_calls: serde_json::from_value(json!(tool_calls_value)).ok(),
            tool_call_id: None,
        });

        for (_, id, name, args) in &pending {
            let fake_call = llm_client::LlmToolCall {
                id: id.clone(),
                r#type: "function".to_string(),
                function: llm_client::LlmFunctionCall {
                    name: name.clone(),
                    arguments: args.clone(),
                },
            };
            let result = executor.execute(&fake_call).await;

            logging::log!("[Agent] Autonomous action '{name}': {result}");
            let _ = app_ctx
                .db_client
                .insert_agent_event(
                    &AgentEventType::ActionTaken,
                    &format!("Called {name}: {result}"),
                )
                .await;

            current_messages.push(LlmMessage::tool_result(id, &result));
            actions_taken += 1;
        }
    }

    logging::log!("[Agent] Monitoring cycle complete. Actions taken: {actions_taken}");
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn send_chunk(tx: &tokio::sync::mpsc::Sender<Bytes>, chunk: &StreamChunk) {
    if let Ok(mut json) = serde_json::to_string(chunk) {
        json.push('\n');
        let _ = tx.send(Bytes::from(json)).await;
    }
}
