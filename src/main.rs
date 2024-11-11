#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use formicaio::{
        app::*, bg_tasks::spawn_bg_tasks, db_client::DbClient, docker_client::DockerClient,
        fileserv::file_and_error_handler, metrics_client::NodesMetrics,
    };
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    logging::log!("Starting Formicaio v{} ...", env!("CARGO_PKG_VERSION"));

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // We'll keep the database and Docker clients instances in server global state.
    let db_client = DbClient::connect().await.unwrap();
    let docker_client = DockerClient::new().await.unwrap();

    let latest_bin_version = Arc::new(Mutex::new(None));
    let nodes_metrics = Arc::new(Mutex::new(NodesMetrics::new(db_client.clone())));
    // We'll use this flag to keep track if server API is being hit by any
    // active client, in order to prevent from polling nodes unnecessarily.
    let server_api_hit = Arc::new(Mutex::new(true));

    spawn_bg_tasks(
        docker_client.clone(),
        latest_bin_version.clone(),
        nodes_metrics.clone(),
        db_client.clone(),
        server_api_hit.clone(),
    );

    let app_state = ServerGlobalState {
        leptos_options,
        db_client,
        docker_client,
        latest_bin_version,
        nodes_metrics,
        server_api_hit,
    };

    let app = Router::new()
        .leptos_routes(&app_state, routes, App)
        .fallback(file_and_error_handler)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    logging::log!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}
