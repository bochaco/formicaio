use super::icons::{IconFirstPage, IconLastPage, IconNextPage, IconPreviousPage};
use crate::app::ClientGlobalState;

use leptos::prelude::*;

#[component]
pub fn PaginationView() -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();
    let num_pages = move || {
        let page_size = context.app_settings.read().node_list_page_size as usize;
        context.nodes.read().1.len().div_ceil(page_size)
    };
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

    Effect::new(move |_| {
        let n = num_pages();
        if n > 0 && context.current_page.get() >= n {
            context.current_page.update(|p| *p = n - 1);
        }
    });

    view! {
        <Show when=move || 1 < num_pages()>
            <nav class="flex items-center justify-center gap-2">
                <PaginationButton
                    title="First page"
                    disabled=Signal::derive(move || context.current_page.get() == 0)
                    on_click=move || context.current_page.update(|p| *p = 0)
                >
                    <IconFirstPage />
                </PaginationButton>

                <For each=move || pages() key=|i: &usize| *i let:page_index>
                    <Show
                        when=move || page_index != context.current_page.get()
                        fallback=move || {
                            view! {
                                <PaginationButton
                                    title="Previous"
                                    disabled=Signal::derive(move || context.current_page.get() == 0)
                                    on_click=move || context.current_page.update(|p| *p -= 1)
                                >
                                    <IconPreviousPage />
                                </PaginationButton>

                                <span class="px-3 py-1.5 text-slate-500">
                                    {move || format!("Page {}/{}", page_index + 1, num_pages())}
                                </span>

                                <PaginationButton
                                    title="Next"
                                    disabled=Signal::derive(move || {
                                        context.current_page.get() >= num_pages() - 1
                                    })
                                    on_click=move || context.current_page.update(|p| *p += 1)
                                >
                                    <IconNextPage />
                                </PaginationButton>
                            }
                                .into_view()
                        }
                    >
                        <PaginationButton
                            title=""
                            on_click=move || context.current_page.update(|p| *p = page_index)
                        >
                            {page_index + 1}
                        </PaginationButton>
                    </Show>
                </For>

                <PaginationButton
                    title="Last page"
                    disabled=Signal::derive(move || context.current_page.get() >= num_pages() - 1)
                    on_click=move || context.current_page.update(|p| *p = num_pages() - 1)
                >
                    <IconLastPage />
                </PaginationButton>
            </nav>
        </Show>
    }
}

#[component]
fn PaginationButton(
    title: &'static str,
    on_click: impl Fn() + 'static,
    #[prop(default = Signal::stored(false))] disabled: Signal<bool>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            on:click=move |_| on_click()
            disabled=disabled
            title=title
            class=move || {
                format!(
                    "flex items-center justify-center h-9 min-w-[36px] px-2 rounded-lg text-sm font-bold transition-colors bg-slate-800 text-slate-400 hover:bg-slate-700 hover:text-white {}",
                    if disabled.get() { "opacity-50 cursor-not-allowed" } else { "" },
                )
            }
        >
            {children()}
        </button>
    }
}
