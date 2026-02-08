use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use growable_bloom_filter::GrowableBloom;
use priority_queue::PriorityQueue;
use tokio::sync::Mutex;
use tracing::debug;
use url::Url;

use darkscraper_core::CrawlJob;

/// Wrapper for priority ordering (higher priority = dequeued first)
#[derive(Debug, Clone)]
struct Priority(f64);

impl PartialEq for Priority {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Priority {}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(Ordering::Equal)
    }
}

/// URL key for the priority queue (must be Hash + Eq)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct UrlKey(String);

/// Per-network priority queue with its own job storage.
struct NetworkQueue {
    queue: PriorityQueue<UrlKey, Priority>,
    jobs: HashMap<String, CrawlJob>,
}

impl NetworkQueue {
    fn new() -> Self {
        Self {
            queue: PriorityQueue::new(),
            jobs: HashMap::new(),
        }
    }

    fn push(&mut self, normalized: String, job: CrawlJob) {
        let priority = Priority(job.priority);
        let key = UrlKey(normalized.clone());
        self.jobs.insert(normalized, job);
        self.queue.push(key, priority);
    }

    fn pop(&mut self) -> Option<CrawlJob> {
        let (key, _priority) = self.queue.pop()?;
        self.jobs.remove(&key.0)
    }

