use std::net::SocketAddr;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let json_logs = std::env::var("LOG_FORMAT").ok().as_deref() == Some("json");
    if json_logs {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }
    knotree_accounts::observability_init();
    if let Err(err) = run().await {
        eprintln!("startup failed: {}", err.startup_message());
        std::process::exit(1);
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut interrupt =
            signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
        let mut terminate =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn run() -> knotree_accounts::AppResult<()> {
    let config = knotree_accounts::config::from_env()?;
    let bind: SocketAddr = config.bind.parse().map_err(|err| {
        knotree_accounts::AppError::internal(format!("BIND_ADDR is invalid: {err}"))
    })?;
    let state = knotree_accounts::connect(config).await?;
    knotree_accounts::auth::bootstrap_admin(&state).await?;
    knotree_accounts::auth::ensure_dev_redirects(&state).await?;
    let worker = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Err(err) = knotree_accounts::email_retry(&worker).await {
                tracing::error!(error = %err, "email retry failed");
            }
        }
    });
    let app = knotree_accounts::router(state);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(knotree_accounts::AppError::internal)?;
    tracing::info!(%bind, "Knotree Accounts listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        shutdown_signal().await;
        tracing::info!("shutdown signal received");
    })
    .await
    .map_err(knotree_accounts::AppError::internal)?;
    Ok(())
}
