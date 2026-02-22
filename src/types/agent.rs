use serde::{Deserialize, Serialize};

// ─── Chat message types ────────────────────────────────────────────────────────

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum ChatRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

impl ChatRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }
}

/// A chat message kept in the server-side in-memory session store.
/// `tool_calls_display` captures every tool call made during an assistant turn
/// as `(name, input, result)` triples so they can be rendered in the UI when
/// a previous session is reloaded.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default)]
    pub tool_calls_display: Vec<(String, String, String)>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            tool_calls_display: vec![],
        }
    }

    pub fn assistant(
        content: impl Into<String>,
        tool_calls_display: Vec<(String, String, String)>,
    ) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_calls_display,
        }
    }
}

// ─── Autonomous event types ────────────────────────────────────────────────────

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum AgentEventType {
    #[serde(rename = "action_taken")]
    ActionTaken,
    #[serde(rename = "anomaly_detected")]
    AnomalyDetected,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "error")]
    Error,
}

impl AgentEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentEventType::ActionTaken => "action_taken",
            AgentEventType::AnomalyDetected => "anomaly_detected",
            AgentEventType::Info => "info",
            AgentEventType::Error => "error",
        }
    }
}

impl std::str::FromStr for AgentEventType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "action_taken" => Ok(AgentEventType::ActionTaken),
            "anomaly_detected" => Ok(AgentEventType::AnomalyDetected),
            "info" => Ok(AgentEventType::Info),
            "error" => Ok(AgentEventType::Error),
            other => Err(format!("Unknown agent event type: {other}")),
        }
    }
}

/// A record of something the autonomous agent observed or did.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AgentEvent {
    pub id: i64,
    pub event_type: AgentEventType,
    pub description: String,
    pub timestamp: i64,
}

// ─── LLM wire types (shared between SSR and WASM via Serialize/Deserialize) ───

/// A chunk delivered from the streaming chat endpoint.
/// The frontend deserialises each newline-delimited JSON object.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum StreamChunk {
    /// A piece of the assistant's text response.
    #[serde(rename = "token")]
    Token { content: String },
    /// The agent is about to call a tool.
    #[serde(rename = "tool_start")]
    ToolStart { name: String, input: String },
    /// The tool has returned a result.
    #[serde(rename = "tool_result")]
    ToolResult { name: String, result: String },
    /// The full turn is complete.
    #[serde(rename = "done")]
    Done,
    /// An error occurred during the turn.
    #[serde(rename = "error")]
    Error { message: String },
}
