use crate::error_template::{AppError, ErrorTemplate};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::{
    node_instance::NodeInstanceView,
    server_api::{add_node_instance, nodes_instances},
    stats::AggregatedStatsView,
};

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

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
                            view! {
                                // show general stats on top
                                <div class="stats shadow flex">
                                    <AggregatedStatsView nodes />
                                </div>

                                // when we click we create a new node instance and add it to the list
                                <button
                                    class="btn btn-square btn-outline btn-wide"
                                    on:click=move |_| add_node_instance(nodes)
                                >
                                    "Add node"
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        class="h-6 w-6"
                                        fill="none"
                                        viewBox="0 0 24 24"
                                        stroke="green"
                                    >
                                        <path stroke-width="3" d="M12 3 L12 20 M3 12 L20 12 Z" />
                                    </svg>
                                </button>

                                <div class="flex flex-wrap">
                                    <For
                                        each=move || nodes.get()
                                        key=|node| node.get().peer_id.clone()
                                        let:child
                                    >
                                        <NodeInstanceView info=child nodes />
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
