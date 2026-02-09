use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

/// Represents different types of form fields
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    Username,
    Email,
    Password,
    PasswordConfirm,
    FirstName,
    LastName,
    FullName,
    DateOfBirth,
    Country,
    City,
    PostalCode,
    Phone,
    Captcha,
    TermsCheckbox,
    NewsletterCheckbox,
    SecurityQuestion,
    Other(String),
}

/// Represents different HTML input types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputType {
    Text,
    Email,
    Password,
    Checkbox,
    Radio,
    Select,
    Textarea,
    File,
    Number,
    Date,
    Tel,
    Url,
    Hidden,
}

/// Evidence for why a field was classified a certain way
#[derive(Debug, Clone)]
pub struct Evidence {
    pub source: String,
    pub confidence: f32,
    pub details: String,
}

/// A classified form field with confidence score
#[derive(Debug, Clone)]
pub struct ClassifiedField {
    pub selector: String,
    pub name: Option<String>,
    pub id: Option<String>,
    pub field_type: FieldType,
    pub input_type: InputType,
    pub confidence: f32,
    pub evidence: Vec<Evidence>,
    pub required: bool,
    pub placeholder: Option<String>,
    pub label: Option<String>,
    pub aria_label: Option<String>,
}

/// Information about a detected form
#[derive(Debug, Clone)]
pub struct FormInfo {
    pub selector: String,
    pub action: Option<String>,
    pub method: String,
    pub fields: Vec<ClassifiedField>,
    pub submit_button: Option<SubmitButton>,
    pub form_type: FormType,
    pub confidence: f32,
    pub is_multi_step: bool,
    pub current_step: usize,
    pub total_steps: Option<usize>,
}

/// Types of forms
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormType {
    Registration,
    Login,
    Combined,  // Both login and registration on same page
    Unknown,
}

/// Submit button information
#[derive(Debug, Clone)]
pub struct SubmitButton {
    pub selector: String,
    pub text: String,
    pub confidence: f32,
}

/// Detection signal for form identification
#[derive(Debug, Clone)]
pub struct DetectionSignal {
    pub signal_type: String,
    pub confidence: f32,
    pub details: String,
}

/// Registration data to fill into forms
#[derive(Debug, Clone)]
pub struct RegistrationData {
    pub username: String,
    pub email: Option<String>,
    pub password: String,
    pub password_confirm: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub country: Option<String>,
    pub accept_terms: bool,
    pub subscribe_newsletter: bool,
    pub captcha_solution: Option<String>,
    pub custom_fields: HashMap<String, String>,
}

/// Result of a registration attempt
#[derive(Debug, Clone)]
pub struct RegistrationResult {
    pub success: bool,
    pub account_info: Option<AccountInfo>,
    pub error: Option<String>,
    pub requires_email_verification: bool,
    pub session_cookies: Option<String>,
    pub evidence: Vec<ResultEvidence>,
}

/// Information about a successfully created account
#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub username: String,
    pub email: Option<String>,
    pub user_id: Option<String>,
    pub profile_url: Option<String>,
}

/// Evidence for success/failure determination
#[derive(Debug, Clone)]
pub struct ResultEvidence {
    pub evidence_type: String,
    pub confidence: f32,
    pub details: String,
}

/// Page state snapshot for comparison
#[derive(Debug, Clone)]
pub struct PageState {
    pub url: String,
    pub html: String,
    pub cookies: Vec<String>,
    pub timestamp: std::time::SystemTime,
}

/// Trait for site-specific adapters
#[async_trait]
pub trait SiteAdapter: Send + Sync {
    /// Name of the adapter
    fn name(&self) -> &str;

    /// Check if this adapter matches the given site
    fn matches(&self, url: &Url, html: &str) -> f32;

    /// Detect registration form on the page
    async fn detect_form(&self, html: &str, url: &str) -> anyhow::Result<FormInfo>;

    /// Fill the form with registration data
    async fn fill_form(&self, form: &FormInfo, data: &RegistrationData) -> anyhow::Result<HashMap<String, String>>;

    /// Submit the form
    async fn submit_form(&self, form: &FormInfo) -> anyhow::Result<SubmitButton>;

    /// Detect success or failure after submission
    async fn detect_result(&self, before: &PageState, after: &PageState) -> anyhow::Result<RegistrationResult>;

    /// Optional hook before filling form
    async fn pre_fill_hook(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Optional hook after form submission
    async fn post_submit_hook(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Trait for field mapping strategies
pub trait FieldMappingStrategy: Send + Sync {
    /// Name of the strategy
    fn name(&self) -> &str;

    /// Map fields in the form
    fn map_fields(&self, html: &str, field_type: &FieldType) -> Vec<FieldMatch>;
}

/// A potential field match
#[derive(Debug, Clone)]
pub struct FieldMatch {
    pub selector: String,
    pub confidence: f32,
    pub strategy: String,
    pub evidence: Vec<String>,
}

/// Trait for success/failure detection signals
pub trait SuccessSignal: Send + Sync {
    /// Name of the signal
    fn name(&self) -> &str;

    /// Detect success indicator
    fn detect(&self, before: &PageState, after: &PageState) -> Option<f32>;
}

/// Trait for failure detection signals
pub trait FailureSignal: Send + Sync {
    /// Name of the signal
    fn name(&self) -> &str;

    /// Detect failure indicator
    fn detect(&self, before: &PageState, after: &PageState) -> Option<f32>;
}

/// Trait for retry strategies
pub trait RetryStrategy: Send + Sync {
    /// Name of the strategy
    fn name(&self) -> &str;

    /// Check if should retry
    fn should_retry(&self, error: &crate::RegistrationError, attempt: u32) -> bool;

    /// Calculate backoff duration
    fn backoff_duration(&self, attempt: u32) -> std::time::Duration;
}
