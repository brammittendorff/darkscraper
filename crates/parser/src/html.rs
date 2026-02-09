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
    pub has_register_form: bool,
    pub has_captcha: bool,
    pub requires_email: bool,
    pub is_forum: bool,
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
    let has_register_form = detect_register_form(&document);
    let has_captcha = detect_captcha(&document);
    let requires_email = detect_email_requirement(&document);
    let is_forum = detect_forum(&document, &body_text);
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
        has_register_form,
        has_captcha,
        requires_email,
        is_forum,
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

            // Skip non-crawlable URL schemes
            if href.starts_with("javascript:") ||
               href.starts_with("mailto:") ||
               href.starts_with("tel:") ||
               href.starts_with("data:") ||
               href.starts_with("#") ||
               href == "/" {
                return None;
            }

            // Special handling for Hyphanet links (before url.join)
            // Hyphanet base URLs use hyphanet: scheme which doesn't support standard joining
            if (base_url.scheme() == "hyphanet" || base_url.scheme() == "freenet") &&
               (href.starts_with("/USK@") || href.starts_with("/SSK@") || href.starts_with("/CHK@") ||
                href.starts_with("/freenet:") || href.starts_with("/hyphanet:")) {
                let key = if href.starts_with("/freenet:") {
                    href.strip_prefix("/freenet:").unwrap()
                } else if href.starts_with("/hyphanet:") {
                    href.strip_prefix("/hyphanet:").unwrap()
                } else {
                    href.strip_prefix("/").unwrap()
                };
                return Some(ExtractedLink {
                    url: format!("hyphanet:{}", key),
                    anchor_text: {
                        let t = el.text().collect::<String>().trim().to_string();
                        if t.is_empty() { None } else { Some(t) }
                    },
                    is_onion: false,
                    is_i2p: false,
                    is_zeronet: false,
                    is_hyphanet: true,
                    is_lokinet: false,
                    is_external: true,
                });
            }

            let resolved = base_url.join(href).ok()?;
            let host = resolved.host_str().unwrap_or("");

            // Convert FProxy gateway URLs back to hyphanet: scheme
            // e.g., http://hyphanet1:8888/USK@.../site/0/ -> hyphanet:USK@.../site/0/
            // Also handle /freenet:USK@ and /hyphanet:USK@ formats from FProxy
            let (final_url, is_hyphanet_link) = if resolved.scheme() == "http" || resolved.scheme() == "https" {
                let path = resolved.path();
                if path.starts_with("/freenet:") || path.starts_with("/hyphanet:") {
                    // FProxy format: /freenet:USK@... or /hyphanet:USK@...
                    let key = path.trim_start_matches("/freenet:").trim_start_matches("/hyphanet:");
                    (format!("hyphanet:{}", key), true)
                } else if path.starts_with("/USK@") || path.starts_with("/SSK@") || path.starts_with("/CHK@") {
                    // This is a Hyphanet key accessed via FProxy gateway
                    (format!("hyphanet:{}", path), true)
                } else {
                    (resolved.to_string(), false)
                }
            } else {
                (resolved.to_string(), resolved.scheme() == "hyphanet" || resolved.scheme() == "freenet")
            };

            Some(ExtractedLink {
                url: final_url,
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
                is_hyphanet: is_hyphanet_link,
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

fn detect_register_form(document: &Html) -> bool {
    // MULTILINGUAL DETECTION - supports English, Russian, German, French, Spanish, Chinese, Japanese, etc.

    // 1. Multiple password fields (password + confirm password)
    if let Some(sel) = selector("input[type='password']") {
        let password_fields: Vec<_> = document.select(&sel).collect();
        if password_fields.len() >= 2 {
            return true;
        }
    }

    // 2. Registration keywords in form action or surrounding text (multilingual)
    if let Some(sel) = selector("form") {
        for form in document.select(&sel) {
            // Check form action
            if let Some(action) = form.value().attr("action") {
                // Multilingual check for common keywords
                let action_text = action.to_lowercase();
                if is_registration_keyword(&action_text) {
                    return true;
                }
            }

            // Check form text content (multilingual)
            let form_text = form.text().collect::<String>();
            if is_registration_keyword(&form_text) {
                return true;
            }

            // Check for email + username + password combo (common registration pattern)
            let has_email = form.select(&selector("input[type='email']").unwrap()).next().is_some() ||
                           form.select(&selector("input[name*='email']").unwrap()).next().is_some() ||
                           form.select(&selector("input[name*='mail']").unwrap()).next().is_some() ||
                           form.select(&selector("input[name*='почта']").unwrap()).next().is_some();

            let has_username = form.select(&selector("input[name*='user']").unwrap()).next().is_some() ||
                              form.select(&selector("input[name*='name']").unwrap()).next().is_some() ||
                              form.select(&selector("input[name*='логин']").unwrap()).next().is_some();

            let has_password = form.select(&selector("input[type='password']").unwrap()).next().is_some();

            if (has_email || has_username) && has_password {
                // Check if form has password confirmation (strong indicator of registration)
                let password_fields: Vec<_> = form.select(&selector("input[type='password']").unwrap()).collect();
                if password_fields.len() >= 2 {
                    return true;
                }
            }
        }
    }

    // 3. Check for registration buttons (multilingual)
    if let Some(sel) = selector("button, input[type='submit']") {
        for button in document.select(&sel) {
            let button_text = button.text().collect::<String>();
            let button_value = button.value().attr("value").unwrap_or("");

            if is_registration_keyword(&button_text) || is_registration_keyword(button_value) {
                return true;
            }
        }
    }

    false
}

/// Check if text contains registration keywords in multiple languages
fn is_registration_keyword(text: &str) -> bool {
    let text_lower = text.to_lowercase();

    // English
    if text_lower.contains("register") || text_lower.contains("sign up") ||
       text_lower.contains("signup") || text_lower.contains("create account") ||
       text_lower.contains("join") {
        return true;
    }

    // Russian (Cyrillic)
    if text_lower.contains("регистрация") || text_lower.contains("регистрироваться") ||
       text_lower.contains("зарегистрироваться") || text_lower.contains("создать аккаунт") {
        return true;
    }

    // German
    if text_lower.contains("registrieren") || text_lower.contains("registrierung") ||
       text_lower.contains("konto erstellen") {
        return true;
    }

    // French
    if text_lower.contains("inscription") || text_lower.contains("s'inscrire") ||
       text_lower.contains("créer un compte") {
        return true;
    }

    // Spanish
    if text_lower.contains("registrarse") || text_lower.contains("registro") ||
       text_lower.contains("crear cuenta") {
        return true;
    }

    // Chinese
    if text_lower.contains("注册") || text_lower.contains("註冊") ||
       text_lower.contains("创建账户") {
        return true;
    }

    // Japanese
    if text_lower.contains("登録") || text_lower.contains("新規登録") ||
       text_lower.contains("アカウント作成") {
        return true;
    }

    // Italian
    if text_lower.contains("registrazione") || text_lower.contains("registrati") ||
       text_lower.contains("crea account") {
        return true;
    }

    // Portuguese
    if text_lower.contains("registrar") || text_lower.contains("cadastro") ||
       text_lower.contains("criar conta") {
        return true;
    }

    false
}

fn detect_captcha(document: &Html) -> bool {
    let body_html = document.html().to_lowercase();

    // Common CAPTCHA indicators
    let captcha_patterns = vec![
        // Google reCAPTCHA
        "g-recaptcha", "recaptcha", "grecaptcha",
        // hCaptcha
        "h-captcha", "hcaptcha",
        // Image/text CAPTCHAs
        "captcha", "verification", "challenge",
        // Cloudflare Turnstile
        "cf-turnstile", "turnstile",
        // DDoS protection pages
        "ddos/captcha", "ddos_captcha", "ddos-captcha",
        "/captcha/", "captcha.png", "captcha.jpg",
        // Verification challenges
        "verify you are human", "prove you're human",
    ];

    for pattern in captcha_patterns {
        if body_html.contains(pattern) {
            return true;
        }
    }

    // Check for CAPTCHA-related images
    if let Some(sel) = selector("img[src*='captcha'], img[alt*='captcha'], img[class*='captcha']") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }

    // Check for CAPTCHA script tags
    if let Some(sel) = selector("script[src*='recaptcha'], script[src*='hcaptcha'], script[src*='captcha']") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }

    // Check for divs/forms with CAPTCHA classes
    if let Some(sel) = selector("div[class*='captcha'], div[id*='captcha'], form[action*='captcha'], form[action*='verify']") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }

    // Check title for DDoS/CAPTCHA keywords
    if body_html.contains("<title>") {
        if body_html.contains("ddos") || body_html.contains("protection") || body_html.contains("captcha") {
            return true;
        }
    }

    false
}

