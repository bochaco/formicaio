use leptos::*;

use super::icons::IconHamburguer;

#[derive(Clone, PartialEq)]
pub enum AppScreen {
    Nodes,
    About,
}

#[component]
pub fn NavBar() -> impl IntoView {
    let active_screen = create_rw_signal(AppScreen::Nodes);
    let menu_item_class = move |p: AppScreen| {
        if active_screen.get() == p {
            "block py-2 px-3 text-white bg-blue-700 rounded md:bg-transparent md:text-blue-700 md:p-0 dark:text-white md:dark:text-blue-500"
        } else {
            "block py-2 px-3 text-gray-900 rounded hover:bg-gray-100 md:hover:bg-transparent md:border-0 md:hover:text-blue-700 md:p-0 dark:text-white md:dark:hover:text-blue-500 dark:hover:bg-gray-700 dark:hover:text-white md:dark:hover:bg-transparent"
        }
    };

    view! {
        <nav class="bg-white border-gray-200 dark:bg-gray-900">
            <div class="max-w-screen-xl flex flex-wrap items-center justify-between mx-auto p-4">
                <span class="self-center text-2xl font-semibold whitespace-nowrap dark:text-white">
                    Formicaio
                </span>
                <button
                    data-collapse-toggle="navbar-default"
                    type="button"
                    class="inline-flex items-center p-2 w-10 h-10 justify-center text-sm text-gray-500 rounded-lg md:hidden hover:bg-gray-100 focus:outline-none focus:ring-2 focus:ring-gray-200 dark:text-gray-400 dark:hover:bg-gray-700 dark:focus:ring-gray-600"
                    aria-controls="navbar-default"
                    aria-expanded="false"
                >
                    <span class="sr-only">Open main menu</span>
                    <IconHamburguer />
                </button>
                <div class="hidden w-full md:block md:w-auto" id="navbar-default">
                    <ul class="font-medium flex flex-col p-4 md:p-0 mt-4 border border-gray-100 rounded-lg bg-gray-50 md:flex-row md:space-x-8 rtl:space-x-reverse md:mt-0 md:border-0 md:bg-white dark:bg-gray-800 md:dark:bg-gray-900 dark:border-gray-700">
                        <li>
                            <a
                                href="/"
                                on:click=move |_| active_screen.set(AppScreen::Nodes)
                                class=move || menu_item_class(AppScreen::Nodes)
                                aria-current="page"
                            >
                                Nodes
                            </a>
                        </li>
                        <li>
                            <a
                                href="/about"
                                on:click=move |_| active_screen.set(AppScreen::About)
                                class=move || menu_item_class(AppScreen::About)
                            >
                                About
                            </a>
                        </li>
                    </ul>
                </div>
            </div>
        </nav>
    }
}
