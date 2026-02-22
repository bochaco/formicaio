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
    use eyre::{WrapErr, bail};
    use formicaio::{
        app::{App, AppContext, ServerGlobalState, shell},
        bg_tasks::spawn_bg_tasks,
        db_client::DbClient,
        node_mgr::NodeManager,
    };
    use leptos::{logging, prelude::*};
    use leptos_axum::{LeptosRoutes, generate_route_list};

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

    // Let's read currently cached settings to use
    let settings = db_client.get_settings().await;

    let app_ctx = AppContext::new(db_client).await;

    #[cfg(not(feature = "native"))]
    let node_manager = NodeManager::new(app_ctx.clone()).await.wrap_err(
        "Failed to initialize Docker client. Please ensure Docker is running and accessible.",
    )?;
    #[cfg(feature = "native")]
    let node_manager = NodeManager::new(
        app_ctx.clone(),
        sub_cmds.data_dir_path,
        sub_cmds.no_auto_start,
        sub_cmds.node_start_interval,
    )
    .await
    .wrap_err("Failed to initialize node manager.")?;

    let app_state = ServerGlobalState {
        leptos_options: leptos_options.clone(),
        node_manager,
        app_ctx,
    };

    spawn_bg_tasks(
        app_state.app_ctx.clone(),
        app_state.node_manager.clone(),
        settings.clone(),
    );

    // If enabled by the user start the MCP server
    #[cfg(feature = "native")]
    if sub_cmds.mcp {
        formicaio::bg_tasks::start_mcp_server(
            sub_cmds.mcp_addr,
            app_state.app_ctx.clone(),
            app_state.node_manager.clone(),
        );
    }

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
        .wrap_err(format!("Failed to bind to TCP address {listen_addr}. Please check if the port is available and you have sufficient permissions."))?;
    logging::log!("• Formicaio backend server is now listening on http://{listen_addr}");

    tokio::select! {
        res = axum::serve(listener, app.into_make_service()) => {
            if let Err(err) = res {
                bail!("Failed to start HTTP server. Please check your network configuration and try again: {err:?}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down...");
        }
    }

    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}
