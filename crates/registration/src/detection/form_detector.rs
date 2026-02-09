use scraper::{Html, Selector};
use tracing::{info, debug};
use crate::core::types::*;
use crate::multilingual::MultilingualDetector;

pub struct FormDetector {
    keyword_weight: f32,
    structure_weight: f32,
    semantic_weight: f32,
    threshold: f32,
}

impl Default for FormDetector {
    fn default() -> Self {
        Self {
            keyword_weight: 1.5,  // Increase keyword weight for compatibility
            structure_weight: 1.0,
            semantic_weight: 0.8,
            threshold: 0.3,  // Lower threshold to match old parser behavior
        }
    }
}

impl FormDetector {
    pub fn new(keyword_weight: f32, structure_weight: f32, semantic_weight: f32, threshold: f32) -> Self {
        Self {
            keyword_weight,
            structure_weight,
            semantic_weight,
            threshold,
        }
    }

    /// Detect registration forms with multi-signal approach
    pub fn detect_forms(&self, html: &str, url: &str) -> Vec<FormDetectionResult> {
        let document = Html::parse_document(html);
        let form_selector = Selector::parse("form").unwrap();

        let mut results = Vec::new();

        for form_elem in document.select(&form_selector) {
            let form_html = form_elem.html();

            // Analyze form with multiple signals
            let keyword_score = self.analyze_keywords(&form_html);
            let structure_score = self.analyze_structure(&form_elem);
            let semantic_score = self.analyze_semantics(html, url);

            // Calculate weighted confidence
            let total_weight = self.keyword_weight + self.structure_weight + self.semantic_weight;
            let confidence = (
                keyword_score * self.keyword_weight +
                structure_score * self.structure_weight +
                semantic_score * self.semantic_weight
            ) / total_weight;

            debug!(
                "Form analysis: keyword={:.2}, structure={:.2}, semantic={:.2}, confidence={:.2}",
                keyword_score, structure_score, semantic_score, confidence
            );

            if confidence >= self.threshold {
                let form_type = self.determine_form_type(&form_html);

                results.push(FormDetectionResult {
                    form_selector: self.get_form_selector(&form_elem),
                    confidence,
                    signals: vec![
                        DetectionSignal {
                            signal_type: "keyword".to_string(),
                            confidence: keyword_score,
                            details: "Registration keywords detected".to_string(),
                        },
                        DetectionSignal {
                            signal_type: "structure".to_string(),
                            confidence: structure_score,
                            details: "Form structure matches registration pattern".to_string(),
                        },
                        DetectionSignal {
                            signal_type: "semantic".to_string(),
                            confidence: semantic_score,
                            details: "Page context suggests registration".to_string(),
                        },
                    ],
                    form_type,
                });
            }
        }

        // Sort by confidence
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        if !results.is_empty() {
            info!("Detected {} potential registration form(s)", results.len());
        }

        results
    }

    /// Analyze keywords in form HTML (multilingual)
    fn analyze_keywords(&self, form_html: &str) -> f32 {
        let form_lower = form_html.to_lowercase();

        // Use existing multilingual detector
        if MultilingualDetector::is_registration_text(&form_html) {
            return 1.0;
        }

        // Additional keyword analysis
        let registration_keywords = [
            "register", "sign up", "signup", "create account", "join",
            "new account", "registration", "member", "get started",
        ];

        let login_keywords = ["login", "log in", "sign in", "signin"];

        let reg_count = registration_keywords.iter()
            .filter(|k| form_lower.contains(*k))
            .count();

        let login_count = login_keywords.iter()
            .filter(|k| form_lower.contains(*k))
            .count();

        // More registration keywords = higher score
        // Presence of login keywords reduces score
        let score = (reg_count as f32 * 0.3) - (login_count as f32 * 0.2);
        score.max(0.0).min(1.0)
    }

