//! X3 Gateway - REST and GraphQL API for indexed blockchain data.

mod config;
mod db;
mod error;
mod graphql;
mod rest;

use crate::config::GatewayConfig;
use crate::db::Database;
use crate::error::{GatewayError, Result};
use crate::graphql::create_schema;
use crate::rest::create_router;
use clap::Parser;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// X3 Chain API Gateway
#[derive(Parser, Debug)]
#[command(name = "x3-gateway")]
#[command(about = "REST and GraphQL API gateway for X3 Chain")]
#[command(version)]
struct Args {
    /// Config file path
    #[arg(short, long, env = "GATEWAY_CONFIG")]
    config: Option<String>,

    /// HTTP server host
    #[arg(long, env = "GATEWAY_HOST", default_value = "127.0.0.1")]
    host: String,

    /// HTTP server port
    #[arg(short, long, env = "GATEWAY_PORT", default_value_t = 8080)]
    port: u16,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    /// Log level
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    log_level: String,
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_logging(&args.log_level);

    info!("X3 Gateway starting...");

    // Load configuration
    let config = match args.config {
        Some(path) => GatewayConfig::load(&path)?,
        None => {
            let mut config = GatewayConfig::default();
            config.server.host = args.host;
            config.server.port = args.port;
            if let Some(url) = args.database_url {
                config.database.url = url;
            }
            config
        }
    };

    // Connect to database
    let db = Database::connect(&config.database).await?;

    info!("Database connected");

    // Create GraphQL schema
    let schema = create_schema(db.clone());

    // Create REST router
    let app = create_router(db, schema);

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|e| GatewayError::Config(format!("invalid bind address: {e}")))?;

    info!("Server listening on http://{}", addr);
    info!("GraphQL endpoint: http://{}/graphql", addr);
    info!("GraphQL playground: http://{}/graphql/playground", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            if let Err(err) = shutdown_signal().await {
                tracing::error!("graceful shutdown setup failed: {}", err);
            }
        })
        .await
        .map_err(|e| GatewayError::Internal(format!("gateway server failed: {e}")))?;

    info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() -> Result<()> {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| GatewayError::Internal(format!("failed to install Ctrl+C handler: {e}")))
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| GatewayError::Internal(format!("failed to install signal handler: {e}")))?
            .recv()
            .await;
        Ok::<(), GatewayError>(())
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<Result<()>>();

    tokio::select! {
        result = ctrl_c => result?,
        result = terminate => result?,
    }

    info!("Shutdown signal received");
    Ok(())
}
