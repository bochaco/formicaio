#![recursion_limit = "256"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> eyre::Result<()> {
    use formicaio::cli_cmds::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use structopt::StructOpt;

    let cmds = CliCmds::from_args();
    match cmds.sub_cmds {
        CliSubCmds::Start => start_backend(cmds.addr).await?,
        CliSubCmds::CliCommands(cmd) => {
            let res = cmd
                .send_request(cmds.addr.unwrap_or(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    52100,
                )))
                .await?;
            res.printstd();
        }
    }
    Ok(())
}

#[cfg(feature = "ssr")]
async fn start_backend(listen_addr: Option<std::net::SocketAddr>) -> eyre::Result<()> {
    use axum::Router;
    use eyre::WrapErr;
    use formicaio::{
        app::*, bg_tasks::spawn_bg_tasks, db_client::DbClient, metrics_client::NodesMetrics,
        server_api_types::Stats,
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
    let mut leptos_options = get_configuration(None)
        .wrap_err("Failed to obtain Leptos config options from env values")?
        .leptos_options;

    // we make sure some values are set to some
    // defaults only if no env vars are being set for them
    if std::env::var("LEPTOS_SITE_ROOT").is_err() {
        leptos_options.site_root = "site".into();
    }
    if std::env::var("LEPTOS_SITE_ADDR").is_err() {
        leptos_options.site_addr = "0.0.0.0:52100".parse().unwrap_or(leptos_options.site_addr);
    }
    if std::env::var("LEPTOS_ENV").is_err() {
        leptos_options.env = "PROD".into();
    }

    logging::log!("Web service config: {leptos_options:?}");
    let listen_addr = listen_addr.unwrap_or(leptos_options.site_addr);
    let routes = generate_route_list(App);

    // We'll keep the database and Docker clients instances in server global state.
    let db_client = DbClient::connect()
        .await
        .wrap_err("Failed to initialise database")?;
    #[cfg(not(feature = "native"))]
    let docker_client = formicaio::docker_client::DockerClient::new()
        .await
        .wrap_err("Failed to initialise Docker client")?;
    #[cfg(feature = "native")]
    let node_manager = formicaio::node_manager::NodeManager::new()
        .await
        .wrap_err("Failed to instantiate node manager")?;

    let latest_bin_version = Arc::new(Mutex::new(None));
    let nodes_metrics = Arc::new(Mutex::new(NodesMetrics::new(db_client.clone())));
    // List of nodes which status is temporarily immutable
    let node_status_locked = ImmutableNodeStatus::default();

    // Channel to send commands to the bg jobs.
    let (bg_tasks_cmds_tx, _rx) = broadcast::channel::<BgTasksCmds>(1_000);
    // Let's read currently cached settings to use and push it to channel
    let settings = db_client.get_settings().await;
    // List of node instaces batches currently in progress
    let node_action_batches = Arc::new(Mutex::new((broadcast::channel(3).0, Vec::new())));
    // Flag which indicates if there is an active client querying the public API.
    let server_api_hit = Arc::new(Mutex::new(false));
    let stats = Arc::new(Mutex::new(Stats::default()));

    let app_state = ServerGlobalState {
        leptos_options: leptos_options.clone(),
        db_client,
        #[cfg(not(feature = "native"))]
        docker_client,
        #[cfg(feature = "native")]
        node_manager,
        latest_bin_version,
        server_api_hit,
        nodes_metrics,
        node_status_locked,
        bg_tasks_cmds_tx,
        node_action_batches,
        stats,
    };

    #[cfg(feature = "native")]
    {
        // let's make sure we have node binary installed before continuing
        let version = app_state
            .node_manager
            .upgrade_master_node_binary(None)
            .await
            .wrap_err("Failed to download node binary")?;
        *app_state.latest_bin_version.lock().await = Some(version);

        // let's create a batch to start nodes which were Active
        let nodes_in_db = app_state.db_client.get_nodes_list().await;
        let active_nodes = nodes_in_db
            .into_iter()
            .filter(|(_, node_info)| node_info.status.is_active())
            .map(|(id, _)| id)
            .collect::<Vec<_>>();

        if !active_nodes.is_empty() {
            // let's set them to inactive otherwise they won't be started
            for node_id in active_nodes.iter() {
                app_state
                    .db_client
                    .update_node_status(node_id, formicaio::node_instance::NodeStatus::Inactive)
                    .await;
            }

            let _ = formicaio::server_api::helper_node_action_batch(
                formicaio::server_api_types::BatchType::Start(active_nodes),
                0,
                &app_state,
            )
            .await;
        }
    }

    spawn_bg_tasks(app_state.clone(), settings);

    let app = Router::new()
        .leptos_routes(&app_state, routes, {
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler::<ServerGlobalState, _>(
            shell,
        ))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .wrap_err("Failed to bind to TCP address")?;
    logging::log!("listening on http://{}", &listen_addr);
    axum::serve(listener, app.into_make_service())
        .await
        .wrap_err("Failed to start HTTP listener")?;

    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}
