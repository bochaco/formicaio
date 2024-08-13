mod node_instance;
mod stats;
mod tools;

use leptos::*;

use node_instance::NodeInstanceView;
use stats::AggregatedStatsView;
use tools::{add_node_instance, read_nodes_instances_info};

fn main() {
    console_error_panic_hook::set_once();

    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    // we first read (async) the list of nodes instances that currently exist
    let nodes = create_resource(|| (), |_| async move { read_nodes_instances_info().await });

    view! {
      <Suspense
          fallback=move || view! { <p>"Loading..."</p> }
      >
        {nodes.get().map(|nodes| view! {
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
                    stroke="green">
                      <path stroke-width="3" d="M12 3 L12 20 M3 12 L20 12 Z" />
                </svg>
            </button>

            <div class="flex flex-wrap">
              <For
                  each=move || nodes.get()
                  key=|node| node.get().peer_id.clone()
                  let:child
              >
                <NodeInstanceView info = child nodes />
              </For>
            </div>
        })}
      </Suspense>
    }
}
