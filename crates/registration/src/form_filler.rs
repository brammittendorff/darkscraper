use scraper::{Html, Selector};
use std::collections::HashMap;
use tracing::info;

use crate::RegistrationError;

#[derive(Debug, Clone)]
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub selector: String,
}

#[derive(Debug, Clone)]
pub struct RegistrationForm {
    pub action: Option<String>,
    pub method: String,
    pub fields: Vec<FormField>,
    pub has_username: bool,
    pub has_email: bool,
    pub has_password: bool,
    pub has_password_confirm: bool,
}

pub struct FormAnalyzer;

impl FormAnalyzer {
    /// Analyze HTML and find registration form
    pub fn find_registration_form(html: &str) -> Result<RegistrationForm, RegistrationError> {
        let document = Html::parse_document(html);

        // Find forms that look like registration forms
        let form_selector = Selector::parse("form").unwrap();

        for form_elem in document.select(&form_selector) {
            let form_html = form_elem.html().to_lowercase();

            // Check if this is a registration form
            if Self::is_registration_form(&form_html) {
                let action = form_elem.value().attr("action").map(|s| s.to_string());
                let method = form_elem
                    .value()
                    .attr("method")
                    .unwrap_or("post")
                    .to_string();

                let fields = Self::extract_form_fields(&form_elem);

                // Analyze field types
                let has_username = fields.iter().any(|f| Self::is_username_field(&f.name));
                let has_email = fields.iter().any(|f| Self::is_email_field(&f.name, &f.field_type));
                let password_fields: Vec<_> = fields
                    .iter()
                    .filter(|f| f.field_type == "password")
                    .collect();
                let has_password = !password_fields.is_empty();
                let has_password_confirm = password_fields.len() >= 2;

                info!(
                    "found registration form: username={}, email={}, password={}, confirm={}",
                    has_username, has_email, has_password, has_password_confirm
                );

                return Ok(RegistrationForm {
                    action,
                    method,
                    fields,
                    has_username,
                    has_email,
                    has_password,
                    has_password_confirm,
                });
            }
        }

        Err(RegistrationError::FormNotFound)
    }

    /// Check if form looks like a registration form
    fn is_registration_form(form_html: &str) -> bool {
        let keywords = vec![
            "register",
            "sign up",
            "signup",
            "create account",
            "join",
            "new account",
        ];

        for keyword in keywords {
            if form_html.contains(keyword) {
                return true;
            }
        }

        false
    }

    /// Extract all input fields from form
    fn extract_form_fields(form_elem: &scraper::ElementRef) -> Vec<FormField> {
        let mut fields = Vec::new();

        // Parse input fields
        let input_selector = Selector::parse("input").unwrap();
        for input in form_elem.select(&input_selector) {
            let name = input.value().attr("name").unwrap_or("").to_string();
            let field_type = input.value().attr("type").unwrap_or("text").to_string();
            let required = input.value().attr("required").is_some();

            if !name.is_empty() && field_type != "submit" && field_type != "hidden" {
                let selector = if let Some(id) = input.value().attr("id") {
                    format!("#{}", id)
                } else {
                    format!("input[name='{}']", name)
                };

                fields.push(FormField {
                    name: name.clone(),
                    field_type,
                    required,
                    selector,
                });
            }
        }

        // Parse textarea fields
        let textarea_selector = Selector::parse("textarea").unwrap();
        for textarea in form_elem.select(&textarea_selector) {
            if let Some(name) = textarea.value().attr("name") {
                let required = textarea.value().attr("required").is_some();

                let selector = if let Some(id) = textarea.value().attr("id") {
                    format!("#{}", id)
                } else {
                    format!("textarea[name='{}']", name)
                };

                fields.push(FormField {
                    name: name.to_string(),
                    field_type: "textarea".to_string(),
                    required,
                    selector,
                });
            }
        }

        // Parse select fields
        let select_selector = Selector::parse("select").unwrap();
        for select in form_elem.select(&select_selector) {
            if let Some(name) = select.value().attr("name") {
                let required = select.value().attr("required").is_some();

                let selector = if let Some(id) = select.value().attr("id") {
                    format!("#{}", id)
                } else {
                    format!("select[name='{}']", name)
                };

                fields.push(FormField {
                    name: name.to_string(),
                    field_type: "select".to_string(),
                    required,
                    selector,
                });
            }
        }

        fields
    }

    /// Check if field is a username field
    fn is_username_field(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        name_lower.contains("user") || name_lower.contains("username") || name_lower == "name"
    }

    /// Check if field is an email field
    fn is_email_field(name: &str, field_type: &str) -> bool {
        field_type == "email" || name.to_lowercase().contains("email")
    }

    /// Create field mapping for form submission
    pub fn create_field_mapping(
        form: &RegistrationForm,
        username: &str,
        password: &str,
        email: Option<&str>,
    ) -> HashMap<String, String> {
        let mut mapping = HashMap::new();

        for field in &form.fields {
            let name_lower = field.name.to_lowercase();

            // Fill username fields
            if Self::is_username_field(&field.name) {
                mapping.insert(field.selector.clone(), username.to_string());
                info!("mapping username to field: {}", field.name);
            }
            // Fill email fields
            else if Self::is_email_field(&field.name, &field.field_type) {
                if let Some(email_addr) = email {
                    mapping.insert(field.selector.clone(), email_addr.to_string());
                    info!("mapping email to field: {}", field.name);
                }
            }
            // Fill password fields
            else if field.field_type == "password" {
                mapping.insert(field.selector.clone(), password.to_string());
                info!("mapping password to field: {}", field.name);
            }
            // Handle other common fields with defaults
            else if name_lower.contains("agree") || name_lower.contains("terms") {
                if field.field_type == "checkbox" {
                    // Will need to click checkbox instead of setting value
                    info!("found terms agreement checkbox: {}", field.name);
                }
            }
        }

        mapping
    }
}