fn detect_email_requirement(document: &Html) -> bool {
    // Check for required email fields in forms
    if let Some(sel) = selector("input[type='email'][required], input[name*='email'][required]") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }

    // Check for email fields with asterisk or "required" text nearby
    if let Some(sel) = selector("input[type='email'], input[name*='email']") {
        for email_input in document.select(&sel) {
            // Check if HTML5 required attribute is present
            if email_input.value().attr("required").is_some() {
                return true;
            }

            // Check for asterisk in label or nearby text (common pattern)
            // Note: parent node doesn't have .html() method, so we check siblings and form context instead
            // This is checked in the form-level validation below
        }
    }

    // Check form text for email requirements
    if let Some(sel) = selector("form") {
        for form in document.select(&sel) {
            let form_text = form.text().collect::<String>().to_lowercase();
            if (form_text.contains("email") && form_text.contains("required")) ||
               (form_text.contains("e-mail") && form_text.contains("*")) {
                return true;
            }
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

fn detect_forum(document: &Html, body_text: &str) -> bool {
    let body_lower = body_text.to_lowercase();
    let _html = document.html().to_lowercase();

    // Forum keywords in title
    if let Some(title_sel) = selector("title") {
        if let Some(title_elem) = document.select(&title_sel).next() {
            let title_text = title_elem.text().collect::<String>().to_lowercase();
            if title_text.contains("forum") ||
               title_text.contains("board") ||
               title_text.contains("discussion") ||
               title_text.contains("community") {
                return true;
            }
        }
    }

    // Forum software signatures
    let forum_signatures = vec![
        // English
        "forum", "board", "discussion", "community",
        "topics", "threads", "posts", "replies",
        "subforum", "category",
        // Russian
        "форум", "обсуждение", "сообщество",
        // German  
        "foren", "diskussion",
        // French
        "forum", "discussion", "communauté",
        // Software
        "phpbb", "vbulletin", "mybb", "smf",
        "discourse", "flarum", "nodebb",
    ];

    for sig in forum_signatures {
        if body_lower.contains(sig) && 
           (body_lower.contains("topic") || 
            body_lower.contains("post") || 
            body_lower.contains("thread")) {
            return true;
        }
    }

    // Forum-specific HTML structures
    if let Some(sel) = selector(".forum, .board, .topic, .thread, #forum, #board") {
        if document.select(&sel).next().is_some() {
            return true;
        }
    }

    // Check for forum navigation (threads, members, search)
    let nav_text = if let Some(nav_sel) = selector("nav, .navigation, #menu") {
        document.select(&nav_sel)
            .map(|el| el.text().collect::<String>())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    } else {
        String::new()
    };

    if (nav_text.contains("threads") || nav_text.contains("topics")) &&
       (nav_text.contains("members") || nav_text.contains("users")) {
        return true;
    }

    false
}
