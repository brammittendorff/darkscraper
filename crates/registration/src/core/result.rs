use super::context::*;
use super::types::*;

/// Detailed result with diagnostics
#[derive(Debug)]
pub struct DetailedRegistrationResult {
    pub result: RegistrationResult,
    pub context: RegistrationContext,
    pub diagnostics: Diagnostics,
}

/// Diagnostics information for debugging
#[derive(Debug, Clone)]
pub struct Diagnostics {
    pub total_duration_ms: u128,
    pub page_loads: usize,
    pub form_detection_attempts: usize,
    pub field_mapping_attempts: usize,
    pub captcha_encountered: bool,
    pub validation_errors: Vec<String>,
    pub retry_count: u32,
    pub adapter_used: String,
    pub state_transitions: Vec<String>,
    pub screenshots_taken: Vec<String>,
}

impl DetailedRegistrationResult {
    pub fn new(result: RegistrationResult, context: RegistrationContext) -> Self {
        let diagnostics = Diagnostics {
            total_duration_ms: context.duration().as_millis(),
            page_loads: context.page_states.len(),
            form_detection_attempts: 0,  // Will be incremented during process
            field_mapping_attempts: 0,
            captcha_encountered: false,
            validation_errors: Vec::new(),
            retry_count: context.retry_count,
            adapter_used: context.adapter.clone(),
            state_transitions: context
                .evidence
                .iter()
                .map(|t| format!("{} -> {}", t.from, t.to))
                .collect(),
            screenshots_taken: Vec::new(),
        };

        Self {
            result,
            context,
            diagnostics,
        }
    }

    /// Check if registration was successful
    pub fn is_success(&self) -> bool {
        self.result.success
    }

    /// Get human-readable summary
    pub fn summary(&self) -> String {
        if self.is_success() {
            format!(
                "✓ Registration successful for '{}' in {}ms (retries: {})",
                self.context.data.username,
                self.diagnostics.total_duration_ms,
                self.diagnostics.retry_count
            )
        } else {
            format!(
                "✗ Registration failed: {} (duration: {}ms, retries: {})",
                self.result.error.as_deref().unwrap_or("unknown error"),
                self.diagnostics.total_duration_ms,
                self.diagnostics.retry_count
            )
        }
    }
}
