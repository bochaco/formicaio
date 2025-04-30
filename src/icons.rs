use leptos::prelude::*;

#[component]
pub fn IconAddNode() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6 mx-2"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
        >
            <path stroke-width="3" d="M12 3 L12 20 M3 12 L20 12 Z" />
        </svg>
    }
}

#[component]
pub fn IconShowLogs() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
        >
            <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M2 2 L15 2 L22 9 L15 9 L15 2 M22 9 L22 22 L2 22 L2 2 M6 9 L11 9 M6 13 L17 13 M6 17 L17 17"
            />
        </svg>
    }
}

#[component]
pub fn IconCancel() -> impl IntoView {
    view! {
        <svg
            class="w-3 h-3"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 14 14"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"
            />
        </svg>
    }
}

#[component]
pub fn IconRecycle() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
        >
            <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 8 L11 3.5 L16 9.5 L13 9 L16.5 7.5 L16 9.5 M17.5 13 L21 18 L13 18 L14.5 16 L14.5 20 L13 18 M8.5 18 L2 18 L6 12 L3 12.5 L7 14 L6 12"
            />
        </svg>
    }
}

#[component]
pub fn IconRemove() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 7h14m-9 3v8m4-8v8M10 3h4a1 1 0 0 1 1 1v3H9V4a1 1 0 0 1 1-1ZM6 7h12v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V7Z"
            />
        </svg>
    }
}

#[component]
pub fn IconStartNode() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
        >
            <polygon points="6,6 18,12 6,18" fill="currentColor" stroke-width="2" />
        </svg>
    }
}

#[component]
pub fn IconStopNode() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
        >
            <rect width="13" height="13" x="5" y="5" fill="currentColor" stroke-width="2" />
        </svg>
    }
}

#[component]
pub fn IconShowChart() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 4v15a1 1 0 0 0 1 1h15M8 16l2.5-5.5 3 3L17.273 7 20 9.667"
            />
        </svg>
    }
}

#[component]
pub fn IconUpgradeNode(
    #[prop(default = "currentColor".to_string())] color: String,
) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="10" stroke=color.clone() stroke-width="2" />
            <path
                stroke=color.clone()
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 11 L12 7 L16 11 M12 7 L12 17"
            />
        </svg>
    }
}

#[component]
pub fn IconAlertMsgError() -> impl IntoView {
    view! {
        <svg
            class="flex-shrink-0 w-4 h-4 mx-2"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="currentColor"
            viewBox="0 0 20 20"
        >
            <path d="M10 .5a9.5 9.5 0 1 0 9.5 9.5A9.51 9.51 0 0 0 10 .5ZM9.5 4a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3ZM12 15H8a1 1 0 0 1 0-2h1v-3H8a1 1 0 0 1 0-2h2a1 1 0 0 1 1 1v4h1a1 1 0 0 1 0 2Z" />
        </svg>
    }
}

#[component]
pub fn IconHamburguer() -> impl IntoView {
    view! {
        <svg
            class="w-5 h-5"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 17 14"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M1 1h15M1 7h15M1 13h15"
            />
        </svg>
    }
}

#[component]
pub fn IconPasteAddr() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 20H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h2.429M7 8h3M8 8V4h4v2m4 0V5h-4m3 4v3a1 1 0 0 1-1 1h-3m9-3v9a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-6.397a1 1 0 0 1 .27-.683l2.434-2.603a1 1 0 0 1 .73-.317H19a1 1 0 0 1 1 1Z"
            />
        </svg>
    }
}

#[component]
pub fn IconSettings() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M21 13v-2a1 1 0 0 0-1-1h-.757l-.707-1.707.535-.536a1 1 0 0 0 0-1.414l-1.414-1.414a1 1 0 0 0-1.414 0l-.536.535L14 4.757V4a1 1 0 0 0-1-1h-2a1 1 0 0 0-1 1v.757l-1.707.707-.536-.535a1 1 0 0 0-1.414 0L4.929 6.343a1 1 0 0 0 0 1.414l.536.536L4.757 10H4a1 1 0 0 0-1 1v2a1 1 0 0 0 1 1h.757l.707 1.707-.535.536a1 1 0 0 0 0 1.414l1.414 1.414a1 1 0 0 0 1.414 0l.536-.535 1.707.707V20a1 1 0 0 0 1 1h2a1 1 0 0 0 1-1v-.757l1.707-.708.536.536a1 1 0 0 0 1.414 0l1.414-1.414a1 1 0 0 0 0-1.414l-.535-.536.707-1.707H20a1 1 0 0 0 1-1Z"
            />
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"
            />
        </svg>
    }
}

