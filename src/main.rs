#![recursion_limit = "256"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> eyre::Result<()> {
    use formicaio::cli_cmds::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use structopt::StructOpt;

    let cmds = CliCmds::from_args();
    match cmds.sub_cmds {
        #[cfg(not(feature = "native"))]
        CliSubCmds::Start => start_backend(cmds.addr).await?,
        #[cfg(feature = "native")]
        CliSubCmds::Start(sub_cmds) => start_backend(cmds.addr, sub_cmds).await?,
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
async fn start_backend(
    listen_addr: Option<std::net::SocketAddr>,
    #[cfg(feature = "native")] sub_cmds: formicaio::cli_cmds::StartSubcommands,
) -> eyre::Result<()> {
    use axum::Router;
    use eyre::WrapErr;
    use formicaio::{
        app::{App, ServerGlobalState, shell},
        bg_tasks::spawn_bg_tasks,
        bg_tasks::{BgTasksCmds, ImmutableNodeStatus},
        db_client::DbClient,
        graphql::create_schema,
        metrics_client::NodesMetrics,
        types::Stats,
    };
    use leptos::{logging, prelude::*};
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use std::sync::Arc;
    use tokio::sync::{RwLock, broadcast};

    logging::log!("Starting Formicaio v{} ...", env!("CARGO_PKG_VERSION"));

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let mut leptos_options = get_configuration(None)
        .wrap_err("Failed to load Leptos configuration from environment variables. Please check your configuration settings.")?
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

    logging::log!("Web service configuration: {leptos_options:?}");
    let listen_addr = listen_addr.unwrap_or(leptos_options.site_addr);
    let routes = generate_route_list(App);

    // We'll keep the database and Docker clients instances in server global state.
    #[cfg(not(feature = "native"))]
    let db_path = None;
    #[cfg(feature = "native")]
    let db_path = sub_cmds.data_dir_path.clone();

    let db_client = DbClient::connect(db_path)
        .await
        .wrap_err("Failed to initialize database connection. Please check your database configuration and permissions.")?;

    // List of nodes which status is temporarily immutable
    let node_status_locked = ImmutableNodeStatus::default();

    #[cfg(not(feature = "native"))]
    let docker_client = formicaio::docker_client::DockerClient::new()
        .await
        .wrap_err(
            "Failed to initialize Docker client. Please ensure Docker is running and accessible.",
        )?;
    #[cfg(feature = "native")]
    let node_manager = formicaio::node_manager::NodeManager::new(
        node_status_locked.clone(),
        sub_cmds.data_dir_path,
    )
    .await
    .wrap_err("Failed to initialize node manager.")?;

    let latest_bin_version = Arc::new(RwLock::new(None));
    let nodes_metrics = Arc::new(RwLock::new(NodesMetrics::new(db_client.clone())));

    // Channel to send commands to the bg jobs.
    let (bg_tasks_cmds_tx, _rx) = broadcast::channel::<BgTasksCmds>(1_000);
    // Let's read currently cached settings to use and push it to channel
    let settings = db_client.get_settings().await;
    // List of node instaces batches currently in progress
    let node_action_batches = Arc::new(RwLock::new((broadcast::channel(3).0, Vec::new())));
    let stats = Arc::new(RwLock::new(Stats::default()));

    let app_state = ServerGlobalState {
        leptos_options: leptos_options.clone(),
        db_client,
        #[cfg(not(feature = "native"))]
        docker_client,
        #[cfg(feature = "native")]
        node_manager,
        latest_bin_version,
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
        *app_state.latest_bin_version.write().await = Some(version);

        // let's create a batch to start nodes which were Active
        use formicaio::types::{InactiveReason, NodeStatus};
        let nodes_in_db = app_state.db_client.get_nodes_list().await;
        let mut active_nodes = vec![];
        for (node_id, node_info) in nodes_in_db {
            if node_info.status.is_active() {
                // let's set it to inactive otherwise it won't be started
                app_state
                    .db_client
                    .update_node_status(&node_id, &NodeStatus::Inactive(InactiveReason::Stopped))
                    .await;
                active_nodes.push(node_id);
            } else if node_info.is_status_locked {
                app_state.db_client.unlock_node_status(&node_id).await;
            }
        }

        let auto_start_interval = if sub_cmds.no_auto_start {
            if sub_cmds.node_start_interval.is_some() {
                eyre::bail!(
                    "Invalid arguments: Cannot set 'node-start-interval' when 'no-auto-start' flag is enabled."
                );
            } else {
                None
            }
        } else {
            sub_cmds.node_start_interval.or(Some(5))
        };

        if let Some(node_start_interval) = auto_start_interval {
            if !active_nodes.is_empty() {
                logging::log!(
                    "Auto-starting {} previously active nodes with {node_start_interval} second intervals",
                    active_nodes.len()
                );
                let _ = formicaio::server_api::helper_node_action_batch(
                    formicaio::types::BatchType::Start(active_nodes),
                    node_start_interval,
                    &app_state,
                )
                .await;
            }
        }
    }

    spawn_bg_tasks(app_state.clone(), settings);

    // Create GraphQL schema
    let schema = create_schema(Arc::new(app_state.clone()));

    let app = Router::new()
        .leptos_routes(&app_state, routes, {
            move || shell(leptos_options.clone())
        })
        .route("/graphql", async_graphql_axum::GraphQL::new(schema))
        .route(
            "/graphiql",
            async_graphql_axum::GraphiQLSource::new("/graphql").finish(),
        )
        .fallback(leptos_axum::file_and_error_handler::<ServerGlobalState, _>(
            shell,
        ))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .wrap_err(format!("Failed to bind to TCP address {listen_addr}. Please check if the port is available and you have sufficient permissions."))?;
    logging::log!("Formicaio backend server is now listening on http://{listen_addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .wrap_err(
            "Failed to start HTTP server. Please check your network configuration and try again.",
        )?;

    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}
