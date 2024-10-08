use leptos::*;

use super::icons::{ThreeBarsIcon, ThreeDotsIcon};

#[derive(Clone)]
pub enum AppScreen {
    Nodes,
    About,
}

impl AppScreen {
    pub fn is_nodes(&self) -> bool {
        matches!(self, Self::Nodes)
    }
    pub fn is_about(&self) -> bool {
        matches!(self, Self::About)
    }
}

#[component]
pub fn NavBar(active_screen: RwSignal<AppScreen>) -> impl IntoView {
    let is_menu_visible = create_rw_signal(false);
    let set_active_screen = move |screen: AppScreen| {
        active_screen.set(screen.clone());
        is_menu_visible.set(false);
    };

    view! {
        <div class="navbar bg-base-300">
            <div class="navbar-start">
                <div class="dropdown">
                    <div
                        tabindex="0"
                        class="btn btn-ghost btn-circle"
                        on:click=move |_| is_menu_visible.set(!is_menu_visible.get())
                    >
                        <ThreeBarsIcon />
                    </div>
                    <Show
                        when=move || is_menu_visible.get()
                        fallback=move || view! { "" }.into_view()
                    >
                        <ul
                            tabindex="0"
                            class="menu menu-sm dropdown-content bg-base-100 rounded-box z-[1] mt-3 w-52 p-2 shadow"
                        >

                            <li>
                                <a
                                    class=move || {
                                        if active_screen.get().is_nodes() { "active" } else { "" }
                                    }
                                    on:click=move |_| set_active_screen(AppScreen::Nodes)
                                >
                                    "Nodes"
                                </a>
                            </li>
                            <li>
                                <a
                                    class=move || {
                                        if active_screen.get().is_about() { "active" } else { "" }
                                    }
                                    on:click=move |_| set_active_screen(AppScreen::About)
                                >
                                    "About"
                                </a>
                            </li>
                        </ul>
                    </Show>
                </div>
            </div>
            <div class="navbar-center">
                <a class="shadow-lg">Formicaio</a>
            </div>
            <div class="navbar-end">
                <button class="btn btn-square btn-ghost">
                    <ThreeDotsIcon />
                </button>
            </div>
        </div>
    }
}
