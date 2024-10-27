#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::Mutex;

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use formicaio::fileserv::file_and_error_handler;
    use formicaio::{
        app::NodesMetrics, app::*, docker_client::DockerClient, metadata_db::DbClient,
    };
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};

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
    let nodes_metrics = Arc::new(Mutex::new(NodesMetrics::new()));
    spawn_bg_tasks(
        docker_client.clone(),
        latest_bin_version.clone(),
        nodes_metrics.clone(),
    );

    let app_state = ServerGlobalState {
        leptos_options,
        db_client,
        docker_client,
        latest_bin_version,
        nodes_metrics,
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

// Spawn any required background tasks
#[cfg(feature = "ssr")]
fn spawn_bg_tasks(
    docker_client: formicaio::docker_client::DockerClient,
    latest_bin_version: Arc<Mutex<Option<String>>>,
    nodes_metrics: Arc<Mutex<formicaio::app::NodesMetrics>>,
) {
    use formicaio::{metrics_client::NodeMetricsClient, node_instance::NodeStatus};
    use leptos::logging;
    use tokio::time::{sleep, Duration};

    // Check latest version of node binary every couple of hours
    const BIN_VERSION_POLLING_FREQ: Duration = Duration::from_secs(60 * 60 * 2);
    tokio::spawn(async move {
        loop {
            if let Some(version) = latest_version_available().await {
                logging::log!("Latest version of node binary available: {version}");
                *latest_bin_version.lock().await = Some(version);
            }
            sleep(BIN_VERSION_POLLING_FREQ).await;
        }
    });

    // Let's pull the node image already to reduce the time it'll take
    // to create the very first node instance.
    // Also, attempt to pull a new version of the formica image every six hours
    const FORMICA_IMAGE_PULLING_FREQ: Duration = Duration::from_secs(60 * 60 * 6);

    let docker_client_clone = docker_client.clone();
    tokio::spawn(async move {
        loop {
            logging::log!("Pulling formica node image ...");
            if let Err(err) = docker_client_clone.pull_formica_image().await {
                logging::log!("Failed to pull node image when starting up: {err}");
            }
            sleep(FORMICA_IMAGE_PULLING_FREQ).await;
        }
    });

    // Collect metrics from nodes and cache them in global context
    const NODES_METRICS_POLLING_FREQ: Duration = Duration::from_secs(5);

    tokio::spawn(async move {
        loop {
            sleep(NODES_METRICS_POLLING_FREQ).await;

            let containers = match docker_client.get_containers_list(false).await {
                Ok(containers) if !containers.is_empty() => containers,
                _ => continue,
            };

            logging::log!("Polling nodes ({}) metrics ...", containers.len());
            for container in containers {
                let metrics_port = match (
                    NodeStatus::from(&container.State).is_active(),
                    container.metrics_port(),
                ) {
                    (true, Some(metrics_port)) => metrics_port,
                    _ => continue,
                };

                let node_ip = container.node_ip();
                let metrics_client = NodeMetricsClient::new(&node_ip, metrics_port);
                match metrics_client.fetch_metrics().await {
                    Ok(metrics) => {
                        nodes_metrics.lock().await.push(&container.Id, &metrics);
                    }
                    Err(err) => logging::log!(
                        "Failed to pull node metrics from {node_ip:?}:{metrics_port}: {err}"
                    ),
                }
            }
        }
    });
}

// Query crates.io to find out latest version available of the node
#[cfg(feature = "ssr")]
async fn latest_version_available() -> Option<String> {
    let url = format!("https://crates.io/api/v1/crates/{}", "sn_node");
    let client = reqwest::Client::new();
    const MY_USER_AGENT: &str = "formicaio (https://github.com/bochaco/formicaio)";

    let response = match client
        .get(url)
        .header(reqwest::header::USER_AGENT, MY_USER_AGENT)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(_) => return None,
    };

    if response.status().is_success() {
        let body = match response.text().await {
            Ok(body) => body,
            Err(_) => return None,
        };
        let json: serde_json::Value = match serde_json::from_str(&body) {
            Ok(json) => json,
            Err(_) => return None,
        };

        if let Some(version) = json["crate"]["newest_version"].as_str() {
            if let Ok(latest_version) = semver::Version::parse(version) {
                return Some(latest_version.to_string());
            }
        }
    }

    None
}
