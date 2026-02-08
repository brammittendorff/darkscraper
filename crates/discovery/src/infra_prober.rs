use url::Url;

/// For each domain, generate a list of probe paths to request.
/// Returns discovered URLs from robots.txt, sitemaps, and a dark-web-specific wordlist.
pub struct InfraProber;

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub probe_url: String,
    pub probe_type: String,
}

/// Essential infrastructure paths to probe on every domain.
#[allow(dead_code)]
const _ESSENTIAL_PROBES: &[(&str, &str)] = &[
    ("/robots.txt", "robots_txt"),
    ("/sitemap.xml", "sitemap"),
    ("/sitemap_index.xml", "sitemap"),
    ("/.well-known/security.txt", "security_txt"),
    ("/crossdomain.xml", "crossdomain"),
    ("/humans.txt", "humans_txt"),
    ("/favicon.ico", "favicon"),
];

/// Server misconfiguration probes.
#[allow(dead_code)]
const _MISCONFIG_PROBES: &[(&str, &str)] = &[
    ("/server-status", "server_status"),
    ("/server-info", "server_info"),
    ("/.env", "env_file"),
    ("/.git/config", "git_config"),
    ("/.git/HEAD", "git_head"),
    ("/wp-json/", "wordpress_api"),
    ("/api/", "api_root"),
    ("/swagger.json", "swagger"),
    ("/openapi.json", "openapi"),
    ("/graphql", "graphql"),
    ("/debug/", "debug"),
    ("/phpinfo.php", "phpinfo"),
    ("/.DS_Store", "ds_store"),
];

/// Dark-web-specific directory wordlist.
#[allow(dead_code)]
const _DARKWEB_PATHS: &[&str] = &[
    "/admin",
    "/login",
    "/panel",
    "/dashboard",
    "/cp",
    "/forum",
    "/board",
    "/chat",
    "/messages",
    "/inbox",
    "/market",
    "/shop",
    "/store",
    "/products",
    "/listings",
    "/paste",
    "/upload",
    "/files",
    "/documents",
    "/dump",
    "/api",
    "/v1",
    "/v2",
    "/graphql",
    "/rest",
    "/mirror",
    "/backup",
    "/old",
    "/archive",
    "/test",
    "/staff",
    "/mod",
    "/vendor",
    "/support",
    "/ticket",
    "/pgp",
    "/keys",
    "/contact",
    "/about",
    "/faq",
    "/rules",
    "/register",
    "/signup",
    "/invite",
    "/verify",
    "/wallet",
    "/withdraw",
    "/deposit",
    "/escrow",
    "/search",
    "/results",
    "/category",
    "/tag",
];

/// Discovery-relevant probes only — robots.txt, sitemap, favicon.
/// The full probe lists (ESSENTIAL_PROBES, MISCONFIG_PROBES, DARKWEB_PATHS)
/// are kept above for future use.
const DISCOVERY_PROBES: &[(&str, &str)] = &[
    ("/robots.txt", "robots_txt"),
    ("/sitemap.xml", "sitemap"),
    ("/favicon.ico", "favicon"),
];

impl InfraProber {
    /// Generate discovery probe URLs for a given base domain URL.
    pub fn generate_probes(base_url: &Url) -> Vec<ProbeResult> {
        let mut probes = Vec::new();
        for (path, probe_type) in DISCOVERY_PROBES {
            if let Ok(url) = base_url.join(path) {
                probes.push(ProbeResult {
                    probe_url: url.to_string(),
                    probe_type: probe_type.to_string(),
                });
            }
        }
        probes
    }

    /// Parse robots.txt content and extract Disallow/Allow/Sitemap URLs.
    pub fn parse_robots_txt(content: &str, base_url: &Url) -> Vec<String> {
        let mut urls = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if let Some(path) = line
                .strip_prefix("Disallow:")
                .or_else(|| line.strip_prefix("Allow:"))
            {
                let path = path.trim();
                if !path.is_empty() && path != "/" {
                    if let Ok(url) = base_url.join(path) {
                        urls.push(url.to_string());
                    }
                }
            } else if let Some(sitemap) = line.strip_prefix("Sitemap:") {
                let sitemap = sitemap.trim();
                if !sitemap.is_empty() {
                    urls.push(sitemap.to_string());
                }
            }
        }

        urls
    }

    /// Parse sitemap.xml content and extract <loc> URLs.
    pub fn parse_sitemap(content: &str) -> Vec<String> {
        let mut urls = Vec::new();
        // Simple regex extraction - works for both sitemap and sitemap index
        let loc_re = regex::Regex::new(r"<loc>\s*([^<]+)\s*</loc>").unwrap();
        for cap in loc_re.captures_iter(content) {
            urls.push(cap[1].trim().to_string());
        }
        urls
    }
}
