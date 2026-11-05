use dear_us_cloud::{build_state, config::Config, router, sweep};
use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=warn".into())).init();
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuration problem: {e}");
            std::process::exit(2);
        }
    };
    let bind = cfg.bind.clone();
    tracing::info!(
        "starting: storage={}, cupid={}, microsoft_store={}",
        if cfg.r2.is_some() { "r2" } else { "local disk" },
        cfg.gemini_key.is_some(),
        cfg.microsoft.is_some()
    );
    let state = build_state(cfg).await.unwrap_or_else(|e| {
        eprintln!("startup failed: {e}");
        std::process::exit(1);
    });
    let s = state.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = sweep(&s).await {
                tracing::warn!("sweep failed: {}", e.message);
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    tracing::info!("listening on {bind}");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
