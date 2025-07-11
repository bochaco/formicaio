use super::{app::ClientGlobalState, types::NodesSortStrategy};

use leptos::prelude::*;

#[component]
pub fn SortStrategyView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    view! {
        <div class="z-50 hidden my-4 text-base list-none bg-white divide-y divide-gray-100 rounded-lg shadow-sm dark:bg-gray-900 dark:divide-gray-600">
            <ul
                class="block py-2.5 px-0 text-sm text-gray-500 bg-transparent border-0 appearance-none dark:text-gray-400 focus:outline-none focus:ring-0 focus:border-gray-200"
                aria-labelledby="user-menu-button"
            >
                {NodesSortStrategy::variants()
                    .into_iter()
                    .map(|variant| {
                        view! {
                            <li>
                                <label
                                    class=move || {
                                        if context.nodes_sort_strategy.read() == variant {
                                            "block px-2 py-0 text-sm text-gray-700 bg-gray-100 dark:bg-gray-600 dark:text-gray-200 dark:hover:text-white"
                                        } else {
                                            "block px-2 py-0 text-sm text-gray-700 hover:bg-gray-100 dark:hover:bg-gray-600 dark:text-gray-200 dark:hover:text-white"
                                        }
                                    }
                                    on:click=move |_| context.nodes_sort_strategy.set(variant)
                                >
                                    "Sort by "
                                    {variant.to_string()}
                                </label>
                            </li>
                        }
                            .into_view()
                    })
                    .collect::<Vec<_>>()}
            </ul>
        </div>
    }
}
