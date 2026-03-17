#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{Router, routing::get};
    use iu_configurator::{app::*, handlers};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
    use tracing::Level;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "iu_configurator=info,tower_http=info".into()),
        )
        .init();

    let conf = get_configuration(None).unwrap();
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

    let app = Router::new()
        .route("/healthz", get(handlers::health))
        .merge(
            Router::new()
                .leptos_routes(&leptos_options, routes, {
                    let leptos_options = leptos_options.clone();
                    move || shell(leptos_options.clone())
                })
                .fallback(leptos_axum::file_and_error_handler(shell))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                ),
        )
        .with_state(leptos_options);

    tracing::info!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
