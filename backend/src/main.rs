use std::path::PathBuf;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::controllers::static_files::StaticConfig;
use xterm_backend::controllers::terminal_ws::WsAppState;
use xterm_backend::controllers::{
    auth, bookmarks, cloudflare, config, containers, files, servers, snippets, static_files,
    terminal_ws,
};
use xterm_backend::models::db::DbStore;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::vault::Vault;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    // Parse command line arguments
    let mut port: u16 = 9095;
    let mut data_dir: Option<PathBuf> = None;
    let mut dist_dir: Option<PathBuf> = None;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(9095);
                    i += 1;
                }
            }
            "--data-dir" => {
                if i + 1 < args.len() {
                    data_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--dist-dir" => {
                if i + 1 < args.len() {
                    dist_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let base_data_dir = data_dir.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("xterm-rust")
    });
    std::fs::create_dir_all(&base_data_dir)?;

    let vault_path = base_data_dir.join("vault.json");
    let db_path = base_data_dir.join("xterm.sqlite");

    let dist_path = dist_dir.unwrap_or_else(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let candidate = exe_dir.join("../frontend/dist");
        if candidate.exists() {
            candidate
        } else {
            PathBuf::from("frontend/dist")
        }
    });

    info!("Starting xterm-rust backend on port {}", port);
    info!("Data directory: {:?}", base_data_dir);
    info!("Frontend dist directory: {:?}", dist_path);

    let vault = Vault::new(&vault_path).await?;
    let db = DbStore::new(&db_path)?;
    let ssh_pool = SshPool::new();

    let cf_client = CloudflareClient::new();

    let app_state = AppState {
        db: db.clone(),
        vault: vault.clone(),
        pool: ssh_pool.clone(),
        cf: cf_client,
    };

    let ws_state = WsAppState {
        app: app_state.clone(),
        pool: ssh_pool.clone(),
    };

    // Generate random 32-character hex token for session protection in index.html
    let mut token_bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token_bytes);
    let termdeck_token = token_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    let static_config = StaticConfig {
        dist_dir: dist_path,
        token: termdeck_token,
    };

    // Combine routers
    let api_router = axum::Router::new()
        .nest("/api/auth", auth::router(app_state.clone()))
        .nest("/api", servers::router(app_state.clone()))
        .nest("/api", bookmarks::router(app_state.clone()))
        .nest("/api", snippets::router(app_state.clone()))
        .nest("/api", config::router(app_state.clone()))
        .nest("/api", files::router(app_state.clone()))
        .nest("/api", containers::router(app_state.clone()))
        .nest("/api", cloudflare::router(app_state.clone()));

    let ws_router = terminal_ws::router(ws_state);
    let static_router = static_files::router(static_config);

    let app = axum::Router::new()
        .merge(api_router)
        .merge(ws_router)
        .merge(static_router);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Server listening on http://localhost:{}", port);

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutting down server, locking vault...");
        vault.lock().await;
    });

    server.await?;
    Ok(())
}
