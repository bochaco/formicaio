use super::icons::{IconCode, IconCpu, IconPlug, IconShare, IconShield, IconTerminal, IconZap};
use crate::{app::ClientGlobalState, server_api::get_mcp_info};

use leptos::prelude::*;

#[component]
pub fn McpView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let fetch_mcp_info = Resource::new(
        move || context.is_online.get(),
        async |is_online| {
            if is_online {
                get_mcp_info().await
            } else {
                Ok(None)
            }
        },
    );

    let mcp_info = move || match fetch_mcp_info.get() {
        Some(Ok(info)) => info,
        Some(Err(_)) | None => None::<String>,
    };

    view! {
        <div class="p-4 lg:p-8 max-w-6xl mx-auto space-y-12 animate-in fade-in slide-in-from-bottom-4 duration-500">
            <div class="text-center space-y-4">
                <div class="inline-flex items-center gap-2 px-4 py-1.5 rounded-full bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 text-sm font-bold uppercase tracking-widest">
                    AI Integration
                </div>
                <h2 class="text-4xl font-extrabold tracking-tight">MCP Server Interface</h2>
                <p class="text-slate-400 max-w-2xl mx-auto">
                    "Seamlessly integrate Formicaio with external AI agents via the Model Context Protocol, enabling contextual data exchange and coordinated workflows. "
                    "Enable autonomous node management using agents and automation tools (e.g., Claude, ChatGPT, n8n)."
                </p>
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-3 gap-8">
                <div class="lg:col-span-2 space-y-8">
                    // Connection Info
                    <Suspense fallback=move || {
                        view! { <p>"Retrieving MCP server info..."</p> }
                    }>
                        <div class="bg-slate-900 border border-slate-800 rounded-3xl p-8 shadow-xl">
                            <div class="flex items-center justify-between mb-8">
                                <div class="flex items-center gap-4">
                                    <div class="w-12 h-12 bg-indigo-600 rounded-2xl flex items-center justify-center text-white">
                                        <IconPlug class="w-8 h-8 font-bold" />
                                    </div>
                                    <div>
                                        <h3 class="text-lg font-bold">Server Status</h3>
                                        <div class=move || {
                                            format!(
                                                "flex items-center gap-1.5 text-xs {} font-bold uppercase",
                                                if mcp_info().is_some() {
                                                    "text-emerald-500"
                                                } else {
                                                    "text-rose-500"
                                                },
                                            )
                                        }>
                                            <span class=move || {
                                                format!(
                                                    "w-2 h-2 rounded-full {}",
                                                    if mcp_info().is_some() {
                                                        "bg-emerald-500 animate-pulse"
                                                    } else {
                                                        "bg-rose-500"
                                                    },
                                                )
                                            } />
                                            {move || {
                                                if mcp_info().is_some() { "Active" } else { "Inactive" }
                                            }}
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="bg-slate-950 border border-slate-800 rounded-2xl p-6 font-mono text-sm space-y-4 overflow-x-auto no-scrollbar">
                                <div class="text-slate-500"># Configuration for MCP Client</div>
                                <div class="text-indigo-400">
                                    "ENDPOINT: "
                                    <span class="text-slate-300">
                                        {move || { mcp_info().unwrap_or_else(|| "-".into()) }}
                                    </span>
                                </div>
                            </div>

                        </div>

                        // Tool Capabilities
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                            <FeatureCard
                                icon=view! { <IconCpu class="w-7 h-7 text-cyan-400" /> }.into_view()
                                title="Autonomous Scaling"
                                description="Agent can spin up new nodes based on network demand or performance benchmarks."
                            />
                            <FeatureCard
                                icon=view! { <IconShield class="w-7 h-7 text-emerald-400" /> }
                                    .into_view()
                                title="Self-Healing"
                                description="AI automatically restarts stopped nodes or recycles failing peer IDs without intervention."
                            />
                            <FeatureCard
                                icon=view! { <IconZap class="w-7 h-7 text-amber-400" /> }
                                    .into_view()
                                title="Reward Optimization"
                                description="Predictive analytics to determine best config for maximum yield."
                            />
                            <FeatureCard
                                icon=view! { <IconShare class="w-7 h-7 text-indigo-400" /> }
                                    .into_view()
                                title="Multi‑Instance Orchestration"
                                description="Deploy and manage nodes across multiple VPS providers or Formicaio instances from a single, unified MCP control plane — enabling centralized orchestration, consistent configuration, and seamless failover."
                            />
                        </div>
                    </Suspense>

                </div>

                // Workflow Showcase
                <div class="bg-slate-900 border border-slate-800 rounded-3xl p-8 shadow-xl flex flex-col">
                    <h3 class="text-lg font-bold mb-6 flex items-center gap-2">
                        <IconCode class="w-7 h-7 text-indigo-400" />
                        Example Workflow
                    </h3>

                    <div class="flex-1 space-y-6 relative">
                        <div class="absolute left-[19px] top-6 bottom-6 w-0.5 bg-slate-800" />

                        <WorkflowStep
                            step="1"
                            title="Input Trigger"
                            desc="External Agent detects low network performance or earnings rate decrease."
                        />
                        <WorkflowStep
                            step="2"
                            title="MCP Tool Call"
                            desc="Agent queries 'formicaio_list_nodes' to identify root cause."
                        />
                        <WorkflowStep
                            step="3"
                            title="Optimization Action"
                            desc="Agent triggers 'formicaio_restart_node' on unresponsive units."
                        />
                        <WorkflowStep
                            step="4"
                            title="Verification"
                            desc="Metrics analyzed to confirm system or earnings rate restoration."
                        />
                    </div>

                </div>
            </div>

            // Activation Instructions Card
            <div class="bg-slate-900 border border-slate-800 rounded-3xl p-8 shadow-xl">
                <h3 class="text-lg font-bold mb-6 flex items-center gap-3">
                    <IconTerminal class="w-7 h-7 text-cyan-400" />
                    How to Activate
                </h3>
                <div class="space-y-6 text-slate-300">
                    <p>
                        "You can easily enable the MCP Server by using the "
                        <code class="font-mono bg-slate-800 text-indigo-400 px-1.5 py-0.5 rounded-md">
                            "--mcp"
                        </code>
                        " flag when running Formicaio with the standalone executable. The following command will launch the MCP Server on its default address:"
                    </p>

                    <div>
                        <label class="text-xs font-bold text-slate-500 uppercase tracking-wider">
                            Linux / macOS
                        </label>
                        <pre class="bg-slate-950 border border-slate-800 rounded-lg p-4 mt-2 font-mono text-sm text-slate-200 overflow-x-auto">
                            <code>"./formicaio start --mcp"</code>
                        </pre>
                    </div>

                    <div>
                        <label class="text-xs font-bold text-slate-500 uppercase tracking-wider">
                            Windows
                        </label>
                        <pre class="bg-slate-950 border border-slate-800 rounded-lg p-4 mt-2 font-mono text-sm text-slate-200 overflow-x-auto">
                            <code>"formicaio.exe start --mcp"</code>
                        </pre>
                    </div>

                    <p>
                        "If you wish to specify a different IP address and port for the MCP server, you can do so by using the "
                        <code class="font-mono bg-slate-800 text-indigo-400 px-1.5 py-0.5 rounded-md">
                            "--mcp-addr <IP>:<port>"
                        </code>" argument."
                    </p>
                </div>
            </div>
        </div>
    }
}

