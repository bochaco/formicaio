use super::icons::IconSort;
use crate::{
    app::ClientGlobalState,
    types::{NodeSortField, NodesSortStrategy},
};

use leptos::prelude::*;

#[component]
pub fn SortStrategyView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <div class="flex items-center gap-2">
            <button
                class="flex items-center gap-2 px-3 py-1.5 text-xs font-semibold text-slate-400 hover:text-white bg-slate-800 border border-slate-700 rounded-lg transition-all"
                on:click=move |_| {
                    context.nodes_sort_strategy.update(|s| s.is_descending = !s.is_descending)
                }
            >
                <IconSort />
                {move || {
                    if context.nodes_sort_strategy.read().is_descending {
                        "Descending"
                    } else {
                        "Ascending"
                    }
                }}
            </button>

            <div class="relative">
                <select
                    class="appearance-none bg-slate-800 border border-slate-700 text-xs font-semibold text-slate-400 py-1.5 pl-3 pr-8 rounded-lg cursor-pointer focus:outline-none focus:ring-1 focus:ring-indigo-500"
                    prop:value=context.nodes_sort_strategy.read_untracked().as_arg_str()
                    on:change=move |e| {
                        NodesSortStrategy::from_arg_str(&event_target_value(&e))
                            .map(|s| {
                                context.nodes_sort_strategy.set(s);
                            });
                    }
                >
                    {NodeSortField::fields()
                        .into_iter()
                        .map(|field| {
                            view! {
                                <option value=NodesSortStrategy::new(
                                        field,
                                        context.nodes_sort_strategy.read_untracked().is_descending,
                                    )
                                    .as_arg_str()>{field.to_string()}</option>
                            }
                                .into_view()
                        })
                        .collect::<Vec<_>>()}
                </select>
            </div>
        </div>
    }
}
