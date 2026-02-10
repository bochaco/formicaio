use super::icons::{IconAlertMsgError, IconBell, IconCancel, IconCheck, IconRemove, IconWarning};
use crate::app::ClientGlobalState;

use chrono::{DateTime, Local, Utc};
use leptos::prelude::*;
use rand::Rng;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationType {
    Success,
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub id: u64,
    pub notif_type: NotificationType,
    pub message: String,
    pub shown: bool,
    pub timestamp: u64,
}

impl Notification {
    pub fn new(message: String, notif_type: NotificationType) -> Self {
        let mut rng = rand::rng();
        let id = rng.random::<u64>();
        Self {
            id,
            notif_type,
            message,
            shown: false,
            timestamp: Utc::now().timestamp() as u64,
        }
    }

    pub fn new_error(message: String) -> Self {
        Self::new(message, NotificationType::Error)
    }

    pub fn new_warning(message: String) -> Self {
        Self::new(message, NotificationType::Warning)
    }

    pub fn new_success(message: String) -> Self {
        Self::new(message, NotificationType::Success)
    }

    pub fn time_ago(&self) -> String {
        let seconds = (Utc::now().timestamp() as u64).saturating_sub(self.timestamp);
        if seconds < 60 {
            "Just now".to_string()
        } else if seconds < 3600 {
            format!("{}m ago", seconds / 60)
        } else if seconds < 86400 {
            format!("{}h ago", seconds / 3600)
        } else {
            DateTime::<Utc>::from_timestamp(self.timestamp as i64, 0)
                .unwrap_or_default()
                .with_timezone(&Local)
                .to_string()
        }
    }

    pub fn color(&self) -> String {
        match &self.notif_type {
            NotificationType::Success => "text-emerald-500".to_string(),
            NotificationType::Error => "text-rose-500".to_string(),
            NotificationType::Warning => "text-amber-500".to_string(),
        }
    }

    pub fn icon_color(&self) -> String {
        match &self.notif_type {
            NotificationType::Success => {
                "text-emerald-500 border-emerald-500/30 bg-slate-900/80".to_string()
            }
            NotificationType::Error => {
                "text-rose-500 border-rose-500/30 bg-slate-900/80".to_string()
            }
            NotificationType::Warning => {
                "text-amber-500 border-amber-500/30 bg-slate-900/80".to_string()
            }
        }
    }

    pub fn icon(&self) -> AnyView {
        match &self.notif_type {
            NotificationType::Success => view! { <IconCheck /> }.into_any(),
            NotificationType::Error => view! { <IconAlertMsgError /> }.into_any(),
            NotificationType::Warning => view! { <IconWarning /> }.into_any(),
        }
    }
}

#[component]
pub fn NotificationsView(is_open: RwSignal<bool>) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <Show when=move || is_open.get()>
            <div class="absolute right-0 top-full mt-2 w-80 sm:w-96 bg-slate-900 border border-slate-800 rounded-2xl shadow-2xl z-50 overflow-hidden animate-in fade-in slide-in-from-top-2 duration-200">
                <header class="p-4 border-b border-slate-800 flex items-center justify-between bg-slate-900/50 backdrop-blur-md">
                    <h3 class="text-sm font-bold text-white flex items-center gap-2">
                        Notifications
                        <span class="px-1.5 py-0.5 bg-indigo-500/10 text-indigo-400 rounded-md text-[10px] uppercase tracking-wider">
                            History
                        </span>
                    </h3>
                    <Show when=move || !context.alerts.read().is_empty()>
                        <button
                            on:click=move |_| {
                                is_open.set(false);
                                context.alerts.update(|notifs| notifs.clear());
                            }
                            class="text-xs font-semibold text-slate-500 hover:text-rose-400 flex items-center gap-1 transition-colors"
                        >
                            <IconRemove />
                            Clear All
                        </button>
                    </Show>
                </header>

                <div class="max-h-[400px] overflow-y-auto no-scrollbar bg-slate-900/80 backdrop-blur-sm">
                    <Show
                        when=move || !context.alerts.read().is_empty()
                        fallback=move || {
                            view! {
                                <div class="p-8 text-center flex flex-col items-center gap-3">
                                    <div class="p-4 bg-slate-800/50 rounded-full text-slate-600">
                                        <IconBell />
                                    </div>
                                    <div>
                                        <p class="text-sm font-medium text-slate-400">
                                            All caught up!
                                        </p>
                                        <p class="text-xs text-slate-600 mt-1">
                                            New alerts will appear here.
                                        </p>
                                    </div>
                                </div>
                            }
                        }
                    >
                        <div class="divide-y divide-slate-800">
                            // Notifications list
                            <For each=move || context.alerts.get() key=|notif| notif.id let:child>

                                <div
                                    prop:key=child.id
                                    class=format!(
                                        "p-4 hover:bg-slate-800/40 transition-colors flex gap-4 relative group {}",
                                        if child.shown { "opacity-80" } else { "" },
                                    )
                                >
                                    <div class="shrink-0 mt-1">
                                        <div class=format!(
                                            "p-2 rounded-lg {}",
                                            child.icon_color(),
                                        )>{child.icon()}</div>
                                    </div>
                                    <div class="flex-1 min-w-0">
                                        <div class="flex items-center justify-between gap-2 mb-0.5">
                                            <span class="text-[10px] font-mono text-slate-500 shrink-0">
                                                {child.time_ago()}
                                            </span>
                                        </div>
                                        <p class="text-xs text-slate-400 leading-relaxed break-words">
                                            {child.message}
                                        </p>
                                    </div>
                                    <button
                                        on:click=move |_| {
                                            context
                                                .alerts
                                                .update(|notifs| notifs.retain(|n| n.id != child.id))
                                        }
                                        class="absolute top-4 right-4 p-1.5 text-slate-600 hover:text-rose-400 hover:bg-rose-500/10 rounded-md transition-all opacity-0 group-hover:opacity-100"
                                        title="Remove"
                                    >
                                        <IconCancel />
                                    </button>
                                </div>
                            </For>
                        </div>
                    </Show>

                </div>

            </div>
        </Show>
    }
}
