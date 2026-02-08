use super::{
    ViewType,
    icons::{IconAbout, IconBot, IconDashboard, IconNodes, IconSettings, IconTerminal},
};

use leptos::prelude::*;

#[component]
pub fn Sidebar(active_view: RwSignal<ViewType>, is_open: RwSignal<bool>) -> impl IntoView {
    let version = env!("CARGO_PKG_VERSION");
    let nav_items = [
        (ViewType::Dashboard, "Dashboard", IconDashboard.into_any()),
        (ViewType::Nodes, "Nodes", IconNodes.into_any()),
        (ViewType::Terminal, "Terminal", IconTerminal.into_any()),
        (ViewType::Mcp, "AI", view! { <IconBot /> }.into_any()),
        (ViewType::Settings, "Settings", IconSettings.into_any()),
        (ViewType::About, "About", IconAbout.into_any()),
    ];

    view! {
        <aside class=move || {
            format!(
                "fixed lg:static inset-y-0 left-0 z-50
      w-64 bg-slate-900 border-r border-slate-800 flex flex-col
      transition-transform duration-300 transform {}",
                if is_open.get() { "translate-x-0" } else { "-translate-x-full lg:translate-x-0" },
            )
        }>

            {} <div class="h-16 shrink-0"></div>
            <nav class="flex-1 py-6 px-4 space-y-2 overflow-y-auto no-scrollbar">
                {nav_items
                    .into_iter()
                    .map(|(id, label, icon)| {
                        view! {
                            <button
                                type="button"
                                on:click=move |_| {
                                    active_view.set(id);
                                    is_open.set(false);
                                }
                                class=move || {
                                    format!(
                                        "
              w-full flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-medium transition-all duration-200 {}",
                                        if active_view.read() == id {
                                            "bg-indigo-600/10 text-indigo-400 shadow-sm border border-indigo-500/20"
                                        } else {
                                            "text-slate-400 hover:text-slate-100 hover:bg-slate-800"
                                        },
                                    )
                                }
                            >
                                {icon}
                                {label}
                                <div class=move || {
                                    if active_view.get() == id {
                                        "ml-auto w-1.5 h-1.5 rounded-full bg-indigo-500 shadow-[0_0_8px_rgba(99,102,241,0.8)]"
                                    } else {
                                        ""
                                    }
                                } />
                            </button>
                        }
                    })
                    .collect_view()}
            </nav> <div class="p-4 border-t border-slate-800">
                <span class="text-xs font-semibold text-slate-400 tracking-wider">
                    "Formicaio v" {version}
                </span>
            </div>
        </aside>
    }
}
