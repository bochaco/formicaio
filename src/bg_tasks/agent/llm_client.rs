use async_trait::async_trait;
use futures_util::StreamExt;
use leptos::logging;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// ─── Wire types for the OpenAI-compatible /v1/chat/completions API ────────────

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl LlmMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LlmToolCall {
    pub id: String,
    pub r#type: String,
    pub function: LlmFunctionCall,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LlmFunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Description of a tool that the LLM may call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: FunctionDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A parsed chunk from the streaming chat completions response.
#[derive(Debug)]
pub enum StreamEvent {
    /// A token fragment of the assistant's text reply.
    TextDelta(String),
    /// The assistant wants to call a tool.
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    },
    /// The stream is finished.
    Done,
}

// ─── LLM client abstraction ───────────────────────────────────────────────────

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Stream a chat completion. Returns a stream of `StreamEvent`.
    async fn chat_stream(
        &self,
        messages: Vec<LlmMessage>,
        tools: &[ToolDefinition],
    ) -> Result<
        std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<StreamEvent, LlmError>> + Send>>,
        LlmError,
    >;

    /// Return available model names (calls GET /v1/models).
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("LLM API error: {0}")]
    Api(String),
    #[error("Stream ended unexpectedly")]
    StreamEnded,
}

// ─── OpenAI-compatible implementation ────────────────────────────────────────

pub struct OpenAiCompatClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl OpenAiCompatClient {
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        // Normalise base URL: strip trailing slash, and for Ollama (no /v1) append /v1
        let mut base = base_url.trim_end_matches('/').to_string();
        if !base.ends_with("/v1") {
            base.push_str("/v1");
        }
        Self {
            client: reqwest::Client::new(),
            base_url: base,
            model: model.to_string(),
            api_key: if api_key.is_empty() {
                None
            } else {
                Some(api_key.to_string())
            },
        }
    }

    fn build_request(&self, body: Value) -> reqwest::RequestBuilder {
        let url = format!("{}/chat/completions", self.base_url);
        let mut req = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        req
    }
}

#[async_trait]
impl LlmClient for OpenAiCompatClient {
    async fn chat_stream(
        &self,
        messages: Vec<LlmMessage>,
        tools: &[ToolDefinition],
    ) -> Result<
        std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<StreamEvent, LlmError>> + Send>>,
        LlmError,
    > {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }

        let response = self.build_request(body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            // Extract the human-readable message from the backend's JSON error body.
            // Ollama: {"error": "string"}
            // OpenAI: {"error": {"message": "string", ...}}
            let msg = serde_json::from_str::<Value>(&body_text)
                .ok()
                .and_then(|v| {
                    v["error"]
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v["error"]["message"].as_str().map(|s| s.to_string()))
                })
                .unwrap_or(body_text);
            return Err(LlmError::Api(format!("{status}: {msg}")));
        }

        // Parse SSE frames robustly across network chunk boundaries.
        let stream = response
            .bytes_stream()
            .scan(Vec::<u8>::new(), |pending, chunk_result| {
                let events = match chunk_result {
                    Err(e) => vec![Err(LlmError::Http(e))],
                    Ok(bytes) => {
                        pending.extend_from_slice(&bytes);
                        parse_sse_frames(pending)
                    }
                };
                futures_util::future::ready(Some(events))
            })
            .flat_map(futures_util::stream::iter)
            .boxed();

        Ok(stream)
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/models", self.base_url);
        let mut req = self.client.get(&url);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp: Value = req.send().await?.error_for_status()?.json().await?;
        let models = resp["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }
}

/// Parse one or more SSE data lines from a raw bytes chunk.
/// Parses all complete SSE event frames from `pending`, leaving incomplete trailing
/// bytes in the buffer for the next network chunk.
fn parse_sse_frames(pending: &mut Vec<u8>) -> Vec<Result<StreamEvent, LlmError>> {
    let mut all_events = vec![];

    while let Some((frame_end, delim_len)) = find_sse_frame_end(pending) {
        let frame = pending[..frame_end].to_vec();
        pending.drain(..frame_end + delim_len);

        let text = match std::str::from_utf8(&frame) {
            Ok(t) => t,
            Err(e) => {
                logging::warn!("[Agent] Failed to decode SSE frame as UTF-8: {e}");
                continue;
            }
        };

        all_events.extend(parse_sse_frame_text(text));
    }

    all_events
}

/// Find the end of a complete SSE frame in `pending`.
/// Returns (frame_end_index, delimiter_len), where delimiter is either "\n\n" or "\r\n\r\n".
fn find_sse_frame_end(pending: &[u8]) -> Option<(usize, usize)> {
    if pending.len() < 2 {
        return None;
    }

    for i in 0..(pending.len() - 1) {
        if pending[i] == b'\n' && pending[i + 1] == b'\n' {
            return Some((i, 2));
        }
        if i + 3 < pending.len()
            && pending[i] == b'\r'
            && pending[i + 1] == b'\n'
            && pending[i + 2] == b'\r'
            && pending[i + 3] == b'\n'
        {
            return Some((i, 4));
        }
    }

    None
}

/// Parse one SSE frame text (possibly containing multiple data lines).
fn parse_sse_frame_text(text: &str) -> Vec<Result<StreamEvent, LlmError>> {
    let mut events = vec![];

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        let data = if let Some(d) = line.strip_prefix("data:") {
            d.trim_start()
        } else {
            continue;
        };

        if data == "[DONE]" {
            events.push(Ok(StreamEvent::Done));
            continue;
        }

        match serde_json::from_str::<Value>(data) {
            Err(e) => {
                logging::warn!("[Agent] Failed to parse SSE JSON: {e} — data: {data}");
            }
            Ok(json) => {
                let choices = match json["choices"].as_array() {
                    Some(c) => c,
                    None => continue,
                };

                for choice in choices {
                    let delta = &choice["delta"];

                    // Text content token
                    if let Some(content) = delta["content"].as_str()
                        && !content.is_empty()
                    {
                        events.push(Ok(StreamEvent::TextDelta(content.to_string())));
                    }

                    // Tool call deltas
                    if let Some(tool_calls) = delta["tool_calls"].as_array() {
                        for tc in tool_calls {
                            let index = tc["index"].as_u64().unwrap_or(0) as usize;
                            let id = tc["id"].as_str().map(|s| s.to_string());
                            let name = tc["function"]["name"].as_str().map(|s| s.to_string());
                            let args_delta = tc["function"]["arguments"]
                                .as_str()
                                .unwrap_or("")
                                .to_string();
                            events.push(Ok(StreamEvent::ToolCallDelta {
                                index,
                                id,
                                name,
                                arguments_delta: args_delta,
                            }));
                        }
                    }

                    // Finish reason signals end
                    if let Some(reason) = choice["finish_reason"].as_str()
                        && (reason == "stop" || reason == "tool_calls")
                    {
                        events.push(Ok(StreamEvent::Done));
                    }
                }
            }
        }
    }
    events
}
