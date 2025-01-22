#![recursion_limit = "256"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use formicaio::{
        app::*, bg_tasks::spawn_bg_tasks, db_client::DbClient, docker_client::DockerClient,
        metrics_client::NodesMetrics, node_manager::NodeManager, server_api_types::Stats,
    };
    use leptos::{logging, prelude::*};
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use std::sync::Arc;
    use tokio::sync::{broadcast, Mutex};

    logging::log!("Starting Formicaio v{} ...", env!("CARGO_PKG_VERSION"));

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // We'll keep the database and Docker clients instances in server global state.
    let db_client = DbClient::connect().await.unwrap();
    let docker_client = DockerClient::new().await.unwrap();
    let node_manager = NodeManager::default();

    let latest_bin_version = Arc::new(Mutex::new(None));
    let nodes_metrics = Arc::new(Mutex::new(NodesMetrics::new(db_client.clone())));
    // List of nodes which status is temporarily immutable
    let node_status_locked = ImmutableNodeStatus::default();

    // Channel to send commands to the bg jobs.
    let (bg_tasks_cmds_tx, _rx) = broadcast::channel::<BgTasksCmds>(1_000);
    // Let's read currently cached settings to use and push it to channel
    let settings = db_client.get_settings().await;
    // List of node instaces batches currently in progress
    let node_instaces_batches = Arc::new(Mutex::new((broadcast::channel(3).0, Vec::new())));
    // Flag which indicates if there is an active client querying the public API.
    let server_api_hit = Arc::new(Mutex::new(false));
    let stats = Arc::new(Mutex::new(Stats::default()));

    spawn_bg_tasks(
        docker_client.clone(),
        latest_bin_version.clone(),
        nodes_metrics.clone(),
        db_client.clone(),
        server_api_hit.clone(),
        node_status_locked.clone(),
        bg_tasks_cmds_tx.clone(),
        stats.clone(),
        settings,
    );

    // Spawn the node executor which will take care of run formica nodes
    //spawn_node_exeutor();

    let app_state = ServerGlobalState {
        leptos_options: leptos_options.clone(),
        db_client,
        docker_client,
        node_manager,
        latest_bin_version,
        server_api_hit,
        nodes_metrics,
        node_status_locked,
        bg_tasks_cmds_tx,
        node_instaces_batches,
        stats,
    };

    let app = Router::new()
        .leptos_routes(&app_state, routes, {
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler::<ServerGlobalState, _>(
            shell,
        ))
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
