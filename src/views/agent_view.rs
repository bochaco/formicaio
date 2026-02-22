use super::{
    ClientGlobalState,
    helpers::show_error_alert_msg,
    icons::{IconBot, IconCancel, IconPrompt},
};
use crate::{
    server_api::{
        clear_chat_history, get_agent_events, get_chat_history, get_new_agent_events,
        list_chat_sessions, send_chat_message, toggle_autonomous_mode,
    },
    types::{AgentEvent, AgentEventType, ChatMessage, ChatRole, StreamChunk},
};

use chrono::{DateTime, Local, Utc};
use gloo_timers::future::TimeoutFuture;
use leptos::{html, logging, prelude::*, task::spawn_local};

// ─── Session ID generation ────────────────────────────────────────────────────

fn new_session_id() -> String {
    use rand::Rng;
    let id: u64 = rand::rng().random();
    format!("s{:x}", id)
}

// ─── UI message type ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
struct UiMessage {
    role: ChatRole,
    content: String,
    tool_calls_text: Vec<(String, String, String)>, // (name, input, result)
    is_streaming: bool,
}

impl UiMessage {
    fn from_stored(msg: &ChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
            tool_calls_text: msg.tool_calls_display.clone(),
            is_streaming: false,
        }
    }
}

// ─── Main AgentView component ─────────────────────────────────────────────────

