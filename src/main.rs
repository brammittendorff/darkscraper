mod cli;
mod commands;
mod crawl;
mod seeds;

use anyhow::Result;
use clap::Parser;
use tracing::warn;

// Use mimalloc to prevent memory bloat (glibc malloc doesn't release memory with high concurrency)
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use darkscraper_core::config::AppConfig;

use crate::cli::{Cli, Commands};
use crate::crawl::run_crawl;

fn main() -> Result<()> {
    // Custom Tokio runtime for high concurrency (186 workers at SCALE_LEVEL=5)
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(32)           // Increase from default (CPU cores)
        .thread_stack_size(8 * 1024 * 1024)  // 8MB stack per thread
        .max_blocking_threads(512)    // Allow more blocking operations
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> Result<()> {
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
    let parse_workers =
        |v: &str| -> Option<usize> { v.parse::<usize>().ok().filter(|&n| n > 0 && n <= 128) };

    // SCALE_LEVEL: Simple 1-5 scaling - sets all networks proportionally
    // Usage: SCALE_LEVEL=3 docker compose up
    if let Ok(v) = std::env::var("SCALE_LEVEL") {
        if let Ok(level) = v.parse::<usize>() {
            if level >= 1 && level <= 5 {
                // Tor: Most content, fast network - scale aggressively
                // Level 1=16w, 2=48w, 3=64w, 4=96w, 5=128w
                config.tor.max_concurrency = match level {
                    1 => 16,
                    2 => 48,
                    3 => 64,
                    4 => 96,
                    5 => 128,
                    _ => 32,
                };
                std::env::set_var("TOR_INSTANCES", (level * 2).min(10).to_string());

                // I2P: Moderate content, moderate latency (1x instances, 4x workers)
                config.i2p.max_concurrency = level * 4;
                std::env::set_var("I2P_INSTANCES", level.to_string());

                // Hyphanet: Very slow network, cap at 3 instances (2x workers, max 12)
                config.hyphanet.max_concurrency = (level * 2).min(12);
                std::env::set_var("HYPHANET_INSTANCES", level.min(3).to_string());

                // Lokinet: Limited content, cap at 4 instances (4x workers, max 20)
                config.lokinet.max_concurrency = (level * 4).min(20);
                std::env::set_var("LOKINET_INSTANCES", level.min(4).to_string());

                // Scale crawl limits
                config.general.max_pages_per_domain = match level {
                    1 => 50,   2 => 100,  3 => 200,  4 => 500,  5 => 1000, _ => 200,
                };
                config.general.max_depth = match level {
                    1 => 5,    2 => 8,    3 => 10,   4 => 15,   5 => 25,   _ => 10,
                };

                eprintln!("🎚️  SCALE_LEVEL={}: Tor={}i/{}w, I2P={}i/{}w, Hyphanet={}i/{}w, Lokinet={}i/{}w | depth={}, pages/domain={}",
                    level,
                    (level * 2).min(10), config.tor.max_concurrency,
                    level, level * 4,
                    level.min(3), (level * 2).min(12),
                    level.min(4), (level * 4).min(20),
                    config.general.max_depth,
                    config.general.max_pages_per_domain
                );
            }
        }
    }

    // Individual overrides (can override SCALE_LEVEL)
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
    if let Ok(v) = std::env::var("HYPHANET_WORKERS") {
        if let Some(n) = parse_workers(&v) {
            config.hyphanet.max_concurrency = n;
        }
    }
    if let Ok(v) = std::env::var("HYPHANET_ENABLED") {
        config.hyphanet.enabled = v != "0" && v.to_lowercase() != "false";
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
    if let Ok(v) = std::env::var("HYPHANET_INSTANCES") {
        if let Ok(n) = v.parse::<usize>() {
            config.hyphanet.http_proxies = (1..=n).map(|i| format!("hyphanet{}:8888", i)).collect();
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
            commands::search::run(config, query, entity, entity_type, limit).await?;
        }
        Commands::Status => {
            commands::status::run(config).await?;
        }
        Commands::AutoRegister {
            max_concurrent,
            max_per_domain,
            networks,
            enable_captcha,
            skip_email_verification,
        } => {
            let storage = darkscraper_storage::Storage::new(&config.database.postgres_url).await?;
            commands::auto_register::run(
                storage,
                Some(max_concurrent),
                Some(max_per_domain),
                networks,
                enable_captcha,
                skip_email_verification,
            )
            .await?;
        }
        Commands::Export { format: _, output: _ } => {
            eprintln!("Export command not yet implemented");
            // TODO: Implement export functionality
        }
    }

    Ok(())
}
