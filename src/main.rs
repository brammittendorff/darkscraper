mod cli;
mod commands;
mod crawl;
mod seeds;

use anyhow::Result;
use clap::Parser;
use tracing::warn;

use darkscraper_core::config::AppConfig;

use crate::cli::{Cli, Commands};
use crate::commands::{run_export, run_search, run_status};
use crate::crawl::run_crawl;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let config_str = std::fs::read_to_string(&cli.config).unwrap_or_else(|_| {
        warn!(path = %cli.config, "config file not found, using defaults");
        include_str!("../config/default.toml").to_string()
    });
    let mut config: AppConfig = toml::from_str(&config_str)?;

    // Environment variable overrides for worker/concurrency tuning
    let parse_workers = |v: &str| -> Option<usize> {
        v.parse::<usize>().ok().filter(|&n| n > 0 && n <= 128)
    };
    if let Ok(v) = std::env::var("TOR_WORKERS") {
        if let Some(n) = parse_workers(&v) {
            config.tor.max_concurrency = n;
        }
    }
    if let Ok(v) = std::env::var("I2P_WORKERS") {
        if let Some(n) = parse_workers(&v) {
            config.i2p.max_concurrency = n;
        }
    }
    if let Ok(v) = std::env::var("TOR_ENABLED") {
        config.tor.enabled = v != "0" && v.to_lowercase() != "false";
    }
    if let Ok(v) = std::env::var("I2P_ENABLED") {
        config.i2p.enabled = v != "0" && v.to_lowercase() != "false";
    }
    if let Ok(v) = std::env::var("ZERONET_WORKERS") {
        if let Some(n) = parse_workers(&v) {
            config.zeronet.max_concurrency = n;
        }
    }
    if let Ok(v) = std::env::var("ZERONET_ENABLED") {
        config.zeronet.enabled = v != "0" && v.to_lowercase() != "false";
    }
    if let Ok(v) = std::env::var("FREENET_WORKERS") {
        if let Some(n) = parse_workers(&v) {
            config.freenet.max_concurrency = n;
        }
    }
    if let Ok(v) = std::env::var("FREENET_ENABLED") {
        config.freenet.enabled = v != "0" && v.to_lowercase() != "false";
    }
    if let Ok(v) = std::env::var("LOKINET_WORKERS") {
        if let Some(n) = parse_workers(&v) {
            config.lokinet.max_concurrency = n;
        }
    }
    if let Ok(v) = std::env::var("LOKINET_ENABLED") {
        config.lokinet.enabled = v != "0" && v.to_lowercase() != "false";
    }
    if let Ok(v) = std::env::var("MAX_DEPTH") {
        if let Ok(n) = v.parse::<u32>() {
            config.general.max_depth = n;
        }
    }

    // Build proxy lists from TOR_INSTANCES / I2P_INSTANCES / ZERONET_INSTANCES env vars.
    // When set, these override the config file's proxy lists with generated hostnames.
    // Falls back to the config file values if env var is not set.
    if let Ok(v) = std::env::var("TOR_INSTANCES") {
        if let Ok(n) = v.parse::<usize>() {
            config.tor.socks_proxies = (1..=n).map(|i| format!("tor{}:9050", i)).collect();
        }
    }
    if let Ok(v) = std::env::var("I2P_INSTANCES") {
        if let Ok(n) = v.parse::<usize>() {
            config.i2p.http_proxies = (1..=n).map(|i| format!("i2p{}:4444", i)).collect();
        }
    }
    if let Ok(v) = std::env::var("ZERONET_INSTANCES") {
        if let Ok(n) = v.parse::<usize>() {
            config.zeronet.http_proxies = (1..=n).map(|i| format!("zeronet{}:43110", i)).collect();
        }
    }
    if let Ok(v) = std::env::var("FREENET_INSTANCES") {
        if let Ok(n) = v.parse::<usize>() {
            config.freenet.http_proxies = (1..=n).map(|i| format!("freenet{}:8888", i)).collect();
        }
    }
    if let Ok(v) = std::env::var("LOKINET_INSTANCES") {
        if let Ok(n) = v.parse::<usize>() {
            config.lokinet.socks_proxies = (1..=n).map(|i| format!("lokinet{}:1080", i)).collect();
        }
    }

    match cli.command {
        Commands::Crawl { seeds, seed, depth } => {
            run_crawl(config, seeds, seed, depth).await?;
        }
        Commands::Search {
            query,
            entity,
            entity_type,
            limit,
        } => {
            run_search(config, query, entity, entity_type, limit).await?;
        }
        Commands::Status => {
            run_status(config).await?;
        }
        Commands::Export { format, output } => {
            run_export(config, &format, &output).await?;
        }
    }

    Ok(())
}
