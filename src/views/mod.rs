pub mod about;
mod actions_batch;
mod add_nodes;
mod alerts;
mod chart;
mod dashboard;
mod form_inputs;
mod helpers;
mod icons;
mod logs_viewer;
mod mcp_view;
mod node_actions;
mod node_instance;
mod nodes_list;
mod pagination;
mod settings;
mod sidebar;
mod sort_nodes;
pub mod terminal;

pub use helpers::truncated_balance_str;

use self::{
    about::AboutView,
    add_nodes::AddNodesForm,
    alerts::{AlertMsg, OfflineMsg},
    chart::MetricsViewerModal,
    dashboard::DashboardView,
    icons::{IconAddNode, IconHamburguer},
    logs_viewer::LogViewerModal,
    mcp_view::McpView,
    nodes_list::NodesListView,
    settings::SettingsView,
    sidebar::Sidebar,
    terminal::TerminalView,
};
use crate::app::ClientGlobalState;

use leptos::prelude::*;
use std::fmt;

const MB_CONVERTION: f64 = 1_048_576.0;
const GB_CONVERTION: f64 = 1_073_741_824.0;

pub fn format_disk_usage(v: u64) -> String {
    let val = v as f64;
    if val > GB_CONVERTION {
        format!("{:.2} GB", val / GB_CONVERTION)
    } else {
        format!("{:.2} MB", val / MB_CONVERTION)
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum ViewType {
    Dashboard,
    Nodes,
    Terminal,
    Mcp,
    Settings,
    About,
}

impl fmt::Display for ViewType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            ViewType::Dashboard => "Dashboard",
            ViewType::Nodes => "Nodes",
            ViewType::Terminal => "Terminal",
            ViewType::Mcp => "AI",
            ViewType::Settings => "Settings",
            ViewType::About => "About",
        };
        write!(f, "{label}")
    }
}

#[component]
pub fn HomeScreenView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    let active_view = RwSignal::new(ViewType::Dashboard);
    let sidebar_open = RwSignal::new(false);
    let add_nodes_modal_open = RwSignal::new(false);
    // this signal keeps the reactive list of log entries
    let (logs, set_logs) = signal(Vec::new());

    let (chart_data, set_chart_data) = signal((vec![], vec![]));
    let set_render_chart = RwSignal::new(false);

    view! {
        <div class="flex h-screen bg-slate-950 text-slate-100 overflow-hidden">
            // Mobile Sidebar Overlay
            <Show when=move || sidebar_open.get()>
                <div
                    class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm lg:hidden"
                    on:click=move |_| sidebar_open.set(false)
                />
            </Show>

            // Sidebar
            <Show when=move || sidebar_open.get()>
                <Sidebar active_view=active_view is_open=sidebar_open />
            </Show>

            // Main Content
            <main class="flex-1 flex flex-col min-w-0 relative">
                // Header
                <header class="h-16 border-b border-slate-800 flex items-center justify-between px-4 lg:px-8 bg-slate-900/50 backdrop-blur-md sticky top-0 z-30">
                    <div class="flex items-center gap-4">
                        <button
                            class="p-2 text-slate-400 hover:text-white"
                            on:click=move |_| sidebar_open.update(|v| *v = !*v)
                        >
                            <IconHamburguer />
                        </button>

                        <h1 class="text-xl font-bold bg-gradient-to-r from-cyan-500 to-cyan-400 bg-clip-text text-transparent">
                            // App name
                            <span class="hidden sm:inline">"Formicaio - "</span>
                            {move || active_view.get().to_string()}
                        </h1>
                    </div>

                    <div class="flex items-center gap-2 md:gap-4">
                        <button
                            on:click=move |_| add_nodes_modal_open.set(true)
                            class="flex items-center gap-2 bg-indigo-600 hover:bg-indigo-500 transition-colors text-white px-4 py-1.5 rounded-lg font-medium text-sm shadow-lg shadow-indigo-500/20"
                        >
                            <IconAddNode />
                            <span class="hidden sm:inline">Add Node</span>
                        </button>
                    </div>
                </header>

                // View Content
                <div class="flex-1 overflow-y-auto p-1 lg:p-2 no-scrollbar">
                    {move || match active_view.get() {
                        ViewType::Dashboard => view! { <DashboardView /> }.into_any(),
                        ViewType::Nodes => {
                            view! { <NodesListView set_logs set_render_chart set_chart_data /> }
                                .into_any()
                        }
                        ViewType::Terminal => view! { <TerminalView /> }.into_any(),
                        ViewType::Settings => view! { <SettingsView /> }.into_any(),
                        ViewType::About => view! { <AboutView /> }.into_any(),
                        ViewType::Mcp => view! { <McpView /> }.into_any(),
                    }}
                </div>
            </main>

            // Add Nodes Modal
            <Show when=move || add_nodes_modal_open.get()>
                <AddNodesForm is_open=add_nodes_modal_open />
            </Show>

            <Show when=move || context.logs_stream_on_for.read().is_some()>
                <LogViewerModal logs />
            </Show>

            // Metrics Viewer Modal
            <Show when=move || context.metrics_update_on_for.read().is_some()>
                <MetricsViewerModal set_render_chart chart_data />
            </Show>

            // Connection Status Modal
            <OfflineMsg />

            // Alert messages toasts
            <AlertMsg />
        </div>
    }
}