#[component]
pub fn AgentView() -> impl IntoView {
    let session_id = RwSignal::new(new_session_id());
    let messages: RwSignal<Vec<UiMessage>> = RwSignal::new(vec![]);
    let streaming_msg: RwSignal<Option<UiMessage>> = RwSignal::new(None);
    let is_streaming = RwSignal::new(false);
    let input_text = RwSignal::new(String::new());
    let scroll_ref: NodeRef<html::Div> = NodeRef::new();

    // Sessions panel state
    let sessions: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let show_sessions = RwSignal::new(false);

    // Autonomous mode state
    let context = expect_context::<ClientGlobalState>();
    let autonomous_enabled = move || context.app_settings.read().autonomous_enabled;

    // Agent events (shown when autonomous mode is active)
    let agent_events: RwSignal<Vec<AgentEvent>> = RwSignal::new(vec![]);
    // Tracks the timestamp of the most recent event seen, used for polling
    let last_event_ts = RwSignal::new(0i64);

    // Auto-scroll whenever messages or streaming content change
    Effect::new(move |_| {
        let _ = messages.read();
        let _ = streaming_msg.read();
        if let Some(div) = scroll_ref.get() {
            div.set_scroll_top(div.scroll_height());
        }
    });

    // Load sessions list on mount
    spawn_local(async move {
        match list_chat_sessions().await {
            Ok(s) => sessions.set(s),
            Err(e) => logging::warn!("Failed to load sessions: {e}"),
        }
    });

    // ─── Send action ──────────────────────────────────────────────────────────

    let send_msg = move || {
        let text = input_text.get_untracked().trim().to_string();
        if text.is_empty() || is_streaming.get_untracked() {
            return;
        }
        input_text.set(String::new());
        is_streaming.set(true);

        // Optimistically add user message
        messages.update(|m| {
            m.push(UiMessage {
                role: ChatRole::User,
                content: text.clone(),
                tool_calls_text: vec![],
                is_streaming: false,
            })
        });

        // Start the streaming assistant message placeholder
        streaming_msg.set(Some(UiMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            tool_calls_text: vec![],
            is_streaming: true,
        }));

        let sid = session_id.get_untracked();

        spawn_local(async move {
            use futures_util::StreamExt;

            match send_chat_message(sid.clone(), text).await {
                Err(e) => {
                    show_error_alert_msg(format!("Agent error: {e}"));
                    streaming_msg.set(None);
                    is_streaming.set(false);
                }
                Ok(stream) => {
                    let mut inner = stream.into_inner();
                    let mut cur_bytes = Vec::<u8>::new();

                    while let Some(item) = inner.next().await {
                        match item {
                            Err(e) => {
                                logging::error!("[Agent] Stream error: {e}");
                                break;
                            }
                            Ok(bytes) => {
                                cur_bytes.extend_from_slice(&bytes);
                                // Process complete newline-delimited JSON lines
                                while let Some(nl_pos) = cur_bytes.iter().position(|&b| b == b'\n')
                                {
                                    let line_bytes = cur_bytes.drain(..=nl_pos).collect::<Vec<_>>();
                                    let line = String::from_utf8_lossy(&line_bytes);
                                    let line = line.trim();
                                    if line.is_empty() {
                                        continue;
                                    }
                                    match serde_json::from_str::<StreamChunk>(line) {
                                        Err(e) => {
                                            logging::warn!(
                                                "[Agent] Failed to parse chunk: {e} — {line}"
                                            );
                                        }
                                        Ok(StreamChunk::Token { content }) => {
                                            streaming_msg.update(|opt| {
                                                if let Some(m) = opt {
                                                    m.content.push_str(&content);
                                                }
                                            });
                                        }
                                        Ok(StreamChunk::ToolStart { name, input }) => {
                                            streaming_msg.update(|opt| {
                                                if let Some(m) = opt {
                                                    m.tool_calls_text.push((
                                                        name,
                                                        input,
                                                        String::new(),
                                                    ));
                                                }
                                            });
                                        }
                                        Ok(StreamChunk::ToolResult { name, result }) => {
                                            streaming_msg.update(|opt| {
                                                if let Some(m) = opt
                                                    && let Some(tc) = m
                                                        .tool_calls_text
                                                        .iter_mut()
                                                        .find(|(n, _, _)| *n == name)
                                                {
                                                    tc.2 = result;
                                                }
                                            });
                                        }
                                        Ok(StreamChunk::Done) => {
                                            // Move the completed streaming message into the permanent list
                                            if let Some(completed) = streaming_msg.get_untracked() {
                                                let mut fin = completed;
                                                fin.is_streaming = false;
                                                messages.update(|m| m.push(fin));
                                            }
                                            streaming_msg.set(None);
                                            is_streaming.set(false);

                                            // Refresh sessions list
                                            if let Ok(s) = list_chat_sessions().await {
                                                sessions.set(s);
                                            }
                                        }
                                        Ok(StreamChunk::Error { message }) => {
                                            show_error_alert_msg(format!("Agent: {message}"));
                                            streaming_msg.set(None);
                                            is_streaming.set(false);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Ensure streaming state is cleared even if stream ended without Done
                    if is_streaming.get_untracked() {
                        if let Some(completed) = streaming_msg.get_untracked() {
                            let mut fin = completed;
                            fin.is_streaming = false;
                            if !fin.content.is_empty() {
                                messages.update(|m| m.push(fin));
                            }
                        }
                        streaming_msg.set(None);
                        is_streaming.set(false);
                    }
                }
            }
        });
    };

    // ─── Load a previous session ──────────────────────────────────────────────

    let load_session = move |sid: String| {
        if is_streaming.get_untracked() {
            return;
        }
        let sid_clone = sid.clone();
        spawn_local(async move {
            match get_chat_history(sid_clone.clone()).await {
                Ok(history) => {
                    session_id.set(sid_clone);
                    messages.set(history.iter().map(UiMessage::from_stored).collect());
                    show_sessions.set(false);
                }
                Err(e) => show_error_alert_msg(format!("Failed to load session: {e}")),
            }
        });
    };

    // ─── Clear current session ────────────────────────────────────────────────

    let clear_session = move |_| {
        let sid = session_id.get_untracked();
        spawn_local(async move {
            let _ = clear_chat_history(Some(sid)).await;
            messages.set(vec![]);
            session_id.set(new_session_id());
            if let Ok(s) = list_chat_sessions().await {
                sessions.set(s);
            }
        });
    };

    // Start polling for new events while autonomous mode is active
    spawn_local(async move {
        // Fetch the most recent events on enable
        if let Ok(events) = get_agent_events(20).await {
            // events are DESC (newest first); track the newest timestamp
            if let Some(newest) = events.first() {
                last_event_ts.set(newest.timestamp);
            }
            agent_events.set(events);
        }

        loop {
            TimeoutFuture::new(30_000).await;
            if !context.app_settings.read_untracked().autonomous_enabled {
                continue;
            }

            let since = last_event_ts.get_untracked();
            match get_new_agent_events(since).await {
                Ok(new_events) if !new_events.is_empty() => {
                    // new_events are ASC (oldest first); prepend reversed
                    // so newest remains at the top of the list
                    let new_ts = new_events.last().map(|e| e.timestamp).unwrap_or(since);
                    let mut prepend: Vec<AgentEvent> = new_events.into_iter().rev().collect();
                    agent_events.update(|events| {
                        prepend.append(events);
                        *events = prepend;
                    });
                    last_event_ts.set(new_ts);
                }
                Err(e) => {
                    logging::warn!("Failed to poll agent events: {e}")
                }
                _ => {}
            }
        }
    });

    // ─── Toggle autonomous mode ────────────────────────────────────────────────
    let toggle_autonomous = Action::new(move |_input: &()| {
        let new_val = !context.app_settings.read_untracked().autonomous_enabled;
        async move {
            match toggle_autonomous_mode(new_val).await {
                Ok(()) => context
                    .app_settings
                    .update(|s| s.autonomous_enabled = new_val),
                Err(e) => show_error_alert_msg(format!("Failed to toggle autonomous mode: {e}")),
            }
        }
    });

    // ─── Quick actions ────────────────────────────────────────────────────────

    let quick_actions = [
        "Show node statistics",
        "List all nodes",
        "Restart all stopped or inactive nodes",
        "Which nodes have the most connected peers?",
        "Summarize my earnings",
    ];

    view! {
        <div class="flex h-full gap-0 animate-in fade-in duration-500">
            // ── Sessions sidebar ──
            <Show when=move || show_sessions.get()>
                <aside class="w-56 shrink-0 bg-slate-900 border-r border-slate-800 flex flex-col overflow-hidden">
                    <div class="p-4 border-b border-slate-800 flex items-center justify-between">
                        <span class="text-xs font-bold text-slate-400 uppercase tracking-wider">
                            Sessions
                        </span>
                        <button
                            on:click=move |_| show_sessions.set(false)
                            class="text-slate-500 hover:text-white transition-colors"
                        >
                            <IconCancel />
                        </button>
                    </div>
                    <div class="flex-1 overflow-y-auto p-2 space-y-1 no-scrollbar">
                        <button
                            on:click=move |_| {
                                messages.set(vec![]);
                                session_id.set(new_session_id());
                                show_sessions.set(false);
                            }
                            class="w-full text-left px-3 py-2 rounded-lg text-xs text-indigo-400 hover:bg-indigo-500/10 border border-dashed border-indigo-500/30 transition-colors"
                        >
                            "+ New session"
                        </button>
                        <For
                            each=move || sessions.read().clone().into_iter().enumerate()
                            key=|(i, _)| *i
                            let:child
                        >
                            <button
                                on:click={
                                    let sid = child.1.clone();
                                    move |_| load_session(sid.clone())
                                }
                                class="w-full text-left px-3 py-2 rounded-lg text-xs text-slate-300 hover:bg-slate-800 transition-colors truncate"
                            >
                                {format!("Session {}", child.0 + 1)}
                                <div class="text-slate-600 truncate font-mono">
                                    {child.1.clone()}
                                </div>
                            </button>
                        </For>
                    </div>
                </aside>
            </Show>

            // ── Main chat area ──
            <div class="flex-1 flex flex-col min-w-0 h-full">
                // Header
                <div class="shrink-0 px-4 py-3 border-b border-slate-800 bg-slate-900/50 flex items-center justify-between gap-3">
                    <div class="flex items-center gap-3">
                        <div class="w-9 h-9 bg-indigo-600 rounded-xl flex items-center justify-center shrink-0">
                            <IconBot class="w-5 h-5 text-white" />
                        </div>
                        <div>
                            <p class="text-xs text-slate-500">
                                "Local • Privacy-preserving • Offline-capable"
                            </p>
                        </div>
                    </div>
                    <div class="flex items-center gap-2">
                        // Autonomous mode toggle
                        <label class="flex items-center gap-2 cursor-pointer group">
                            <span class="text-xs text-slate-500 group-hover:text-slate-300 transition-colors hidden sm:inline">
                                Autonomous
                            </span>
                            <div class="relative">
                                <input
                                    type="checkbox"
                                    class="sr-only peer"
                                    prop:checked=autonomous_enabled
                                    on:change=move |_| {
                                        toggle_autonomous.dispatch(());
                                    }
                                />
                                <div class="w-8 h-4 bg-slate-700 rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-3 after:w-3 after:transition-all peer-checked:bg-indigo-600"></div>
                            </div>
                        </label>
                        <button
                            on:click=move |_| show_sessions.update(|v| *v = !*v)
                            class="text-xs text-slate-500 hover:text-slate-300 px-2 py-1 rounded-lg hover:bg-slate-800 transition-colors"
                        >
                            "History"
                        </button>
                        <button
                            on:click=clear_session
                            class="text-xs text-slate-500 hover:text-rose-400 px-2 py-1 rounded-lg hover:bg-slate-800 transition-colors"
                        >
                            "Clear"
                        </button>
                    </div>
                </div>

                // Messages list
                <div node_ref=scroll_ref class="flex-1 overflow-y-auto p-4 space-y-4 no-scrollbar">
                    // Welcome message when empty
                    <Show when=move || messages.read().is_empty() && streaming_msg.read().is_none()>
                        <div class="flex flex-col items-center justify-center h-full text-center space-y-6 py-12">
                            <div class="w-16 h-16 bg-indigo-600/20 border border-indigo-500/30 rounded-3xl flex items-center justify-center">
                                <IconBot class="w-8 h-8 text-indigo-400" />
                            </div>
                            <div>
                                <h3 class="text-lg font-bold text-slate-200">
                                    "How can I help you manage your nodes?"
                                </h3>
                                <p class="text-sm text-slate-500 mt-2 max-w-sm">
                                    "Ask anything about your Autonomi nodes — I can monitor, start, stop, and optimise them."
                                </p>
                            </div>
                            // Quick action chips
                            <div class="flex flex-wrap gap-2 justify-center max-w-lg">
                                {quick_actions
                                    .into_iter()
                                    .map(|action| {
                                        view! {
                                            <button
                                                on:click=move |_| {
                                                    input_text.set(action.to_string());
                                                    send_msg();
                                                }
                                                class="px-3 py-1.5 rounded-full bg-slate-800 hover:bg-slate-700 border border-slate-700 hover:border-indigo-500/50 text-xs text-slate-300 transition-all"
                                            >
                                                {action}
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                    </Show>

                    // Message history
                    <For
                        each=move || messages.read().clone().into_iter().enumerate()
                        key=|(i, _)| *i
                        let:child
                    >
                        <MessageBubble msg=child.1 />
                    </For>

                    // Streaming assistant message
                    <Show when=move || {
                        streaming_msg.read().is_some()
                    }>
                        {move || {
                            if let Some(msg) = streaming_msg.get() {
                                view! { <MessageBubble msg=msg /> }.into_any()
                            } else {
                                ().into_any()
                            }
                        }}
                    </Show>
                </div>

                // Agent events (shown when autonomous mode is on)
                <Show when=move || autonomous_enabled()>
                    <div class="shrink-0 border-t border-slate-800 bg-slate-900/30 p-3 max-h-32 overflow-y-auto no-scrollbar">
                        <div class="text-xs font-bold text-slate-500 uppercase tracking-wider mb-2">
                            "Autonomous Events"
                        </div>
                        <Show
                            when=move || !agent_events.read().is_empty()
                            fallback=|| {
                                view! {
                                    <p class="text-xs text-slate-600 italic">
                                        "Waiting for events..."
                                    </p>
                                }
                            }
                        >
                            <For
                                each=move || agent_events.read().clone().into_iter()
                                key=|e| e.timestamp
                                let:child
                            >
                                <AgentEventRow event=child />
                            </For>
                        </Show>
                    </div>
                </Show>

                // Input area — styled to match the terminal prompt
                <form
                    on:submit=move |ev| {
                        ev.prevent_default();
                        send_msg();
                    }
                    class="shrink-0 p-4 bg-slate-950 border-t border-slate-800 flex items-center gap-3"
                >
                    <Show when=move || is_streaming.get() fallback=move || view! { <IconPrompt /> }>
                        <span class="w-4 h-4 border-2 border-indigo-400 border-t-transparent rounded-full animate-spin shrink-0" />
                    </Show>
                    <textarea
                        autofocus
                        placeholder="Ask anything about your nodes..."
                        rows=1
                        class="flex-1 bg-transparent border-none text-slate-100 font-mono text-sm placeholder-slate-700 focus:outline-none resize-none"
                        prop:value=move || input_text.get()
                        prop:disabled=move || is_streaming.get()
                        on:input=move |ev| input_text.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" && !ev.shift_key() {
                                ev.prevent_default();
                                send_msg();
                            }
                        }
                    />
                </form>
            </div>
        </div>
    }
}

// ─── MessageBubble component ───────────────────────────────────────────────────

#[component]
fn MessageBubble(msg: UiMessage) -> impl IntoView {
    let is_user = msg.role == ChatRole::User;
    let is_streaming = msg.is_streaming;
    let content = StoredValue::new(msg.content.clone());
    let tool_calls = StoredValue::new(msg.tool_calls_text.clone());
    let tool_open = RwSignal::new(false);

    view! {
        <div class=format!("flex gap-3 {}", if is_user { "flex-row-reverse" } else { "flex-row" })>
            // Avatar
            <div class=format!(
                "w-8 h-8 rounded-xl flex items-center justify-center shrink-0 text-xs font-bold {}",
                if is_user {
                    "bg-cyan-600/20 border border-cyan-500/30 text-cyan-400"
                } else {
                    "bg-indigo-600/20 border border-indigo-500/30 text-indigo-400"
                },
            )>{if is_user { "U" } else { "AI" }}</div>

            // Bubble
            <div class=format!(
                "max-w-[80%] space-y-2 {}",
                if is_user { "items-end" } else { "items-start" },
            )>
                // Tool calls (assistant only)
                <Show when=move || !tool_calls.with_value(|v| v.is_empty())>
                    <div class="space-y-1">
                        <button
                            on:click=move |_| tool_open.update(|v| *v = !*v)
                            class="text-xs text-indigo-400 hover:text-indigo-300 flex items-center gap-1 transition-colors"
                        >
                            <span class="font-mono">
                                {move || {
                                    tool_calls.with_value(|v| format!("{} tool call(s)", v.len()))
                                }}
                            </span>
                            <span>{move || if tool_open.get() { "▲" } else { "▼" }}</span>
                        </button>
                        <Show when=move || tool_open.get()>
                            <div class="space-y-1">
                                {tool_calls
                                    .get_value()
                                    .into_iter()
                                    .map(|(name, input, result)| {
                                        view! {
                                            <ToolCallPanel name=name input=input result=result />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </Show>
                    </div>
                </Show>

                // Text content
                <Show when=move || !content.with_value(|v| v.is_empty()) || is_streaming>
                    <div class=format!(
                        "px-4 py-3 rounded-2xl text-sm leading-relaxed whitespace-pre-wrap break-words {}",
                        if is_user {
                            "bg-indigo-600 text-white rounded-tr-sm"
                        } else {
                            "bg-slate-800 text-slate-200 rounded-tl-sm border border-slate-700"
                        },
                    )>
                        {content.get_value()} <Show when=move || is_streaming>
                            <span class="inline-block w-1.5 h-4 bg-indigo-400 animate-pulse ml-0.5 align-middle" />
                        </Show>
                    </div>
                </Show>
            </div>
        </div>
    }
}

// ─── ToolCallPanel component ───────────────────────────────────────────────────

#[component]
fn ToolCallPanel(name: String, input: String, result: String) -> impl IntoView {
    let is_done = !result.is_empty();
    let input = StoredValue::new(input);
    let result = StoredValue::new(result);

    view! {
        <div class="bg-slate-900 border border-slate-700 rounded-xl p-3 text-xs font-mono space-y-2">
            <div class="flex items-center gap-2">
                <span class=format!(
                    "w-2 h-2 rounded-full shrink-0 {}",
                    if is_done { "bg-emerald-500" } else { "bg-amber-500 animate-pulse" },
                ) />
                <span class="text-indigo-400 font-bold">{name}</span>
            </div>
            <Show when=move || !input.with_value(|v| v.is_empty())>
                <div class="text-slate-500 overflow-x-auto no-scrollbar">
                    <span class="text-slate-600">{"in: "}</span>
                    {input.get_value()}
                </div>
            </Show>
            <Show when=move || !result.with_value(|v| v.is_empty())>
                <div class="text-slate-400 overflow-x-auto no-scrollbar max-h-32">
                    <span class="text-slate-600">{"out: "}</span>
                    {result.get_value()}
                </div>
            </Show>
        </div>
    }
}

// ─── AgentEventRow component ───────────────────────────────────────────────────

#[component]
fn AgentEventRow(event: AgentEvent) -> impl IntoView {
    let (color, prefix) = match event.event_type {
        AgentEventType::ActionTaken => ("text-emerald-400", "✓"),
        AgentEventType::AnomalyDetected => ("text-amber-400", "⚠"),
        AgentEventType::Info => ("text-slate-400", "ℹ"),
        AgentEventType::Error => ("text-rose-400", "✗"),
    };

    let ts = DateTime::<Utc>::from_timestamp(event.timestamp, 0)
        .map(|dt| dt.with_timezone(&Local).format("%H:%M:%S").to_string())
        .unwrap_or_default();

    view! {
        <div class=format!("flex gap-2 text-xs {color}")>
            <span class="shrink-0 font-mono text-slate-500">{ts}</span>
            <span class="shrink-0">{prefix}</span>
            <span class="text-slate-300">{event.description}</span>
        </div>
    }
}
