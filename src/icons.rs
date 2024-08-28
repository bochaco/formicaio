use leptos::*;

#[component]
pub fn IconAddNode() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6"
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
pub fn IconRemoveNode() -> impl IntoView {
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
                d="M6 18L18 6M6 6l12 12"
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
pub fn IconUpgradeNode() -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="green"
        >
            <circle cx="12" cy="12" r="10" stroke="green" stroke-width="2" />
            <path
                stroke="green"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 11 L12 7 L16 11 M12 7 L12 17"
            />
        </svg>
    }
}
