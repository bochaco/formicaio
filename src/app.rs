use super::{
    add_node::AddNodeView,
    error_template::{AppError, ErrorTemplate},
    node_instance::{NodeInstanceInfo, NodeInstanceView},
    server_api::nodes_instances,
    stats::AggregatedStatsView,
};

use gloo_timers::future::TimeoutFuture;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use std::collections::BTreeMap;

const POLLING_FREQ_MILLIS: u32 = 5000;
// Struct to use client side as a global context/state
#[derive(Clone, Copy, Debug)]
pub struct ClientGlobalState {
    // List of nodes instances and their info/state
    pub nodes: RwSignal<BTreeMap<String, RwSignal<NodeInstanceInfo>>>,
    // Flag to enable/disable nodes' logs stream
    pub logs_stream_is_on: RwSignal<bool>,
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Provide context to manage all client side states that need to be used globally
    provide_context(ClientGlobalState {
        nodes: create_rw_signal(BTreeMap::default()),
        logs_stream_is_on: create_rw_signal(false),
    });

    view! {
        <Stylesheet id="leptos" href="/pkg/formicaio.css" />

        // sets the document title
        <Title text="Formicaio" />

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
            <main>
                <Routes>
                    <Route path="" view=HomePage />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    // we first read (async) the list of nodes instances that currently exist
    let nodes = create_resource(|| (), |_| async move { nodes_instances().await });

    view! {
        <Suspense fallback=move || {
            view! { <p>"Loading..."</p> }
        }>
            {nodes
                .get()
                .map(|nodes| {
                    match nodes {
                        Err(err) => {
                            view! { <p>Failed to load list of nodes: {err.to_string()}</p> }
                                .into_view()
                        }
                        Ok(nodes) => {
                            let context = expect_context::<ClientGlobalState>();
                            context
                                .nodes
                                .set(
                                    nodes
                                        .into_iter()
                                        .map(|(i, n)| (i, create_rw_signal(n)))
                                        .collect(),
                                );
                            if cfg!(feature = "hydrate") {
                                spawn_nodes_list_polling();
                            }
                            view! {
                                // spawn poller task only on client side
                                // show general stats on top
                                <AggregatedStatsView />

                                <AddNodeView />

                                <div class="flex flex-wrap">
                                    <For
                                        each=move || context.nodes.get()
                                        key=|(container_id, _)| container_id.clone()
                                        let:child
                                    >
                                        <NodeInstanceView info=child.1 />
                                    </For>
                                </div>
                            }
                                .into_view()
                        }
                    }
                })}
        </Suspense>
    }
}

// Spawns a task which polls the server to obtain up to date information of nodes instances.
fn spawn_nodes_list_polling() {
    spawn_local(async {
        logging::log!("Polling server every {POLLING_FREQ_MILLIS:?}ms. ...");
        let context = expect_context::<ClientGlobalState>();
        loop {
            TimeoutFuture::new(POLLING_FREQ_MILLIS).await;

            match nodes_instances().await {
                Err(err) => {
                    logging::log!("Failed to get up to date nodes info from server: {err}")
                }
                Ok(nodes) => {
                    // first let's get rid of those removed remotely
                    context
                        .nodes
                        .update(|cx_nodes| cx_nodes.retain(|id, _| nodes.contains_key(id)));
                    // let's now update those with new values
                    context.nodes.with_untracked(|cx_nodes| {
                        for (id, cn) in cx_nodes {
                            nodes.get(id).map(|updated| {
                                if cn.get_untracked() != *updated {
                                    cn.update(|cn| {
                                        if !cn.status.is_changing() {
                                            cn.status = updated.status.clone();
                                        }
                                        cn.status_info = updated.status_info.clone();
                                        cn.balance = updated.balance;
                                        cn.rewards = updated.rewards;
                                        cn.chunks = updated.chunks;
                                        cn.connected_peers = updated.connected_peers;
                                    });
                                }
                            });
                        }
                    });
                    // we can add any new node created remotely, perhaps by another instance of the app
                    nodes
                        .into_iter()
                        .filter(|(id, _)| !context.nodes.get_untracked().contains_key(id))
                        .for_each(|(id, new_node)| {
                            context.nodes.update(|nodes| {
                                let _ = nodes.insert(id.clone(), create_rw_signal(new_node));
                            })
                        });
                }
            }
        }
    });
}