#[component]
fn FeatureCard(
    icon: impl IntoView,
    title: &'static str,
    description: &'static str,
) -> impl IntoView {
    view! {
        <div class="bg-slate-900 border border-slate-800 p-6 rounded-2xl hover:bg-slate-800/50 transition-colors">
            <div class="mb-4">{icon}</div>
            <h4 class="font-bold mb-2">{title}</h4>
            <p class="text-xs text-slate-400 leading-relaxed">{description}</p>
        </div>
    }
}

#[component]
fn WorkflowStep(
    step: &'static str,
    title: &'static str,
    desc: &'static str,
    #[prop(default = true)] active: bool,
    #[prop(default = false)] loading: bool,
) -> impl IntoView {
    view! {
        <div class="flex gap-6 relative group">
            <div class=format!(
                "w-10 h-10 rounded-full flex items-center justify-center text-xs font-bold border-2 z-10 transition-colors {}",
                if active {
                    "bg-indigo-600 border-indigo-500 text-white shadow-lg shadow-indigo-500/30"
                } else if loading {
                    "bg-slate-800 border-indigo-400 text-indigo-400 animate-pulse"
                } else {
                    "bg-slate-900 border-slate-800 text-slate-600"
                },
            )>{step}</div>
            <div class="flex-1 pb-4">
                <h5 class=format!(
                    "text-sm font-bold {}",
                    if active || loading { "text-white" } else { "text-slate-500" },
                )>{title}</h5>
                <p class="text-xs text-slate-500 mt-1">{desc}</p>
            </div>
        </div>
    }
}
