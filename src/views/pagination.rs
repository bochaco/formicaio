use super::icons::{IconFirstPage, IconLastPage, IconNextPage, IconPreviousPage};
use crate::app::{ClientGlobalState, PAGE_SIZE};

use leptos::prelude::*;

#[component]
pub fn PaginationView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let num_pages = move || context.nodes.read().1.len().div_ceil(PAGE_SIZE);
    let pages = move || {
        let total_pages = num_pages();
        if total_pages <= 1 {
            return vec![];
        }

        let current = context.current_page.get();
        let mut indexes = vec![current];

        let remaining = total_pages - current;
        let next_1 = current + (remaining / 5);
        if next_1 > current {
            indexes.push(next_1);

            let next_2 = next_1 + (remaining / 2);
            if next_2 > next_1 {
                indexes.push(next_2);
            }
        } else if current < total_pages - 1 {
            indexes.push(current + 1);
        }

        let next_1 = current - (current / 5);
        if next_1 < current {
            indexes.insert(0, next_1);

            let next_2 = next_1 - (current / 2);
            if next_2 < next_1 {
                indexes.insert(0, next_2);
            }
        } else if current > 1 {
            indexes.insert(0, current - 1);
        }

        indexes
    };

    view! {
        <div class="flex w-full flex-col">
            <div class="divider">
                <Show when=move || (num_pages() > 1) fallback=move || view! { "" }.into_view()>
                    <ul class="flex items-center h-5 text-sm">
                        <li>
                            <label
                                class="pagination-item"
                                on:click=move |_| {
                                    if context.current_page.get() > 0 {
                                        context.current_page.update(|p| *p = 0);
                                    }
                                }
                            >
                                <span class="sr-only">First</span>
                                <IconFirstPage />
                            </label>
                        </li>

                        <For each=move || pages() key=|i: &usize| *i let:page_index>
                            <Show
                                when=move || page_index != context.current_page.get()
                                fallback=move || {
                                    view! {
                                        <li>
                                            <label
                                                class="pagination-item"
                                                on:click=move |_| {
                                                    if context.current_page.get() > 0 {
                                                        context.current_page.update(|p| *p -= 1);
                                                    }
                                                }
                                            >
                                                <span class="sr-only">Previous</span>
                                                <IconPreviousPage />
                                            </label>
                                        </li>

                                        <li class="px-3">
                                            {move || format!("page {}/{}", page_index + 1, num_pages())}
                                        </li>
                                        <li>
                                            <label
                                                class="pagination-item"
                                                on:click=move |_| {
                                                    if context.current_page.get() < num_pages() - 1 {
                                                        context.current_page.update(|p| *p += 1);
                                                    }
                                                }
                                            >
                                                <span class="sr-only">Next</span>
                                                <IconNextPage />
                                            </label>
                                        </li>
                                    }
                                        .into_view()
                                }
                            >
                                <li>
                                    <label
                                        class="pagination-item"
                                        on:click=move |_| {
                                            context.current_page.update(|p| *p = page_index)
                                        }
                                    >
                                        {page_index + 1}
                                    </label>
                                </li>
                            </Show>
                        </For>

                        <li>
                            <label
                                class="pagination-item"
                                on:click=move |_| {
                                    if context.current_page.get() < num_pages() - 1 {
                                        context.current_page.update(|p| *p = num_pages() - 1)
                                    }
                                }
                            >
                                <span class="sr-only">Last</span>
                                <IconLastPage />
                            </label>
                        </li>
                    </ul>
                </Show>
            </div>
        </div>
    }
}
