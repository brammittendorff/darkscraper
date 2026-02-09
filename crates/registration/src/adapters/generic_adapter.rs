use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;
use tracing::info;

use crate::core::types::*;
use crate::detection::{FormDetector, FieldClassifier, SubmitDetector, SuccessDetector};

/// Generic adapter that works for most standard forms
pub struct GenericAdapter {
    form_detector: FormDetector,
    field_classifier: FieldClassifier,
    success_detector: SuccessDetector,
}

impl Default for GenericAdapter {
    fn default() -> Self {
        Self {
            form_detector: FormDetector::default(),
            field_classifier: FieldClassifier::default(),
            success_detector: SuccessDetector::default(),
        }
    }
}

impl GenericAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SiteAdapter for GenericAdapter {
    fn name(&self) -> &str {
        "generic"
    }

    fn matches(&self, _url: &Url, _html: &str) -> f32 {
        // Generic adapter always matches as a fallback, but with low confidence
        0.5
    }

    async fn detect_form(&self, html: &str, url: &str) -> anyhow::Result<FormInfo> {
        info!("Detecting form with generic adapter (threshold: 0.3)");

        // Find registration forms
        let forms = self.form_detector.detect_forms(html, url);

        info!("Found {} potential forms", forms.len());
        for (i, form) in forms.iter().enumerate() {
            info!("  Form {}: confidence={:.2}, type={:?}", i+1, form.confidence, form.form_type);
        }

        if forms.is_empty() {
            anyhow::bail!("No registration form detected");
        }

        // Prefer Registration forms over Combined/Login forms
        // Strategy: Pick highest confidence Registration form, or fallback to highest confidence overall
        let best_form = forms.iter()
            .filter(|f| f.form_type == FormType::Registration)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .or_else(|| {
                // No pure registration form found, try Combined forms
                info!("No pure Registration form found, checking Combined forms");
                forms.iter()
                    .filter(|f| f.form_type == FormType::Combined)
                    .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            })
            .unwrap_or(&forms[0]);

        info!("Selected form type: {:?} (confidence: {:.2})", best_form.form_type, best_form.confidence);

        // Classify fields in the form
        let fields = self.field_classifier.classify_fields(html, &best_form.form_selector);

        // Find submit button
        let submit_button = SubmitDetector::find_submit_button(html, &best_form.form_selector);

        info!(
            "Found form '{}' with {} fields (confidence: {:.2})",
            best_form.form_selector,
            fields.len(),
            best_form.confidence
        );

        Ok(FormInfo {
            selector: best_form.form_selector.clone(),
            action: None,  // Will be determined at runtime
            method: "POST".to_string(),
            fields,
            submit_button,
            form_type: best_form.form_type.clone(),
            confidence: best_form.confidence,
            is_multi_step: false,  // TODO: Detect multi-step forms
            current_step: 1,
            total_steps: None,
        })
    }

    async fn fill_form(&self, form: &FormInfo, data: &RegistrationData) -> anyhow::Result<HashMap<String, String>> {
        info!("Filling form with {} fields", form.fields.len());

        let mut field_mapping = HashMap::new();

        for field in &form.fields {
            let value = match &field.field_type {
                FieldType::Username => Some(data.username.clone()),
                FieldType::Email => data.email.clone(),
                FieldType::Password => Some(data.password.clone()),
                FieldType::PasswordConfirm => {
                    data.password_confirm.clone().or_else(|| Some(data.password.clone()))
                }
                FieldType::FirstName => data.first_name.clone(),
                FieldType::LastName => data.last_name.clone(),
                FieldType::DateOfBirth => data.date_of_birth.clone(),
                FieldType::Country => data.country.clone(),
                FieldType::TermsCheckbox => Some(if data.accept_terms { "true" } else { "" }.to_string()),
                FieldType::NewsletterCheckbox => Some(if data.subscribe_newsletter { "true" } else { "" }.to_string()),
                FieldType::Captcha => data.captcha_solution.clone(),
                _ => data.custom_fields.get(&format!("{:?}", field.field_type)).cloned(),
            };

            if let Some(val) = value {
                field_mapping.insert(field.selector.clone(), val);
                info!("Mapped {:?} to selector '{}'", field.field_type, field.selector);
            }
        }

        Ok(field_mapping)
    }

    async fn submit_form(&self, form: &FormInfo) -> anyhow::Result<SubmitButton> {
        if let Some(ref submit) = form.submit_button {
            info!("Using submit button: {}", submit.selector);
            Ok(submit.clone())
        } else {
            anyhow::bail!("No submit button found for form");
        }
    }

    async fn detect_result(&self, before: &PageState, after: &PageState) -> anyhow::Result<RegistrationResult> {
        info!("Detecting registration result");

        let result = self.success_detector.detect_result(before, after);

        info!(
            "Registration result: success={}, evidence_count={}",
            result.success,
            result.evidence.len()
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generic_adapter_detect_form() {
        let html = r#"
            <html>
            <body>
                <form id="register-form">
                    <input type="text" name="username" />
                    <input type="email" name="email" />
                    <input type="password" name="password" />
                    <button type="submit">Register</button>
                </form>
            </body>
            </html>
        "#;

        let adapter = GenericAdapter::new();
        let result = adapter.detect_form(html, "http://example.com/register").await;

        assert!(result.is_ok());
        let form_info = result.unwrap();
        assert_eq!(form_info.selector, "#register-form");
        assert!(form_info.fields.len() >= 3);
    }
}
