use super::icons::{IconBook, IconGithub};

use leptos::prelude::*;

#[component]
pub fn AboutView() -> impl IntoView {
    let version = env!("CARGO_PKG_VERSION");

    view! {
        <div class="p-4 lg:p-8 max-w-4xl mx-auto space-y-10 animate-in fade-in slide-in-from-bottom-4 duration-500 text-center">
            <section class="flex flex-col sm:flex-row items-center gap-8 bg-slate-900 border border-slate-800 rounded-2xl p-8 text-center sm:text-left">
                <div class="flex-shrink-0">
                    <img
                        src="formicaio.svg"
                        class="max-w-sm"
                        width="128"
                        alt="Formicaio Logo"
                        class="w-28 h-auto"
                    />
                </div>
                <div>
                    <h2 class="text-2xl font-bold text-white mb-2">Il potere delle formiche</h2>
                    <div class="space-y-4 text-slate-400 italic">
                        <p>
                            Le formiche sono insetti sociali che vivono in colonie e sono note per la loro organizzazione e cooperazione.
                        </p>
                        <p>
                            Ants are social insects that live in colonies and are known for their organization and cooperation.
                        </p>
                    </div>
                </div>
            </section>

            <header class="text-center space-y-6">
                <p class="text-slate-400 text-lg max-w-4xl mx-auto">
                    "Simplify your decentralized experience with this intuitive application, designed to streamline your daily tasks when running nodes from home on peer-to-peer (P2P) networks."
                </p>
            </header>

            <main class="grid grid-cols-1 md:grid-cols-2 gap-6 text-left">
                <InfoCard
                    icon=IconGithub.into_any()
                    title="Open Source"
                    description="Formicaio is proudly open source. Feel free to explore the code, or report issues, on our GitHub repository."
                    link_text="View on GitHub"
                    link_href="https://github.com/bochaco/formicaio"
                />
                <InfoCard
                    icon=IconBook.into_any()
                    title="Documentation"
                    description="Dive deeper into the features, deployment and installation options, and CLI usage by checking out our documentation."
                    link_text="Read the Docs"
                    link_href="https://github.com/bochaco/formicaio/blob/main/README.md"
                />
            </main>

            <footer class="pt-8 border-t border-slate-800 text-center">
                <div class="text-sm font-bold font-mono text-cyan-400">"Version " {version}</div>
                <p class="text-slate-500 text-sm">"Built with passion by @bochaco"</p>
            </footer>
        </div>
    }
}

#[component]
fn InfoCard(
    icon: AnyView,
    title: &'static str,
    description: &'static str,
    link_text: &'static str,
    link_href: &'static str,
) -> impl IntoView {
    view! {
        <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6 hover:border-indigo-500/50 transition-all duration-300 group shadow-lg flex flex-col">
            <div class="flex items-center gap-4 mb-4">
                <div class="text-indigo-400">{icon}</div>
                <h3 class="text-lg font-bold text-white">{title}</h3>
            </div>
            <p class="text-sm text-slate-400 mb-4 flex-grow">{description}</p>
            <a
                href=link_href
                target="_blank"
                rel="noopener noreferrer"
                class="text-sm font-bold text-indigo-400 hover:text-indigo-300 transition-colors"
            >
                {link_text}
                " →"
            </a>
        </div>
    }
}
