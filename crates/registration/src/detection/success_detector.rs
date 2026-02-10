use crate::core::types::*;
use tracing::{debug, info};

pub struct SuccessDetector {
    positive_signals: Vec<Box<dyn SuccessSignal>>,
    negative_signals: Vec<Box<dyn FailureSignal>>,
    threshold: f32,
}

impl Default for SuccessDetector {
    fn default() -> Self {
        Self {
            positive_signals: vec![
                Box::new(UrlChangeSignal),
                Box::new(WelcomeMessageSignal),
                Box::new(CookieSignal),
                Box::new(SuccessKeywordSignal),
            ],
            negative_signals: vec![
                Box::new(ErrorMessageSignal),
                Box::new(FormStillPresentSignal),
                Box::new(ValidationErrorSignal),
            ],
            threshold: 0.2,  // Lower threshold for dark web sites (was 0.5)
        }
    }
}

impl SuccessDetector {
    pub fn detect_result(&self, before: &PageState, after: &PageState) -> RegistrationResult {
        let mut positive_score = 0.0;
        let mut negative_score = 0.0;
        let mut evidence = Vec::new();

        // Check positive signals
        for signal in &self.positive_signals {
            if let Some(score) = signal.detect(before, after) {
                positive_score += score;
                evidence.push(ResultEvidence {
                    evidence_type: format!("positive_{}", signal.name()),
                    confidence: score,
                    details: format!("Success indicator: {}", signal.name()),
                });
                info!("Positive signal '{}': {:.2}", signal.name(), score);
            }
        }

        // Check negative signals
        for signal in &self.negative_signals {
            if let Some(score) = signal.detect(before, after) {
                negative_score += score;
                evidence.push(ResultEvidence {
                    evidence_type: format!("negative_{}", signal.name()),
                    confidence: score,
                    details: format!("Failure indicator: {}", signal.name()),
                });
                info!("Negative signal '{}': {:.2}", signal.name(), score);
            }
        }

        // Calculate final score
        let final_score = (positive_score - negative_score).max(0.0).min(1.0);

        // If we have NO negative signals and at least SOME positive signal, consider it success
        // This is important for dark web sites with minimal feedback
        let success = if negative_score == 0.0 && positive_score > 0.0 {
            true
        } else {
            final_score >= self.threshold
        };

        info!(
            "Registration result: positive={:.2}, negative={:.2}, final={:.2}, success={} (threshold={})",
            positive_score, negative_score, final_score, success, self.threshold
        );

        // Check for email verification requirement
        let requires_email_verification = after.html.to_lowercase().contains("verify") &&
                                         after.html.to_lowercase().contains("email");

        RegistrationResult {
            success,
            account_info: if success {
                Some(AccountInfo {
                    username: String::new(),  // Will be filled by caller
                    email: None,
                    user_id: None,
                    profile_url: None,
                })
            } else {
                None
            },
            error: if !success {
                Some(Self::extract_error_message(&after.html))
            } else {
                None
            },
            requires_email_verification,
            session_cookies: None,  // Will be filled by caller
            evidence,
        }
    }

    fn extract_error_message(html: &str) -> String {
        let html_lower = html.to_lowercase();

        // Common error patterns
        if html_lower.contains("username") && (html_lower.contains("taken") || html_lower.contains("exists") || html_lower.contains("already")) {
            return "Username already taken".to_string();
        }

        if html_lower.contains("email") && (html_lower.contains("taken") || html_lower.contains("exists") || html_lower.contains("already")) {
            return "Email already registered".to_string();
        }

        if html_lower.contains("password") && (html_lower.contains("weak") || html_lower.contains("strong") || html_lower.contains("requirement")) {
            return "Password does not meet requirements".to_string();
        }

        if html_lower.contains("captcha") && (html_lower.contains("invalid") || html_lower.contains("incorrect")) {
            return "CAPTCHA verification failed".to_string();
        }

        if html_lower.contains("invalid") && html_lower.contains("login") {
            return "Invalid login (likely filled login form instead of registration)".to_string();
        }

        "Unknown error during registration".to_string()
    }
}

// Success signals

struct UrlChangeSignal;

impl SuccessSignal for UrlChangeSignal {
    fn name(&self) -> &str {
        "url_change"
    }

    fn detect(&self, before: &PageState, after: &PageState) -> Option<f32> {
        if before.url == after.url {
            return None;
        }

        let after_lower = after.url.to_lowercase();

        // Check for success-indicating URLs
        if after_lower.contains("/welcome") || after_lower.contains("/dashboard") ||
           after_lower.contains("/profile") || after_lower.contains("/home") ||
           after_lower.contains("/verify") {
            Some(0.8)
        } else {
            Some(0.3)  // URL changed, but not sure what it means
        }
    }
}

