use scraper::{Html, Selector};
use crate::core::types::SubmitButton;
use crate::multilingual::MultilingualDetector;

pub struct SubmitDetector;

impl SubmitDetector {
    /// Find the submit button for a form
    pub fn find_submit_button(form_html: &str, _form_selector: &str) -> Option<SubmitButton> {
        let document = Html::parse_document(form_html);
        let mut candidates: Vec<(String, String, f32)> = Vec::new();

        // Strategy 1: input[type="submit"]
        if let Ok(selector) = Selector::parse("input[type='submit']") {
            for elem in document.select(&selector) {
                let value = elem.value().attr("value").unwrap_or("Submit");
                let selector_str = Self::build_selector(&elem);
                candidates.push((selector_str, value.to_string(), 0.9));
            }
        }

        // Strategy 2: button[type="submit"]
        if let Ok(selector) = Selector::parse("button[type='submit']") {
            for elem in document.select(&selector) {
                let text = elem.text().collect::<String>().trim().to_string();
                let selector_str = Self::build_selector(&elem);
                candidates.push((selector_str, text, 0.9));
            }
        }

        // Strategy 3: button without explicit type (defaults to submit in forms)
        if let Ok(selector) = Selector::parse("button") {
            for elem in document.select(&selector) {
                if elem.value().attr("type").is_none() || elem.value().attr("type") == Some("submit") {
                    let text = elem.text().collect::<String>().trim().to_string();
                    let selector_str = Self::build_selector(&elem);

                    // Check if text suggests registration
                    let confidence = if Self::is_registration_button_text(&text) {
                        0.8
                    } else {
                        0.5
                    };

                    candidates.push((selector_str, text, confidence));
                }
            }
        }

        // Strategy 4: Links that act as submit buttons (onclick handlers)
        if let Ok(selector) = Selector::parse("a[onclick]") {
            for elem in document.select(&selector) {
                let onclick = elem.value().attr("onclick").unwrap_or("");
                if onclick.contains("submit") {
                    let text = elem.text().collect::<String>().trim().to_string();
                    let selector_str = Self::build_selector(&elem);
                    candidates.push((selector_str, text, 0.6));
                }
            }
        }

        // Sort by confidence and return best match
        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        candidates.first().map(|(selector, text, confidence)| {
            SubmitButton {
                selector: selector.clone(),
                text: text.clone(),
                confidence: *confidence,
            }
        })
    }

    /// Check if button text suggests registration
    fn is_registration_button_text(text: &str) -> bool {
        let text_lower = text.to_lowercase();

        // English
        if text_lower.contains("register") || text_lower.contains("sign up") ||
           text_lower.contains("create") || text_lower.contains("join") {
            return true;
        }

        // Use multilingual detector
        MultilingualDetector::is_registration_text(text)
    }

    /// Build CSS selector for element
    fn build_selector(element: &scraper::ElementRef) -> String {
        if let Some(id) = element.value().attr("id") {
            return format!("#{}", id);
        }

        if let Some(name) = element.value().attr("name") {
            let tag = element.value().name();
            return format!("{}[name='{}']", tag, name);
        }

        if let Some(class) = element.value().attr("class") {
            let first_class: &str = class.split_whitespace().next().unwrap_or("");
            if !first_class.is_empty() {
                let tag = element.value().name();
                return format!("{}.{}", tag, first_class);
            }
        }

        element.value().name().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_submit_button() {
        let html = r#"
            <form>
                <input type="text" name="username" />
                <button type="submit" id="submit-btn">Register</button>
            </form>
        "#;

        let button = SubmitDetector::find_submit_button(html, "form");
        assert!(button.is_some());

        let button = button.unwrap();
        assert_eq!(button.selector, "#submit-btn");
        assert!(button.confidence > 0.7);
    }
}