    /// Analyze form structure
    fn analyze_structure(&self, form_elem: &scraper::ElementRef) -> f32 {
        let input_selector = Selector::parse("input").unwrap();
        let inputs: Vec<_> = form_elem.select(&input_selector).collect();

        let mut has_password = false;
        let mut has_email_or_username = false;
        let mut password_count = 0;
        let mut text_count = 0;
        let mut submit_count = 0;
        let mut score: f32 = 0.0;

        for input in inputs {
            if let Some(input_type) = input.value().attr("type") {
                match input_type {
                    "password" => {
                        has_password = true;
                        password_count += 1;
                    }
                    "email" => has_email_or_username = true,
                    "text" => text_count += 1,
                    "submit" => submit_count += 1,
                    _ => {}
                }
            }

            // Check names for username/email fields
            if let Some(name) = input.value().attr("name") {
                let name_lower = name.to_lowercase();
                if name_lower.contains("user") || name_lower.contains("email") || name_lower.contains("login") {
                    has_email_or_username = true;
                }
            }
        }

        // Registration forms typically have:
        // - 1-2 password fields (registration often has confirmation)
        // - Email or username field
        // - Submit button
        // - Multiple text fields

        if has_password {
            score += 0.4;
        }

        if password_count >= 2 {
            // Two password fields suggests registration (password confirmation)
            score += 0.3;
        }

        if has_email_or_username {
            score += 0.3;
        }

        if text_count >= 2 {
            // Multiple text fields suggest registration
            score += 0.2;
        }

        if submit_count > 0 {
            score += 0.1;
        }

        score.min(1.0_f32)
    }

    /// Analyze semantic context (page title, headings, URL)
    fn analyze_semantics(&self, html: &str, url: &str) -> f32 {
        let document = Html::parse_document(html);
        let mut score: f32 = 0.0;

        // Check page title
        if let Ok(title_selector) = Selector::parse("title") {
            if let Some(title) = document.select(&title_selector).next() {
                let title_text = title.text().collect::<String>().to_lowercase();
                if MultilingualDetector::is_registration_text(&title_text) {
                    score += 0.4;
                }
            }
        }

        // Check H1/H2 headings
        for tag in ["h1", "h2"] {
            if let Ok(selector) = Selector::parse(tag) {
                for heading in document.select(&selector) {
                    let text = heading.text().collect::<String>();
                    if MultilingualDetector::is_registration_text(&text) {
                        score += 0.3;
                        break;
                    }
                }
            }
        }

        // Check URL path
        let url_lower = url.to_lowercase();
        if url_lower.contains("register") || url_lower.contains("signup") || url_lower.contains("join") {
            score += 0.3;
        }

        score.min(1.0_f32)
    }

    /// Determine if form is registration, login, or mixed
    fn determine_form_type(&self, form_html: &str) -> FormType {
        let form_lower = form_html.to_lowercase();

        let is_registration = MultilingualDetector::is_registration_text(&form_html);
        let is_login = MultilingualDetector::is_login_text(&form_html);

        // Count password fields - registrations usually have 2 (confirmation)
        let password_count = form_lower.matches("type=\"password\"").count() +
                            form_lower.matches("type='password'").count();

        if is_registration && is_login {
            FormType::Combined
        } else if is_registration || password_count >= 2 {
            FormType::Registration
        } else if is_login {
            FormType::Login
        } else {
            FormType::Unknown
        }
    }

    /// Get CSS selector for form element
    fn get_form_selector(&self, form_elem: &scraper::ElementRef) -> String {
        // Try ID first
        if let Some(id) = form_elem.value().attr("id") {
            return format!("#{}", id);
        }

        // Try name
        if let Some(name) = form_elem.value().attr("name") {
            return format!("form[name='{}']", name);
        }

        // Try class
        if let Some(class) = form_elem.value().attr("class") {
            let first_class = class.split_whitespace().next().unwrap_or("");
            if !first_class.is_empty() {
                return format!("form.{}", first_class);
            }
        }

        // Fallback to form tag
        "form".to_string()
    }
}

#[derive(Debug, Clone)]
pub struct FormDetectionResult {
    pub form_selector: String,
    pub confidence: f32,
    pub signals: Vec<DetectionSignal>,
    pub form_type: FormType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_registration_form() {
        let html = r#"
            <form id="registration-form">
                <input type="text" name="username" />
                <input type="email" name="email" />
                <input type="password" name="password" />
                <input type="password" name="password_confirm" />
                <button type="submit">Register</button>
            </form>
        "#;

        let detector = FormDetector::default();
        let results = detector.detect_forms(html, "http://example.com/register");

        assert!(!results.is_empty());
        assert!(results[0].confidence > 0.6);
        assert_eq!(results[0].form_type, FormType::Registration);
    }

    #[test]
    fn test_distinguish_login_from_registration() {
        let login_html = r#"
            <form>
                <input type="text" name="username" />
                <input type="password" name="password" />
                <button type="submit">Log In</button>
            </form>
        "#;

        let detector = FormDetector::default();
        let results = detector.detect_forms(login_html, "http://example.com/login");

        if !results.is_empty() {
            // Login form should have lower confidence or be classified as login
            assert_eq!(results[0].form_type, FormType::Login);
        }
    }
}
