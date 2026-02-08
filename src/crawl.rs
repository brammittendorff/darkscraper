use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use dashmap::{DashMap, DashSet};
use tokio::signal;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use darkscraper_core::{AppConfig, CrawlJob, FetchConfig, NetworkDriver, PageData};
use darkscraper_discovery::{
    Correlation, CorrelationEngine, FormSpider, InfraProber, PatternMutator, SourceMiner,
};
use darkscraper_frontier::CrawlFrontier;
use darkscraper_networks::{HyphanetDriver, I2pDriver, LokinetDriver, TorDriver, ZeronetDriver};
use darkscraper_parser::parse_response;
use darkscraper_storage::Storage;

use crate::seeds::{get_all_seeds, is_v3_onion};

/// Bundled result from crawling + discovery on a single page.
pub struct CrawlResult {
    page: PageData,
    correlations: Vec<Correlation>,
}

// MAX_PAGES_PER_DOMAIN removed - now defined per-network in NetworkDriver trait

/// Create a CrawlJob from a discovered URL string, or None if it can't be handled.
/// Only accepts http/https URLs with v3 .onion, .i2p, or .bit hosts.
fn make_crawl_job(
    url_str: &str,
    depth: u32,
    source_url: &url::Url,
    drivers: &[Box<dyn NetworkDriver>],
) -> Option<CrawlJob> {
    let parsed = url::Url::parse(url_str).ok()?;

    // Allow http, https, hyphanet, and legacy freenet schemes
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" && scheme != "hyphanet" && scheme != "freenet" {
        return None;
    }

    if !drivers.iter().any(|d| d.can_handle(&parsed)) {
        return None;
    }

    let network = if scheme == "hyphanet" || scheme == "freenet" {
        // Reject bogus hyphanet://unknown/ or freenet://unknown/ URLs from SourceMiner
        // resolving relative paths against opaque hyphanet: base URLs
        if url_str.starts_with("hyphanet://") || url_str.starts_with("freenet://") {
            return None;
        }
        "hyphanet"
    } else {
        let host = parsed.host_str().unwrap_or("");
        if host.ends_with(".onion") {
            if !is_v3_onion(host) {
                return None; // reject v2 onion addresses
            }
            "tor"
        } else if host.ends_with(".i2p") {
            "i2p"
        } else if host.ends_with(".bit") {
            "zeronet"
        } else if host.ends_with(".loki") {
            "lokinet"
        } else {
            return None; // skip clearnet URLs
        }
    };
    let priority = CrawlFrontier::calculate_priority(&parsed, depth + 1);
    Some(CrawlJob {
        url: parsed,
        depth: depth + 1,
        source_url: Some(source_url.to_string()),
        network: network.to_string(),
        priority,
        retry_count: 0,
    })
}

