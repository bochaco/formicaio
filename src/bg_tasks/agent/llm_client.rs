use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestUserMessage, ChatCompletionResponseStream, ChatCompletionTool,
        ChatCompletionToolChoiceOption, ChatCompletionTools, CreateChatCompletionRequest,
        FinishReason, FunctionCall, FunctionObject, ToolChoiceOptions,
    },
};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Conversation history types ───────────────────────────────────────────────

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

/// A parsed event from the streaming chat completions response.
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
    #[error("OpenAI API error: {0}")]
    OpenAI(#[from] async_openai::error::OpenAIError),
    #[error("LLM API error: {0}")]
    Api(String),
}

// ─── OpenAI-compatible implementation ────────────────────────────────────────

pub struct OpenAiCompatClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiCompatClient {
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        // Normalise base URL: strip trailing slash, and for Ollama (no /v1) append /v1
        let mut base = base_url.trim_end_matches('/').to_string();
        if !base.ends_with("/v1") {
            base.push_str("/v1");
        }
        let mut config = OpenAIConfig::new().with_api_base(base);
        if !api_key.is_empty() {
            config = config.with_api_key(api_key);
        }
        Self {
            client: Client::with_config(config),
            model: model.to_string(),
        }
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
        let openai_messages: Vec<ChatCompletionRequestMessage> =
            messages.iter().map(to_openai_message).collect();

        let request = CreateChatCompletionRequest {
            model: self.model.clone(),
            messages: openai_messages,
            stream: Some(true),
            tools: if tools.is_empty() {
                None
            } else {
                Some(
                    tools
                        .iter()
                        .map(|td| ChatCompletionTools::Function(to_openai_tool(td)))
                        .collect(),
                )
            },
            tool_choice: if tools.is_empty() {
                None
            } else {
                Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Auto,
                ))
            },
            ..Default::default()
        };

        let openai_stream: ChatCompletionResponseStream =
            self.client.chat().create_stream(request).await?;

        let stream = openai_stream
            .flat_map(|result| {
                let events: Vec<Result<StreamEvent, LlmError>> = match result {
                    Err(e) => vec![Err(LlmError::OpenAI(e))],
                    Ok(response) => {
                        let mut events = vec![];
                        for choice in response.choices {
                            let delta = &choice.delta;

                            if let Some(content) = &delta.content
                                && !content.is_empty()
                            {
                                events.push(Ok(StreamEvent::TextDelta(content.clone())));
                            }

                            if let Some(tool_calls) = &delta.tool_calls {
                                for tc in tool_calls {
                                    let args_delta = tc
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.arguments.as_deref())
                                        .unwrap_or("")
                                        .to_string();
                                    events.push(Ok(StreamEvent::ToolCallDelta {
                                        index: tc.index as usize,
                                        id: tc.id.clone(),
                                        name: tc.function.as_ref().and_then(|f| f.name.clone()),
                                        arguments_delta: args_delta,
                                    }));
                                }
                            }

                            if let Some(FinishReason::Stop | FinishReason::ToolCalls) =
                                &choice.finish_reason
                            {
                                events.push(Ok(StreamEvent::Done));
                            }
                        }
                        events
                    }
                };
                futures_util::stream::iter(events)
            })
            .boxed();

        Ok(stream)
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let response = self.client.models().list().await?;
        Ok(response.data.iter().map(|m| m.id.clone()).collect())
    }
}

// ─── Type conversions ─────────────────────────────────────────────────────────

fn to_openai_message(msg: &LlmMessage) -> ChatCompletionRequestMessage {
    match msg.role.as_str() {
        "system" => ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
            content: msg.content.clone().into(),
            name: None,
        }),
        "user" => ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: msg.content.clone().into(),
            name: None,
        }),
        "assistant" => {
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: if msg.content.is_empty() {
                    None
                } else {
                    Some(msg.content.clone().into())
                },
                tool_calls: msg.tool_calls.as_ref().map(|tcs| {
                    tcs.iter()
                        .map(|tc| {
                            ChatCompletionMessageToolCalls::Function(
                                ChatCompletionMessageToolCall {
                                    id: tc.id.clone(),
                                    function: FunctionCall {
                                        name: tc.function.name.clone(),
                                        arguments: tc.function.arguments.clone(),
                                    },
                                },
                            )
                        })
                        .collect()
                }),
                ..Default::default()
            })
        }
        "tool" => ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
            content: msg.content.clone().into(),
            tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
        }),
        role => {
            leptos::logging::warn!("[Agent] Unknown LLM message role '{role}', treating as user");
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: msg.content.clone().into(),
                name: None,
            })
        }
    }
}

fn to_openai_tool(td: &ToolDefinition) -> ChatCompletionTool {
    ChatCompletionTool {
        function: FunctionObject {
            name: td.function.name.clone(),
            description: Some(td.function.description.clone()),
            parameters: Some(td.function.parameters.clone()),
            strict: None,
        },
    }
}
