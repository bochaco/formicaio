use http::status::StatusCode;
use leptos::{error::Errors, prelude::*};
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum AppError {
    #[error("The page you're looking for doesn't exist")]
    NotFound,
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
        }
    }

    pub fn user_message(&self) -> &'static str {
        "The page you're looking for doesn't exist. Please check the URL and try again."
    }
}

// A simple error display component for 404 Not Found errors
#[component]
pub fn ErrorTemplate(
    #[prop(optional)] outside_errors: Option<Errors>,
    #[prop(optional)] errors: Option<RwSignal<Errors>>,
) -> impl IntoView {
    let errors = match outside_errors {
        Some(e) => RwSignal::new(e),
        None => match errors {
            Some(e) => e,
            None => panic!("ErrorTemplate called without any errors to display"),
        },
    };

    let errors = errors.get_untracked();

    // Downcast to our custom error types
    let errors: Vec<AppError> = errors
        .into_iter()
        .filter_map(|(_k, v)| v.downcast_ref::<AppError>().cloned())
        .collect();

    println!("Application errors: {errors:#?}");

    // Set HTTP status code.
    // Only the response code for the first error is actually sent from the server.
    #[cfg(feature = "ssr")]
    {
        use leptos_axum::ResponseOptions;
        let response = use_context::<ResponseOptions>();
        if let Some(response) = response {
            response.set_status(errors[0].status_code());
        }
    }

    view! {
        <div class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900">
            <div class="max-w-md w-full space-y-8 p-8">
                <div class="text-center">
                    <div class="mx-auto h-12 w-12 text-red-500">
                        <svg fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.732 16.5c-.77.833.192 2.5 1.732 2.5z"
                            />
                        </svg>
                    </div>
                    <h1 class="mt-6 text-3xl font-extrabold text-gray-900 dark:text-white">
                        "Page Not Found"
                    </h1>
                </div>

                <div class="bg-white dark:bg-gray-800 shadow rounded-lg p-6 border-l-4 border-red-500">
                    <div class="flex">
                        <div class="flex-shrink-0">
                            <span class="inline-flex items-center justify-center h-8 w-8 rounded-full bg-red-100 dark:bg-red-900">
                                <span class="text-sm font-medium text-red-800 dark:text-red-200">
                                    "404"
                                </span>
                            </span>
                        </div>
                        <div class="ml-3">
                            <h3 class="text-sm font-medium text-gray-900 dark:text-white">
                                "Page Not Found"
                            </h3>
                            <div class="mt-2 text-sm text-gray-700 dark:text-gray-300">
                                <p>
                                    "The page you're looking for doesn't exist. Please check the URL and try again."
                                </p>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="text-center">
                    <a
                        href="/"
                        class="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                    >
                        "Return to Home"
                    </a>
                </div>
            </div>
        </div>
    }
}
