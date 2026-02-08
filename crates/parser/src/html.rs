use std::collections::HashMap;

use scraper::{Html, Selector};
use url::Url;

use darkscraper_core::ExtractedLink;

pub struct HtmlResult {
    pub title: Option<String>,
    pub h1: Vec<String>,
    pub h2: Vec<String>,
    pub h3: Vec<String>,
    pub body_text: String,
    pub links: Vec<ExtractedLink>,
    pub meta_description: Option<String>,
    pub meta_keywords: Vec<String>,
    pub language: Option<String>,
    pub has_login_form: bool,
    pub has_search_form: bool,
    pub open_graph: HashMap<String, String>,
}

pub fn parse_html(html_str: &str, base_url: &Url) -> HtmlResult {
    let document = Html::parse_document(html_str);
    let base_domain = base_url.host_str().unwrap_or("");

    // Title
    let title = selector("title")
        .and_then(|s| document.select(&s).next())
        .map(|el| el.text().collect::<String>().trim().to_string());

    // Headings
    let h1 = extract_text_by_selector(&document, "h1");
    let h2 = extract_text_by_selector(&document, "h2");
    let h3 = extract_text_by_selector(&document, "h3");

    // Body text - get all visible text
    let body_text = selector("body")
        .and_then(|s| document.select(&s).next())
        .map(|el| {
            el.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();

    // Links
    let links = extract_links(&document, base_url, base_domain);

    // Meta tags
    let meta_description = extract_meta_content(&document, "description");
    let meta_keywords = extract_meta_content(&document, "keywords")
        .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Language
    let language = selector("html")
        .and_then(|s| document.select(&s).next())
        .and_then(|el| el.value().attr("lang").map(|s| s.to_string()));

    // Open Graph
    let mut open_graph = HashMap::new();
    if let Some(sel) = selector("meta[property^='og:']") {
        for el in document.select(&sel) {
            if let (Some(prop), Some(content)) =
                (el.value().attr("property"), el.value().attr("content"))
            {
                open_graph.insert(prop.to_string(), content.to_string());
            }
        }
    }

    // Form detection
    let has_login_form = detect_login_form(&document);
    let has_search_form = detect_search_form(&document);

    HtmlResult {
        title,
        h1,
        h2,
        h3,
        body_text,
        links,
        meta_description,
        meta_keywords,
        language,
        has_login_form,
        has_search_form,
        open_graph,
    }
}

fn selector(s: &str) -> Option<Selector> {
    Selector::parse(s).ok()
}

fn extract_text_by_selector(document: &Html, sel: &str) -> Vec<String> {
    selector(sel)
        .map(|s| {
            document
                .select(&s)
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn extract_links(document: &Html, base_url: &Url, base_domain: &str) -> Vec<ExtractedLink> {
    let Some(sel) = selector("a[href]") else {
        return vec![];
    };

    document
        .select(&sel)
        .filter_map(|el| {
            let href = el.value().attr("href")?;
            let resolved = base_url.join(href).ok()?;
            let host = resolved.host_str().unwrap_or("");

            Some(ExtractedLink {
                url: resolved.to_string(),
                anchor_text: {
                    let t = el.text().collect::<String>().trim().to_string();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                },
                is_onion: host.ends_with(".onion"),
                is_i2p: host.ends_with(".i2p"),
                is_zeronet: host.ends_with(".bit"),
                is_freenet: resolved.scheme() == "freenet",
                is_lokinet: host.ends_with(".loki"),
                is_external: host != base_domain,
            })
        })
        .collect()
}

fn extract_meta_content(document: &Html, name: &str) -> Option<String> {
    let sel_str = format!(
        "meta[name='{}'], meta[name='{}']",
        name,
        name.to_uppercase()
    );
    selector(&sel_str)
        .and_then(|s| document.select(&s).next())
        .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
}

fn detect_login_form(document: &Html) -> bool {
    if let Some(sel) = selector("input[type='password']") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }
    false
}

fn detect_search_form(document: &Html) -> bool {
    if let Some(sel) = selector("input[type='search'], form[role='search']") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }
    false
}
