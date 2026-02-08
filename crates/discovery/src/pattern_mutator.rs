use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

/// Detect sequential numeric URL patterns and enumerate neighbors.
pub struct PatternMutator;

static NUMERIC_PATH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"/(?:product|item|listing|user|post|thread|id|page|topic|article|view|node|profile|member|message|order|invoice|ticket|category|tag|p|t|u|f|showthread\.php\?t=)(\d+)")
        .unwrap()
});

static GENERIC_NUMERIC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"/([a-z_-]+)/(\d+)(?:/|$|\?)").unwrap());

impl PatternMutator {
    /// Given a set of discovered URLs, detect numeric patterns and generate
    /// neighboring URLs to enumerate hidden content.
    pub fn mutate(urls: &[String], max_enumerate: u64) -> Vec<String> {
        let mut mutations = Vec::new();
        let mut seen_patterns: std::collections::HashSet<String> = std::collections::HashSet::new();

        for url_str in urls {
            let Ok(url) = Url::parse(url_str) else {
                continue;
            };
            let path = url.path();

            // Try specific named patterns first
            if let Some(cap) = NUMERIC_PATH_RE.captures(path) {
                let num: u64 = match cap[1].parse() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let pattern_key = format!(
                    "{}:{}",
                    url.host_str().unwrap_or(""),
                    &cap[0][..cap[0].len() - cap[1].len()]
                );
                if seen_patterns.contains(&pattern_key) {
                    continue;
                }
                seen_patterns.insert(pattern_key);

                // Enumerate neighbors
                let start = if num > max_enumerate {
                    num - max_enumerate
                } else {
                    1
                };
                let end = num + max_enumerate;
                for i in start..=end {
                    if i == num {
                        continue;
                    }
                    let new_path = path.replace(&format!("{}", num), &format!("{}", i));
                    let mut new_url = url.clone();
                    new_url.set_path(&new_path);
                    mutations.push(new_url.to_string());
                }
            }
            // Generic /segment/123 pattern
            else if let Some(cap) = GENERIC_NUMERIC_RE.captures(path) {
                let segment = &cap[1];
                let num: u64 = match cap[2].parse() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let pattern_key = format!("{}:/{}/", url.host_str().unwrap_or(""), segment);
                if seen_patterns.contains(&pattern_key) {
                    continue;
                }
                seen_patterns.insert(pattern_key);

                let start = if num > max_enumerate {
                    num - max_enumerate
                } else {
                    1
                };
                let end = num + max_enumerate;
                for i in start..=end {
                    if i == num {
                        continue;
                    }
                    let old = format!("/{}/{}", segment, num);
                    let new = format!("/{}/{}", segment, i);
                    let new_path = path.replacen(&old, &new, 1);
                    let mut new_url = url.clone();
                    new_url.set_path(&new_path);
                    mutations.push(new_url.to_string());
                }
            }
        }

        mutations.sort();
        mutations.dedup();
        mutations
    }
}