pub async fn run_crawl(
    config: AppConfig,
    seeds: Option<String>,
    seed: Option<String>,
    max_depth: Option<u32>,
) -> Result<()> {
    let max_depth = max_depth.unwrap_or(config.general.max_depth);

    let tor_workers = if config.tor.enabled {
        config.tor.max_concurrency
    } else {
        0
    };
    let i2p_workers = if config.i2p.enabled {
        config.i2p.max_concurrency
    } else {
        0
    };
    let zeronet_workers = if config.zeronet.enabled {
        config.zeronet.max_concurrency
    } else {
        0
    };
    let hyphanet_workers = if config.hyphanet.enabled {
        config.hyphanet.max_concurrency
    } else {
        0
    };
    let lokinet_workers = if config.lokinet.enabled {
        config.lokinet.max_concurrency
    } else {
        0
    };
    let total_workers =
        tor_workers + i2p_workers + zeronet_workers + hyphanet_workers + lokinet_workers;

    // Scale DB pool to worker count + headroom for storage task
    let pool_size = ((total_workers as u32) + 5).max(10);
    let storage = Storage::with_pool_size(&config.database.postgres_url, pool_size).await?;
    storage.run_migrations().await?;

    // --- Frontier with fresh bloom filter each session ---
    // Bloom starts empty: within-session dedup only, allowing cross-session
    // re-crawling of pages that may have new content/links.
    let frontier = Arc::new(CrawlFrontier::new());

    // Collect seed URLs
    let mut seed_urls: Vec<String> = Vec::new();
    if let Some(s) = seed {
        seed_urls.push(s);
    }
    if let Some(seeds_arg) = seeds {
        if std::path::Path::new(&seeds_arg).exists() {
            let content = std::fs::read_to_string(&seeds_arg)?;
            seed_urls.extend(
                content
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty()),
            );
        } else {
            seed_urls.extend(seeds_arg.split(',').map(|s| s.trim().to_string()));
        }
    }

    // Default seeds if none provided
    if seed_urls.is_empty() {
        seed_urls.extend(get_all_seeds().iter().map(|s| s.to_string()));
        info!("no seeds provided, using default seeds");
    }

    // Detect network from URL and add seeds
    for url_str in &seed_urls {
        let network = if url_str.starts_with("hyphanet:") || url_str.starts_with("freenet:") {
            "hyphanet"
        } else if url_str.contains(".onion") {
            "tor"
        } else if url_str.contains(".i2p") {
            "i2p"
        } else if url_str.contains(".bit") {
            "zeronet"
        } else if url_str.contains(".loki") {
            "lokinet"
        } else {
            "tor"
        };
        frontier
            .add_seeds(std::slice::from_ref(url_str), network)
            .await;
    }
    info!(count = seed_urls.len(), "seeds loaded");

    // Build network drivers
    let mut drivers: Vec<Box<dyn NetworkDriver>> = Vec::new();

    if config.tor.enabled {
        match TorDriver::new(
            &config.tor.socks_proxies,
            config.tor.max_concurrency,
            config.tor.min_delay_seconds,
            config.tor.connect_timeout_seconds,
            config.tor.request_timeout_seconds,
        ) {
            Ok(driver) => {
                info!(proxies = ?config.tor.socks_proxies, workers = tor_workers, "tor driver ready");
                drivers.push(Box::new(driver));
            }
            Err(e) => error!("failed to create tor driver: {}", e),
        }
    }

    if config.i2p.enabled {
        match I2pDriver::new(
            &config.i2p.http_proxies,
            config.i2p.max_concurrency,
            config.i2p.min_delay_seconds,
            config.i2p.connect_timeout_seconds,
            config.i2p.request_timeout_seconds,
        ) {
            Ok(driver) => {
                info!(proxies = ?config.i2p.http_proxies, workers = i2p_workers, "i2p driver ready");
                drivers.push(Box::new(driver));
            }
            Err(e) => error!("failed to create i2p driver: {}", e),
        }
    }

    if config.zeronet.enabled {
        match ZeronetDriver::new(
            &config.zeronet.http_proxies,
            config.zeronet.max_concurrency,
            config.zeronet.min_delay_seconds,
            config.zeronet.connect_timeout_seconds,
            config.zeronet.request_timeout_seconds,
        ) {
            Ok(driver) => {
                info!(proxies = ?config.zeronet.http_proxies, workers = zeronet_workers, "zeronet driver ready");
                drivers.push(Box::new(driver));
            }
            Err(e) => error!("failed to create zeronet driver: {}", e),
        }
    }

    if config.hyphanet.enabled {
        match HyphanetDriver::new(
            &config.hyphanet.http_proxies,
            config.hyphanet.max_concurrency,
            config.hyphanet.min_delay_seconds,
            config.hyphanet.connect_timeout_seconds,
            config.hyphanet.request_timeout_seconds,
        ) {
            Ok(driver) => {
                info!(proxies = ?config.hyphanet.http_proxies, workers = hyphanet_workers, "hyphanet driver ready");
                drivers.push(Box::new(driver));
            }
            Err(e) => error!("failed to create hyphanet driver: {}", e),
        }
    }

    if config.lokinet.enabled {
        match LokinetDriver::new(
            &config.lokinet.socks_proxies,
            config.lokinet.max_concurrency,
            config.lokinet.min_delay_seconds,
            config.lokinet.connect_timeout_seconds,
            config.lokinet.request_timeout_seconds,
        ) {
            Ok(driver) => {
                info!(proxies = ?config.lokinet.socks_proxies, workers = lokinet_workers, "lokinet driver ready");
                drivers.push(Box::new(driver));
            }
            Err(e) => error!("failed to create lokinet driver: {}", e),
        }
    }

    let drivers: Arc<Vec<Box<dyn NetworkDriver>>> = Arc::new(drivers);
    let storage = Arc::new(storage);

    let max_timeout = config
        .tor
        .request_timeout_seconds
        .max(config.i2p.request_timeout_seconds)
        .max(config.zeronet.request_timeout_seconds)
        .max(config.hyphanet.request_timeout_seconds)
        .max(config.lokinet.request_timeout_seconds);
    let fetch_config = FetchConfig {
        timeout: std::time::Duration::from_secs(max_timeout),
        max_body_size: config.general.max_body_size_mb * 1024 * 1024,
        follow_redirects: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0"
            .to_string(),
    };

    // Pipeline channels
    // Large channel buffer to prevent blocking (64 workers * 100 = 6400 headroom)
    let (result_tx, mut result_rx) = mpsc::channel::<CrawlResult>(10000);
    let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    // Track which domains we've already probed for infrastructure
    let probed_domains: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Track pages crawled per domain to prevent one domain from monopolizing the queue
    let domain_page_count: Arc<DashMap<String, AtomicUsize>> = Arc::new(DashMap::new());

    // Apply network-specific retry policies (clear dead URLs on startup if requested)
    for driver in drivers.iter() {
        let (clear_on_startup, _) = driver.retry_policy();
        if clear_on_startup {
            let network_name = driver.name();
            match storage.clear_dead_urls_for_network(network_name).await {
                Ok(cleared) if cleared > 0 => {
                    info!(
                        network = network_name,
                        cleared,
                        "cleared dead URLs on startup per network retry policy"
                    );
                }
                Ok(_) => {}
                Err(e) => error!(network = network_name, "failed to clear dead URLs: {}", e),
            }
        }
    }

    // Dead URLs — permanently failed after max retries, never re-crawled.
    // Loaded from DB at startup so they persist across restarts.
    // Uses DashSet for lock-free concurrent reads from all workers.
    let dead_set_loaded = storage.load_dead_urls().await.unwrap_or_default();
    info!(
        count = dead_set_loaded.len(),
        "loaded dead URLs from database"
    );
    let dead_urls: Arc<DashSet<String>> = Arc::new(DashSet::new());
    for url in dead_set_loaded {
        dead_urls.insert(url);
    }

    // Storage task — writes parsed pages + correlations to DB with batched inserts
    let storage_handle = {
        let storage = Arc::clone(&storage);
        let mut shutdown = shutdown_tx.subscribe();
        let result_tx_monitor = result_tx.clone();
        tokio::spawn(async move {
            let mut pages_stored = 0u64;
            let mut last_store_time = std::time::Instant::now();
            let stall_threshold = std::time::Duration::from_secs(300); // 5 minutes
            loop {
                tokio::select! {
                    Some(result) = result_rx.recv() => {
                        match storage.store_page(&result.page).await {
                            Ok(id) => {
                                pages_stored += 1;
                                last_store_time = std::time::Instant::now();

                                // Warn if channel is getting full (potential deadlock risk)
                                let capacity = result_tx_monitor.capacity();
                                if capacity < 1000 {
                                    warn!(
                                        capacity,
                                        pages_stored,
                                        "storage channel low capacity - DB may be slow"
                                    );
                                }

                                info!(page_id = id, url = %result.page.url, total = pages_stored, "stored page");
                            }
                            Err(e) => error!(url = %result.page.url, "store failed: {}", e),
                        }
                        for corr in &result.correlations {
                            if let Err(e) = storage.store_correlation(
                                &corr.domain, &corr.correlation_type, &corr.value
                            ).await {
                                error!(domain = %corr.domain, "correlation store failed: {}", e);
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                        if last_store_time.elapsed() > stall_threshold {
                            warn!(
                                pages_stored,
                                idle_secs = last_store_time.elapsed().as_secs(),
                                "no pages stored in {} seconds — crawl may be stalled",
                                last_store_time.elapsed().as_secs()
                            );
                        }
                    }
                    _ = shutdown.recv() => {
                        info!(pages_stored, "storage task shutting down");
                        while let Ok(result) = result_rx.try_recv() {
                            let _ = storage.store_page(&result.page).await;
                            for corr in &result.correlations {
                                let _ = storage.store_correlation(
                                    &corr.domain, &corr.correlation_type, &corr.value
                                ).await;
                            }
                        }
                        break;
                    }
                }
            }
        })
    };

    // Spawn periodic retry tasks for networks that request them
    // Each network can define its own retry interval via retry_policy()
    let mut retry_handles = Vec::new();
    for driver in drivers.iter() {
        let (_, retry_interval) = driver.retry_policy();
        if retry_interval > 0 {
            let network_name = driver.name().to_string();
            let storage = Arc::clone(&storage);
            let mut shutdown = shutdown_tx.subscribe();

            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(retry_interval));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            match storage.clear_dead_urls_for_network(&network_name).await {
                                Ok(cleared) if cleared > 0 => {
                                    info!(
                                        network = network_name,
                                        cleared,
                                        interval_hours = retry_interval / 3600,
                                        "cleared dead URLs for periodic retry"
                                    );
                                }
                                Ok(_) => {}
                                Err(e) => error!(network = network_name, "failed to clear dead URLs: {}", e),
                            }
                        }
                        _ = shutdown.recv() => {
                            info!(network = network_name, "retry task shutting down");
                            break;
                        }
                    }
                }
            });

            retry_handles.push(handle);
        }
    }

    // Spawn parallel crawl workers
    info!(
        total_workers,
        tor_workers,
        i2p_workers,
        zeronet_workers,
        hyphanet_workers,
        lokinet_workers,
        "spawning crawl workers"
    );
    let mut worker_handles = Vec::new();

    for worker_id in 0..total_workers {
        let frontier = Arc::clone(&frontier);
        let drivers = Arc::clone(&drivers);
        let probed = Arc::clone(&probed_domains);
        let dead = Arc::clone(&dead_urls);
        let domain_counts = Arc::clone(&domain_page_count);
        let crawl_storage = Arc::clone(&storage);
        let result_tx = result_tx.clone();
        let mut shutdown = shutdown_tx.subscribe();
        let fetch_config = fetch_config.clone();

        // Determine which network this worker handles (for startup probes)
        let worker_network = if worker_id < tor_workers {
            "tor"
        } else if worker_id < tor_workers + i2p_workers {
            "i2p"
        } else if worker_id < tor_workers + i2p_workers + zeronet_workers {
            "zeronet"
        } else if worker_id < tor_workers + i2p_workers + zeronet_workers + hyphanet_workers {
            "hyphanet"
        } else {
            "lokinet"
        };
        let worker_network = worker_network.to_string();

        let handle = tokio::spawn(async move {
            // Startup probe: wait UNTIL network proxy is actually ready
            // Don't start crawling until the network can handle requests
            if worker_network != "tor" {
                info!(worker_id, network = %worker_network, "waiting for network to be ready...");

                // Hyphanet and I2P need HTTP probe to verify they're actually ready with peers/tunnels
                // ZeroNet and Lokinet just need TCP port check
                let needs_http_probe = worker_network == "hyphanet" || worker_network == "i2p";

                let mut probe_attempts = 0u32;
                loop {
                    let is_ready = if needs_http_probe {
                        // HTTP probe for Hyphanet/I2P - check it's actually ready, not just port open
                        let probe_url = match worker_network.as_str() {
                            "hyphanet" => std::env::var("HYPHANET_PROXY")
                                .unwrap_or_else(|_| "hyphanet1:8888".to_string()),
                            "i2p" => std::env::var("I2P_PROXY")
                                .unwrap_or_else(|_| "i2p1:4444".to_string()),
                            _ => unreachable!(),
                        };

                        // Try to fetch a test page and check response
                        match reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(10))
                            .build()
                            .ok()
                            .and_then(|client| {
                                tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(async {
                                        client
                                            .get(format!("http://{}/", probe_url))
                                            .send()
                                            .await
                                            .ok()
                                    })
                                })
                            })
                            .and_then(|resp| {
                                tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current()
                                        .block_on(async { resp.text().await.ok() })
                                })
                            }) {
                            Some(text) => {
                                if worker_network == "hyphanet" {
                                    // Hyphanet is ready when NOT showing setup wizard
                                    !text.contains("Set Up Freenet")
                                        && !text.contains("First Time Wizard")
                                } else {
                                    // I2P is ready when NOT showing proxy errors (means tunnels are built)
                                    !text.contains("Proxy error")
                                        && !text.contains("Can't create connection")
                                        && !text.contains("Host is down")
                                }
                            }
                            None => false,
                        }
                    } else {
                        // TCP probe for ZeroNet, Lokinet
                        let probe_addr = match worker_network.as_str() {
                            "zeronet" => std::env::var("ZERONET_PROXY")
                                .unwrap_or_else(|_| "zeronet1:43110".to_string()),
                            "lokinet" => std::env::var("LOKINET_PROXY")
                                .unwrap_or_else(|_| "lokinet1:1080".to_string()),
                            _ => unreachable!(),
                        };
                        tokio::net::TcpStream::connect(&probe_addr).await.is_ok()
                    };

                    if is_ready {
                        info!(worker_id, network = %worker_network, "network is ready");
                        break;
                    } else {
                        probe_attempts += 1;
                        if probe_attempts % 10 == 1 {
                            info!(worker_id, network = %worker_network, attempts = probe_attempts, "still waiting for network...");
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        // No timeout - keep waiting until ready
                    }
                }
            }

            info!(worker_id, network = %worker_network, "worker started");
            let mut idle_count = 0u32;
            loop {
                tokio::select! {
                    _ = shutdown.recv() => {
                        info!(worker_id, "worker shutting down");
                        break;
                    }
                    _ = async {
                        let Some(job) = frontier.pop_for_network(&worker_network).await else {
                            idle_count += 1;
                            if idle_count % 30 == 1 {
                                info!(worker_id, network = %worker_network, idle_count, "queue empty, waiting for URLs");
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            return;
                        };
                        idle_count = 0;

                        let url = job.url.clone();
                        let depth = job.depth;

                        // Skip if this URL is permanently dead (lock-free check via DashSet)
                        if dead.contains(url.as_str()) {
                            return;
                        }

                        // Find appropriate driver
                        let driver = drivers.iter().find(|d| d.can_handle(&url));
                        let Some(driver) = driver else {
                            warn!(worker_id, url = %url, "no driver for URL");
                            return;
                        };

                        // NO POLITENESS CHECKS - just crawl everything in parallel

                        info!(worker_id, url = %url, depth, retry = job.retry_count, "fetching");
                        let resp = match driver.fetch(&url, &fetch_config).await {
                            Ok(r) => r,
                            Err(e) => {
                                let retries = job.retry_count;
                                let max_retries = driver.max_retries();
                                if retries < max_retries {
                                    warn!(worker_id, url = %url, retry = retries, "fetch failed, will retry: {}", e);
                                    let retry_job = CrawlJob {
                                        retry_count: retries + 1,
                                        priority: job.priority * 0.5,
                                        ..job
                                    };
                                    frontier.push(retry_job).await;
                                    // NO SLEEP - immediately try next URL
                                } else {
                                    let err_msg = e.to_string();
                                    let domain = url.host_str().unwrap_or("unknown");
                                    let network = &job.network;
                                    let failure_type = driver.classify_error(&err_msg);
                                    warn!(
                                        worker_id,
                                        url = %url,
                                        retries,
                                        network,
                                        failure_type,
                                        "fetch failed permanently: {}",
                                        err_msg
                                    );
                                    dead.insert(url.to_string());
                                    let _ = crawl_storage.mark_dead(
                                        url.as_str(), network, domain, retries, &err_msg, failure_type
                                    ).await;
                                }
                                return;
                            }
                        };

                        let domain = url.host_str().unwrap_or("unknown").to_string();
                        let url_path = url.path().to_string();
                        let raw_html = String::from_utf8_lossy(&resp.body);

                        // -- Handle special probe responses --

                        // robots.txt
                        if url_path.ends_with("/robots.txt") && resp.status < 400 {
                            let extra = InfraProber::parse_robots_txt(&raw_html, &url);
                            info!(worker_id, url = %url, count = extra.len(), "robots.txt URLs");
                            for url_str in &extra {
                                if let Some(job) = make_crawl_job(url_str, depth, &url, &drivers) {
                                    frontier.push(job).await;
                                }
                            }
                            return;
                        }

                        // sitemap.xml
                        if url_path.contains("sitemap") && url_path.ends_with(".xml") && resp.status < 400 {
                            let extra = InfraProber::parse_sitemap(&raw_html);
                            info!(worker_id, url = %url, count = extra.len(), "sitemap URLs");
                            for url_str in &extra {
                                if let Some(job) = make_crawl_job(url_str, depth, &url, &drivers) {
                                    frontier.push(job).await;
                                }
                            }
                            return;
                        }

                        // favicon.ico → hash for correlation
                        if url_path.ends_with("/favicon.ico") && resp.status < 400 {
                            let corr = CorrelationEngine::hash_favicon(&domain, &resp.body);
                            let _ = crawl_storage.store_correlation(
                                &corr.domain, &corr.correlation_type, &corr.value
                            ).await;
                            return;
                        }

                        // -- Normal HTML processing --
                        let is_html = resp
                            .content_type
                            .as_deref()
                            .map(|ct| ct.contains("text/html"))
                            .unwrap_or(true);

                        if !is_html {
                            return;
                        }

                        let page = match parse_response(&resp) {
                            Ok(p) => p,
                            Err(e) => {
                                warn!(worker_id, url = %url, "parse error: {}", e);
                                return;
                            }
                        };

                        // Increment domain page counter and check network-specific limit
                        let domain_page_count = domain_counts.entry(domain.clone())
                            .or_insert_with(|| AtomicUsize::new(0))
                            .fetch_add(1, Ordering::Relaxed);

                        // Hard stop: Skip domains that exceeded the network's limit
                        let max_pages = driver.max_pages_per_domain();
                        if domain_page_count >= max_pages {
                            warn!(
                                worker_id,
                                url = %url,
                                domain = %domain,
                                count = domain_page_count,
                                max = max_pages,
                                network = %job.network,
                                "domain exceeded page limit, skipping"
                            );
                            return;
                        }

                        // ============ DISCOVERY MODULES ============

                        let mut discovered_urls: Vec<String> = Vec::new();

                        // 1. Source mining
                        let mined = SourceMiner::mine(&raw_html, &url);
                        if !mined.is_empty() {
                            info!(worker_id, url = %url, count = mined.len(), "source miner URLs");
                        }
                        discovered_urls.extend(mined);

                        // 2. Correlations
                        let correlations = CorrelationEngine::extract(&domain, &raw_html, &resp.headers);
                        if !correlations.is_empty() {
                            info!(worker_id, url = %url, count = correlations.len(), "correlations");
                        }

                        // 2.5. Extract I2P base32 address (if visiting human-readable .i2p)
                        // This is I2P-specific but called here because we need the response
                        if url.host_str().map(|h| h.ends_with(".i2p") && !h.ends_with(".b32.i2p")).unwrap_or(false) {
                            if let Some(b32_url) = I2pDriver::extract_base32_address(&resp.headers, &raw_html, &url) {
                                info!(worker_id, url = %url, b32 = %b32_url, "discovered I2P base32 address");
                                discovered_urls.push(b32_url);
                            }
                        }

                        // 3. Form spidering
                        if page.metadata.has_search_form {
                            let forms = FormSpider::find_search_forms(&raw_html, &url);
                            let form_urls = FormSpider::generate_search_urls(&forms);
                            if !form_urls.is_empty() {
                                info!(worker_id, url = %url, count = form_urls.len(), "form spider URLs");
                            }
                            discovered_urls.extend(form_urls);
                        }

                        // 4. Pattern mutation
                        let link_urls: Vec<String> = page.links.iter().map(|l| l.url.clone()).collect();
                        let mutated = PatternMutator::mutate(&link_urls, 2);
                        if !mutated.is_empty() {
                            info!(worker_id, url = %url, count = mutated.len(), "pattern mutator URLs");
                        }
                        discovered_urls.extend(mutated);

                        // 5. Infrastructure probing (once per domain)
                        // Skip for hyphanet — opaque scheme has no domain to probe
                        if url.scheme() != "hyphanet" && url.scheme() != "freenet" {
                            let mut probed_set = probed.lock().await;
                            if !probed_set.contains(&domain) {
                                probed_set.insert(domain.clone());
                                drop(probed_set);

                                let base = format!("{}://{}", url.scheme(), domain);
                                if let Ok(base_url) = url::Url::parse(&base) {
                                    let probes = InfraProber::generate_probes(&base_url);
                                    info!(worker_id, domain = %domain, count = probes.len(), "infra probes queued");
                                    let probe_urls: Vec<String> =
                                        probes.into_iter().map(|p| p.probe_url).collect();
                                    discovered_urls.extend(probe_urls);
                                }
                            }
                        }

                        // 6. Standard links from page
                        for link in &page.links {
                            discovered_urls.push(link.url.clone());
                        }

                        // Enqueue all discovered URLs (dedup + skip dead ones)
                        if depth < max_depth {
                            // Dedup within this page's discovered URLs
                            let mut seen_this_page = HashSet::with_capacity(discovered_urls.len());
                            discovered_urls.retain(|u| seen_this_page.insert(u.clone()));

                            // Collect all jobs into a batch, then push once
                            // Boost priority for NEW domains (never seen before)
                            let probed_set = probed.lock().await;
                            let mut batch: Vec<CrawlJob> = Vec::with_capacity(discovered_urls.len());
                            for url_str in &discovered_urls {
                                // Lock-free dead URL check via DashSet
                                if dead.contains(url_str.as_str()) {
                                    continue;
                                }
                                if let Some(mut job) = make_crawl_job(url_str, depth, &url, &drivers) {
                                    let link_domain = job.url.host_str().unwrap_or("");
                                    if !link_domain.is_empty() {
                                        // MASSIVE priority boost for domains we haven't visited yet
                                        if !probed_set.contains(link_domain) {
                                            job.priority *= 1000.0;
                                        }
                                        // Penalty for domains approaching page limit
                                        // (Hard stop enforced during fetch, this just deprioritizes)
                                        if let Some(count_entry) = domain_counts.get(link_domain) {
                                            let page_count = count_entry.load(Ordering::Relaxed);
                                            // Get the limit from the driver that can handle this URL
                                            if let Some(d) = drivers.iter().find(|d| d.can_handle(&job.url)) {
                                                if page_count > d.max_pages_per_domain() / 2 {
                                                    job.priority *= 0.1; // 10x penalty when > 50% of limit
                                                }
                                            }
                                        }
                                    }
                                    batch.push(job);
                                }
                            }
                            drop(probed_set);
                            if !batch.is_empty() {
                                let enqueued = frontier.push_batch(batch).await;
                                if enqueued > 0 {
                                    info!(worker_id, url = %url, enqueued, "URLs enqueued");
                                }
                            }
                        }

                        // Send to storage with timeout to prevent deadlock
                        let result = CrawlResult { page, correlations };
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(30),
                            result_tx.send(result)
                        ).await {
                            Ok(Ok(_)) => {},
                            Ok(Err(_)) => error!(worker_id, "result channel closed"),
                            Err(_) => {
                                warn!(worker_id, url = %url, "storage channel full for 30s, dropping result to prevent deadlock");
                                // Drop result to prevent deadlock - workers must keep processing
                            }
                        }
                    } => {}
                }
            }
        });
        worker_handles.push(handle);
    }

    // Drop the original sender so channel closes when all workers finish
    drop(result_tx);

    // Wait for shutdown signal
    info!(
        "press Ctrl+C to stop crawling ({} workers active)",
        total_workers
    );
    signal::ctrl_c().await?;
    info!("shutdown signal received");
    let _ = shutdown_tx.send(());

    // Wait for all workers to finish
    for handle in worker_handles {
        let _ = handle.await;
    }
    let _ = storage_handle.await;

    info!("shutdown complete");
    Ok(())
}