#[component]
pub fn IconOpenActionsMenu() -> impl IntoView {
    view! {
        <svg
            class="w-5 h-5 transition-transform group-hover:rotate-45"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 18 18"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 1v16M1 9h16"
            />
        </svg>
    }
}

#[component]
pub fn IconManageNodes() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="currentColor"
            viewBox="0 0 20 20"
        >
            <path d="m13.835 7.578-.005.007-7.137 7.137 2.139 2.138 7.143-7.142-2.14-2.14Zm-10.696 3.59 2.139 2.14 7.138-7.137.007-.005-2.141-2.141-7.143 7.143Zm1.433 4.261L2 12.852.051 18.684a1 1 0 0 0 1.265 1.264L7.147 18l-2.575-2.571Zm14.249-14.25a4.03 4.03 0 0 0-5.693 0L11.7 2.611 17.389 8.3l1.432-1.432a4.029 4.029 0 0 0 0-5.689Z" />
        </svg>
    }
}

#[component]
pub fn IconSelectAll() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 11.917 9.724 16.5 19 7.5"
            />
            <rect stroke="currentColor" width="22" height="22" x="1" y="1" stroke-width="2" />
        </svg>
    }
}

#[component]
pub fn IconSelectActives() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                d="M 1 4 q 6 -6 12 0"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />
            <path
                d="M 3 6 q 4 -4 8 0"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />
            <path
                d="M 5 8 q 2 -2 4 0"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />
            <circle r="1" cx="7" cy="10" fill="currentColor" />

            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M14 17 16 19 20 13"
            />
            <rect stroke="currentColor" width="12" height="12" x="11" y="10" stroke-width="2" />
        </svg>
    }
}

#[component]
pub fn IconSelectInactives() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                d="M 1 4 q 6 -6 12 0"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />
            <path
                d="M 3 6 q 4 -4 8 0"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />
            <path
                d="M 5 8 q 2 -2 4 0"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />
            <circle r="1" cx="7" cy="10" fill="currentColor" />
            <path
                d="M 4 10 12 1"
                stroke="currentColor"
                stroke-width="1"
                fill="none"
                stroke-linecap="round"
            />

            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M14 17 16 19 20 13"
            />
            <rect stroke="currentColor" width="12" height="12" x="11" y="10" stroke-width="2" />
        </svg>
    }
}

#[component]
pub fn IconPreviousPage() -> impl IntoView {
    view! {
        <svg
            class="w-2.5 h-2.5 rtl:rotate-180"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 6 10"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 1 1 5l4 4"
            />
        </svg>
    }
}

#[component]
pub fn IconFirstPage() -> impl IntoView {
    view! {
        <svg
            class="w-2.5 h-2.5 rtl:rotate-180"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 10 10"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 1 5 5l4 4 M1 1 1 9"
            />
        </svg>
    }
}

#[component]
pub fn IconNextPage() -> impl IntoView {
    view! {
        <svg
            class="w-2.5 h-2.5 rtl:rotate-180"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 6 10"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="m1 9 4-4-4-4"
            />
        </svg>
    }
}

#[component]
pub fn IconLastPage() -> impl IntoView {
    view! {
        <svg
            class="w-2.5 h-2.5 rtl:rotate-180"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 10 10"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="m1 9 4-4-4-4 M9 1 9 9"
            />
        </svg>
    }
}

#[component]
pub fn IconSort() -> impl IntoView {
    view! {
        <svg
            class="w-6 h-6 text-gray-800 dark:text-white"
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
        >
            <path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M7 4v16M7 4l3 3M7 4 4 7m9-3h6l-6 6h6m-6.5 10 3.5-7 3.5 7M14 18h4"
            />
        </svg>
    }
}
