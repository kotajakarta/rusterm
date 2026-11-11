#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use xterm_backend::cli;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::controllers::static_files::StaticConfig;
use xterm_backend::controllers::terminal_ws::WsAppState;
use xterm_backend::controllers::{
    api_errors, auth, bookmarks, cloudflare, request_guard, config, containers, files, servers, snippets, static_files,
    system, terminal_ws,
};
use xterm_backend::models::db::DbStore;
use xterm_backend::services::app_launcher;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::vault::Vault;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let cli = cli::parse_args(&args);
    let port = cli.port;
    let no_browser = cli.no_browser;
    let data_dir = cli.data_dir.clone();
    let dist_dir = cli.dist_dir.clone();

    let exe_path = std::env::current_exe().ok();
    let base_data_dir = app_launcher::resolve_data_dir(data_dir, exe_path);
    std::fs::create_dir_all(&base_data_dir)?;

    #[cfg(target_os = "windows")]
    let file_appender = tracing_appender::rolling::never(&base_data_dir, "rusterm.log");
    #[cfg(target_os = "windows")]
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    #[cfg(target_os = "windows")]
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_writer(non_blocking)
        .finish();
    #[cfg(target_os = "windows")]
    tracing::subscriber::set_global_default(subscriber).ok();

    #[cfg(not(target_os = "windows"))]
    {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::INFO)
            .finish();
        tracing::subscriber::set_global_default(subscriber).ok();
    }

    // Check if an existing RusTerm instance is already running on this port
    if app_launcher::check_existing_instance(port).await {
        info!("Instans RusTerm lain sudah aktif di port {}. Membuka jendela aplikasi dan keluar.", port);
        if !no_browser {
            app_launcher::launch_app_window(port, &base_data_dir);
        }
        return Ok(());
    }

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

    info!("Starting RusTerm backend on port {}", port);
    info!("Data directory: {:?}", base_data_dir);
    info!("Frontend dist directory: {:?}", dist_path);

    let vault = Vault::new(&vault_path).await?;
    let db = DbStore::new(&db_path)?;
    let ssh_pool = SshPool::with_host_keys(db.clone());

    let cf_client = CloudflareClient::new();

    let app_state = AppState {
        db: db.clone(),
        vault: vault.clone(),
        pool: ssh_pool.clone(),
        cf: cf_client,
        transfers: Default::default(),
    };

    let ws_state = WsAppState {
        app: app_state.clone(),
        pool: ssh_pool.clone(),
    };

    // Generate random 32-character hex token for session protection in index.html
    let rusterm_token = cli.token.clone().unwrap_or_else(|| {
        let mut token_bytes = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut token_bytes);
        token_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    });

    let guard_config = request_guard::GuardConfig {
        token: rusterm_token.clone(),
        loopback_only: cli.loopback_only(),
    };
    let static_config = StaticConfig {
        dist_dir: dist_path,
        token: rusterm_token,
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
        .nest("/api", cloudflare::router(app_state.clone()))
        .nest("/api", system::router(app_state.clone()))
        .layer(axum::middleware::map_response(api_errors::wrap_plain_errors));

    let ws_router = terminal_ws::router(ws_state);
    let static_router = static_files::router(static_config);

    let app = axum::Router::new()
        .merge(api_router)
        .merge(ws_router)
        .merge(static_router)
        .layer(axum::middleware::from_fn_with_state(guard_config, request_guard::guard));

    if !cli.loopback_only() {
        tracing::warn!(
            "RusTerm terbuka untuk jaringan di {}:{} — siapa pun yang bisa menjangkau alamat ini dapat mencoba PIN Anda.",
            cli.host,
            port
        );
    }
    let bind_host = cli.host.trim_start_matches('[').trim_end_matches(']').to_string();
    let listener = match TcpListener::bind((bind_host.as_str(), port)).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            info!("Port {} sudah digunakan oleh instans lain. Membuka jendela aplikasi dan keluar secara bersih.", port);
            if !no_browser {
                app_launcher::launch_app_window(port, &base_data_dir);
            }
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    info!("Server listening on http://localhost:{}", port);

    // Launch standalone browser window if requested
    if !no_browser {
        let dir_clone = base_data_dir.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            app_launcher::launch_app_window(port, &dir_clone);
        });
    }

    let pool_clone = ssh_pool.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutting down server, locking vault and disconnecting SSH pool...");
        vault.lock().await;
        pool_clone.disconnect_all().await;
    });

    server.await?;
    Ok(())
}