struct WelcomeMessageSignal;

impl SuccessSignal for WelcomeMessageSignal {
    fn name(&self) -> &str {
        "welcome_message"
    }

    fn detect(&self, _before: &PageState, after: &PageState) -> Option<f32> {
        let html_lower = after.html.to_lowercase();

        if html_lower.contains("welcome") || html_lower.contains("successfully registered") ||
           html_lower.contains("account created") {
            Some(0.9)
        } else {
            None
        }
    }
}

struct CookieSignal;

impl SuccessSignal for CookieSignal {
    fn name(&self) -> &str {
        "session_cookie"
    }

    fn detect(&self, before: &PageState, after: &PageState) -> Option<f32> {
        // Check if new session cookies were set
        if after.cookies.len() > before.cookies.len() {
            // Look for common session cookie names
            for cookie in &after.cookies {
                let cookie_lower = cookie.to_lowercase();
                if cookie_lower.contains("session") || cookie_lower.contains("auth") ||
                   cookie_lower.contains("token") || cookie_lower.contains("sid") {
                    return Some(0.7);
                }
            }
            Some(0.4)  // New cookies, but not obviously session-related
        } else {
            None
        }
    }
}

struct SuccessKeywordSignal;

impl SuccessSignal for SuccessKeywordSignal {
    fn name(&self) -> &str {
        "success_keywords"
    }

    fn detect(&self, _before: &PageState, after: &PageState) -> Option<f32> {
        let html_lower = after.html.to_lowercase();

        let success_keywords = [
            "success", "successful", "congratulations", "confirmed",
            "activated", "registered", "created", "welcome", "thank you",
            "check your email", "verify", "logged in",
            "account created", "you may now login", "registration complete",
        ];

        let error_keywords = [
            "invalid login", "incorrect password", "authentication failed",
            "login failed", "access denied"
        ];

        let has_success = success_keywords.iter().any(|k| html_lower.contains(k));
        let has_error = error_keywords.iter().any(|k| html_lower.contains(k));

        if has_success && !has_error {
            Some(0.6)
        } else if has_success && has_error {
            Some(0.2)  // Mixed signals
        } else if !has_error {
            // If no error keywords at all, give small positive signal
            Some(0.1)
        } else {
            None
        }
    }
}

// Failure signals

struct ErrorMessageSignal;

impl FailureSignal for ErrorMessageSignal {
    fn name(&self) -> &str {
        "error_message"
    }

    fn detect(&self, _before: &PageState, after: &PageState) -> Option<f32> {
        let html_lower = after.html.to_lowercase();

        // Only detect VERY specific registration errors
        // Dark web sites often have "error 404" in footers, etc.
        let registration_errors =
            (html_lower.contains("invalid") && html_lower.contains("login")) ||
            (html_lower.contains("incorrect") && html_lower.contains("password")) ||
            (html_lower.contains("username") && html_lower.contains("already") && html_lower.contains("taken")) ||
            (html_lower.contains("email") && html_lower.contains("already") && html_lower.contains("exist")) ||
            html_lower.contains("registration failed") ||
            html_lower.contains("signup failed");

        if registration_errors {
            Some(0.5)  // Reduced from 0.8
        } else {
            None  // Don't penalize for generic "error" text
        }
    }
}

struct FormStillPresentSignal;

impl FailureSignal for FormStillPresentSignal {
    fn name(&self) -> &str {
        "form_still_present"
    }

    fn detect(&self, before: &PageState, after: &PageState) -> Option<f32> {
        // Don't use this signal - too many false positives on dark web sites
        // Many sites keep forms visible after registration
        None
    }
}

struct ValidationErrorSignal;

impl FailureSignal for ValidationErrorSignal {
    fn name(&self) -> &str {
        "validation_error"
    }

    fn detect(&self, _before: &PageState, after: &PageState) -> Option<f32> {
        let html_lower = after.html.to_lowercase();

        // Only detect SPECIFIC validation errors in context
        let validation_errors =
            (html_lower.contains("username") && html_lower.contains("required")) ||
            (html_lower.contains("password") && html_lower.contains("required")) ||
            (html_lower.contains("email") && html_lower.contains("required")) ||
            (html_lower.contains("field") && html_lower.contains("required"));

        if validation_errors {
            Some(0.4)  // Reduced from 0.7
        } else {
            None
        }
    }
}
