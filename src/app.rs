use super::error_template::{AppError, ErrorTemplate};

use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use super::{
    add_node::AddNodeView, node_instance::NodeInstanceView, server_api::nodes_instances,
    stats::AggregatedStatsView,
};

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Provide context to manage flag to enblable/disable nodes' logs stream
    let logs_stream_is_on = create_rw_signal(false);
    provide_context(logs_stream_is_on);

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
                                <AggregatedStatsView nodes />

                                <AddNodeView nodes />

                                <div class="flex flex-wrap">
                                    <For
                                        each=move || nodes.get()
                                        key=|node| node.get().container_id.clone()
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
