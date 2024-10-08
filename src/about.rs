use leptos::*;

#[component]
pub fn AboutView() -> impl IntoView {
    let version = env!("CARGO_PKG_VERSION");
    view! {
        <div class="hero bg-base-200 min-h-screen">
            <div class="hero-content flex-col lg:flex-row">
                <img src="formicaio.svg" class="max-w-sm" />
                <div>
                    <h1 class="text-3xl font-bold">"Il potere delle formiche"</h1>
                    <p class="py-6">
                        "Le formiche sono insetti sociali che vivono in colonie e sono
                        note per la loro organizzazione e cooperazione."
                    </p>
                    <p class="py-1">
                        "Ants are social insects that live in colonies and are
                        known for their organization and cooperation."
                    </p>
                    <p class="py-1">
                        "Simplify your decentralized experience with this intuitive application,
                        designed to streamline your daily tasks when running nodes from home
                        on peer-to-peer (P2P) networks."
                    </p>
                    <p class="py-1">
                        "Seamlessly participate in online
                        communities using the integrated Nostr client, and manage your
                        digital assets with ease through the built-in wallet."
                    </p>
                    <p class="py-1">
                        "Receive, send,
                        and store tokens, rewards, and coins earned from running nodes or received
                        from third-party sources, all within a single, user-friendly interface."
                    </p>
                    <h2 class="font-bold py-8">"Version: " {version}</h2>
                </div>
            </div>
        </div>
    }
}
