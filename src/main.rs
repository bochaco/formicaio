#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::Mutex;

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use formicaio::fileserv::file_and_error_handler;
    use formicaio::{app::*, metadata_db::DbClient, portainer_client::PortainerClient};
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // We'll keep the database and Portainer clients instances in server global state.
    let db_client = DbClient::connect().await.unwrap();
    let portainer_client = PortainerClient::new(db_client.clone()).await.unwrap();

    let latest_bin_version = Arc::new(Mutex::new(None));

    let app_state = ServerGlobalState {
        leptos_options,
        db_client,
        portainer_client,
        latest_bin_version: latest_bin_version.clone(),
    };

    spawn_bg_tasks(latest_bin_version);

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
fn spawn_bg_tasks(latest_bin_version: Arc<Mutex<Option<String>>>) {
    use tokio::time::{sleep, Duration};
    // check latest version of node binary every couple of hours
    const BIN_VERSION_POLLING_FREQ: Duration = Duration::from_secs(60 * 60 * 2);

    tokio::spawn(async move {
        loop {
            if let Some(version) = latest_version_available().await {
                leptos::logging::log!("Latest version of node binary available: {version}");
                *latest_bin_version.lock().await = Some(version);
            }
            sleep(BIN_VERSION_POLLING_FREQ).await;
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
