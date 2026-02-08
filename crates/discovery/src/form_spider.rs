use scraper::{Html, Selector};
use url::Url;

/// Find search forms on pages and generate submission URLs.
pub struct FormSpider;

#[derive(Debug, Clone)]
pub struct FormTarget {
    pub action_url: String,
    pub method: String,
    pub search_param: String,
}

/// Queries to submit to discovered search forms.
const SEARCH_QUERIES: &[&str] = &["", "a", "e", "test", "admin", "market"];

impl FormSpider {
    /// Find search forms in HTML and return their action URLs with query parameters.
    pub fn find_search_forms(html: &str, base_url: &Url) -> Vec<FormTarget> {
        let document = Html::parse_document(html);
        let mut forms = Vec::new();

        let form_sel = Selector::parse("form").unwrap();
        let input_sel = Selector::parse("input").unwrap();

        for form in document.select(&form_sel) {
            let action = form.value().attr("action").unwrap_or("");
            let method = form.value().attr("method").unwrap_or("get").to_lowercase();
            let role = form.value().attr("role").unwrap_or("");

            // Only interested in GET search forms
            if method != "get" && role != "search" {
                continue;
            }

            // Find text/search input fields
            let mut search_param = None;
            let mut has_password = false;

            for input in form.select(&input_sel) {
                let input_type = input.value().attr("type").unwrap_or("text");
                let name = input.value().attr("name").unwrap_or("");

                if input_type == "password" {
                    has_password = true;
                    break;
                }

                if (input_type == "text" || input_type == "search") && !name.is_empty() {
                    search_param = Some(name.to_string());
                }
            }

            // Skip login forms
            if has_password {
                continue;
            }

            if let Some(param) = search_param {
                let action_url = if action.is_empty() {
                    base_url.to_string()
                } else if let Ok(resolved) = base_url.join(action) {
                    resolved.to_string()
                } else {
                    continue;
                };

                forms.push(FormTarget {
                    action_url,
                    method,
                    search_param: param,
                });
            }
        }

        forms
    }

    /// Generate URLs to fetch by submitting search queries to discovered forms.
    pub fn generate_search_urls(forms: &[FormTarget]) -> Vec<String> {
        let mut urls = Vec::new();

        for form in forms {
            for query in SEARCH_QUERIES {
                if form.method == "get" {
                    if let Ok(mut url) = Url::parse(&form.action_url) {
                        url.query_pairs_mut().append_pair(&form.search_param, query);
                        urls.push(url.to_string());
                    }
                }
            }
        }

        urls
    }
}