    fn len(&self) -> usize {
        self.queue.len()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

pub struct CrawlFrontier {
    /// Per-network priority queues — workers only pop from their own network
    networks: DashMap<String, Arc<Mutex<NetworkQueue>>>,
    /// Global bloom filter for URL dedup (shared across all networks)
    seen_urls: Arc<Mutex<GrowableBloom>>,
    /// Per-host last-request timestamp for politeness
    host_last_seen: DashMap<String, Instant>,
}

impl Default for CrawlFrontier {
    fn default() -> Self {
        Self::new()
    }
}

impl CrawlFrontier {
    /// Create a new empty frontier.
    pub fn new() -> Self {
        // GrowableBloom starts small and auto-grows as needed.
        // target FP rate 0.1% with initial capacity hint of 100k.
        let bloom = GrowableBloom::new(0.001, 100_000);

        Self {
            networks: DashMap::new(),
            seen_urls: Arc::new(Mutex::new(bloom)),
            host_last_seen: DashMap::new(),
        }
    }

    /// Mark URLs as already seen (for loading from DB at startup).
    /// Does NOT add them to any queue — just marks them in the bloom filter
    /// so they won't be re-crawled.
    pub async fn mark_seen_batch(&self, urls: &[String]) {
        let mut bloom = self.seen_urls.lock().await;
        for url_str in urls {
            if let Ok(url) = Url::parse(url_str) {
                let normalized = Self::normalize_url(&url);
                bloom.insert(&normalized);
            } else {
                // For non-parseable URLs (e.g. freenet keys), use as-is
                bloom.insert(url_str.to_lowercase());
            }
        }
    }

    /// Normalize URL for dedup: lowercase host, remove fragment, strip trailing slash
    fn normalize_url(url: &Url) -> String {
        let mut normalized = url.clone();
        normalized.set_fragment(None);
        let path = normalized.path().to_string();
        if path.len() > 1 && path.ends_with('/') {
            normalized.set_path(&path[..path.len() - 1]);
        }
        normalized.to_string().to_lowercase()
    }

    /// Get or create the network queue for a given network.
    fn get_network_queue(&self, network: &str) -> Arc<Mutex<NetworkQueue>> {
        self.networks
            .entry(network.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(NetworkQueue::new())))
            .clone()
    }

    /// Add a URL to the frontier. Returns true if added, false if already seen.
    pub async fn push(&self, job: CrawlJob) -> bool {
        let normalized = Self::normalize_url(&job.url);
        let is_retry = job.retry_count > 0;
        let network = job.network.clone();

        // Skip bloom check for retries — they were already seen but need re-queuing
        if !is_retry {
            let mut bloom = self.seen_urls.lock().await;
            if bloom.contains(&normalized) {
                return false;
            }
            bloom.insert(&normalized);
        }

        let nq = self.get_network_queue(&network);
        let mut queue = nq.lock().await;
        queue.push(normalized, job);
        true
    }

    /// Add multiple URLs to the frontier in one batch, acquiring locks minimally.
    /// Returns the number of URLs actually enqueued (after dedup).
    pub async fn push_batch(&self, jobs: Vec<CrawlJob>) -> usize {
        if jobs.is_empty() {
            return 0;
        }

        // Partition into retries (bypass bloom) vs fresh URLs
        let (retries, fresh): (Vec<_>, Vec<_>) = jobs.into_iter().partition(|j| j.retry_count > 0);

        // Single bloom lock for all fresh URLs
        let mut to_enqueue = retries;
        if !fresh.is_empty() {
            let mut bloom = self.seen_urls.lock().await;
            for job in fresh {
                let normalized = Self::normalize_url(&job.url);
                if !bloom.contains(&normalized) {
                    bloom.insert(&normalized);
                    to_enqueue.push(job);
                }
            }
        }

        // Group by network, one lock per network
        let mut by_network: HashMap<String, Vec<(String, CrawlJob)>> = HashMap::new();
        for job in to_enqueue {
            let normalized = Self::normalize_url(&job.url);
            by_network.entry(job.network.clone()).or_default().push((normalized, job));
        }
        let mut added = 0;
        for (network, jobs) in by_network {
            let nq = self.get_network_queue(&network);
            let mut queue = nq.lock().await;
            for (normalized, job) in jobs {
                queue.push(normalized, job);
                added += 1;
            }
        }
        added
    }

    /// Pop the highest-priority job for a specific network.
    /// Workers call this with their assigned network — no cross-network pollution.
    pub async fn pop_for_network(&self, network: &str) -> Option<CrawlJob> {
        let nq = self.networks.get(network)?;
        let mut queue = nq.lock().await;
        queue.pop()
    }

    /// Pop up to `n` jobs from a network's queue in a single lock acquisition.
    /// Returns jobs and whether the queue is empty (so callers know to back off).
    pub async fn pop_batch_for_network(&self, network: &str, n: usize) -> Vec<CrawlJob> {
        let Some(nq) = self.networks.get(network) else {
            return Vec::new();
        };
        let mut queue = nq.lock().await;
        let mut batch = Vec::with_capacity(n.min(queue.len()));
        for _ in 0..n {
            match queue.pop() {
                Some(job) => batch.push(job),
                None => break,
            }
        }
        batch
    }

    /// Push back jobs that couldn't be processed (e.g. politeness delay).
    /// Single lock acquisition for all jobs.
    pub async fn push_back(&self, network: &str, jobs: Vec<CrawlJob>) {
        if jobs.is_empty() {
            return;
        }
        let nq = self.get_network_queue(network);
        let mut queue = nq.lock().await;
        for job in jobs {
            let normalized = Self::normalize_url(&job.url);
            queue.push(normalized, job);
        }
    }

    /// Check if a specific network's queue is empty.
    pub async fn is_network_empty(&self, network: &str) -> bool {
        match self.networks.get(network) {
            Some(nq) => nq.lock().await.is_empty(),
            None => true,
        }
    }

    /// Total items across all network queues.
    pub async fn len(&self) -> usize {
        let mut total = 0;
        for entry in self.networks.iter() {
            total += entry.value().lock().await.len();
        }
        total
    }

    /// How many items in a specific network's queue.
    pub async fn network_len(&self, network: &str) -> usize {
        match self.networks.get(network) {
            Some(nq) => nq.lock().await.len(),
            None => 0,
        }
    }

    /// Record that we visited a host now (for politeness delays)
    pub fn record_host_visit(&self, host: &str) {
        self.host_last_seen
            .insert(host.to_string(), Instant::now());
    }

    /// Check if enough time has elapsed since the last visit to this host
    pub fn can_visit_host(&self, host: &str, min_delay: std::time::Duration) -> bool {
        match self.host_last_seen.get(host) {
            Some(last) => last.elapsed() >= min_delay,
            None => true,
        }
    }

    /// How many URLs have been seen (bloom filter estimate).
    pub async fn seen_count(&self) -> usize {
        // GrowableBloom doesn't expose count, but we can track it externally if needed
        0
    }

    /// Add seeds from a list of URL strings.
    /// Seeds ALWAYS bypass the bloom filter — they are re-crawled every session
    /// because they're directories/registries that may have new links.
    pub async fn add_seeds(&self, urls: &[String], network: &str) -> usize {
        let mut added = 0;
        for url_str in urls {
            if let Ok(url) = Url::parse(url_str) {
                let normalized = Self::normalize_url(&url);
                let job = CrawlJob {
                    url,
                    depth: 0,
                    source_url: None,
                    network: network.to_string(),
                    priority: 1.0,
                    retry_count: 0,
                };

                // Add directly to network queue, bypassing bloom check.
                // Mark as seen so discovered links TO seeds are deduped.
                {
                    let mut bloom = self.seen_urls.lock().await;
                    bloom.insert(&normalized);
                }
                let nq = self.get_network_queue(network);
                let mut queue = nq.lock().await;
                queue.push(normalized, job);
                added += 1;
            }
        }
        debug!(added, total = urls.len(), network, "added seeds to frontier");
        added
    }

    // === Backwards compatibility aliases ===

    /// Pop from any network (for legacy code). Checks networks in order.
    pub async fn pop(&self) -> Option<CrawlJob> {
        for entry in self.networks.iter() {
            let mut queue = entry.value().lock().await;
            if let Some(job) = queue.pop() {
                return Some(job);
            }
        }
        None
    }

    /// Check if all network queues are empty.
    pub async fn is_empty(&self) -> bool {
        for entry in self.networks.iter() {
            if !entry.value().lock().await.is_empty() {
                return false;
            }
        }
        true
    }
}
