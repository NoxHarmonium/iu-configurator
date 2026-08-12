#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(warnings)]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("startup failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(feature = "ssr")]
async fn run() -> Result<(), String> {
    use std::env;

    use axum::{Extension, Router, routing::get};
    use iu_configurator::{
        app::{App, shell},
        handlers,
        models::{ServerConfig, env::EnvironmentConfig},
    };
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
    use tracing::Level;

    if let Ok(env_file) = env::var("ENV_FILE") {
        dotenvy::from_filename(&env_file)
            .map_err(|e| format!("Failed to load ENV_FILE {env_file}: {e}"))?;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "iu_configurator=info,tower_http=info".into()),
        )
        .try_init()
        .map_err(|e| format!("Failed to initialize tracing subscriber: {e}"))?;

    let config =
        envy::from_env::<EnvironmentConfig>().map_err(|e| format!("Invalid configuration: {e}"))?;
    config
        .validate()
        .map_err(|e| format!("Invalid configuration: {e}"))?;

    let system_config = iu_configurator::repositories::iuc_config::load(&config.config_dir)
        .await
        .map_err(|e| {
            format!(
                "Failed to load iuc-config.yaml from CONFIG_DIR ({}): {e}",
                config.config_dir
            )
        })?;

    let conf =
        get_configuration(None).map_err(|e| format!("Failed to load Leptos configuration: {e}"))?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;

    tracing::info!(
        output_name = %leptos_options.output_name,
        hash_files = leptos_options.hash_files,
        hash_file = %leptos_options.hash_file,
        site_root = %leptos_options.site_root,
        site_pkg_dir = %leptos_options.site_pkg_dir,
        "leptos options loaded"
    );

    if leptos_options.hash_files {
        let hash_path = std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.parent()
                    .map(|d| d.join(leptos_options.hash_file.as_ref()))
            })
            .unwrap_or_default();
        tracing::info!(hash_path = %hash_path.display(), exists = hash_path.exists(), "hash file");
    }
    let routes = generate_route_list(App);

    // handlers::health pulls EnvironmentConfig via axum's Extension extractor
    // (not Leptos' provide_context below, which only reaches leptos_routes).
    // Missing this layer makes every /healthz request 500 at extraction time,
    // before the handler body — and its logging — ever runs.
    let health_config = config.clone();

    let app = Router::new()
        .route("/healthz", get(handlers::health))
        .layer(Extension(health_config))
        .merge(
            Router::new()
                .leptos_routes_with_context(
                    &leptos_options,
                    routes,
                    {
                        move || {
                            provide_context(ServerConfig {
                                config: config.clone(),
                                setup: system_config.clone(),
                            });
                        }
                    },
                    {
                        let leptos_options = leptos_options.clone();
                        move || shell(leptos_options.clone())
                    },
                )
                .fallback(leptos_axum::file_and_error_handler(shell)),
        )
        // Applied after merge so it wraps every route (including /healthz),
        // not just the leptos router it used to be scoped to.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(leptos_options);

    tracing::info!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind server listener on {addr}: {e}"))?;

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| format!("Server exited with error: {e}"))?;

    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
