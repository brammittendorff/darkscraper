use super::types::*;
use url::Url;
use std::time::SystemTime;

/// State machine for registration process
#[derive(Debug, Clone)]
pub enum RegistrationState {
    Initial,
    NavigatingToPage { url: String },
    LoadingPage,
    DetectingForm,
    FormDetected(FormInfo),
    MappingFields,
    FieldsMapped { mapping: Vec<ClassifiedField> },
    FillingFields { step: usize, total_steps: usize },
    AwaitingValidation,
    SolvingCaptcha { captcha_type: String },
    Submitting,
    AwaitingResponse,
    AwaitingEmailVerification { email: String },
    Success(RegistrationResult),
    Failed(crate::RegistrationError),
}

/// Transition between states with timestamp and evidence
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub timestamp: SystemTime,
    pub details: Option<String>,
}

/// Context for registration attempt
#[derive(Debug)]
pub struct RegistrationContext {
    pub state: RegistrationState,
    pub url: Url,
    pub adapter: String,  // Name of the adapter being used
    pub retry_count: u32,
    pub max_retries: u32,
    pub evidence: Vec<StateTransition>,
    pub page_states: Vec<PageState>,
    pub data: RegistrationData,
    pub started_at: SystemTime,
}

impl RegistrationContext {
    pub fn new(url: Url, data: RegistrationData, max_retries: u32) -> Self {
        Self {
            state: RegistrationState::Initial,
            url,
            adapter: "generic".to_string(),
            retry_count: 0,
            max_retries,
            evidence: Vec::new(),
            page_states: Vec::new(),
            data,
            started_at: SystemTime::now(),
        }
    }

    /// Transition to a new state
    pub fn transition(&mut self, new_state: RegistrationState, details: Option<String>) {
        let old_state = std::mem::replace(&mut self.state, new_state);

        self.evidence.push(StateTransition {
            from: format!("{:?}", old_state),
            to: format!("{:?}", self.state),
            timestamp: SystemTime::now(),
            details,
        });
    }

    /// Record current page state
    pub fn record_page_state(&mut self, html: String, cookies: Vec<String>) {
        self.page_states.push(PageState {
            url: self.url.to_string(),
            html,
            cookies,
            timestamp: SystemTime::now(),
        });
    }

    /// Get the previous page state
    pub fn previous_page_state(&self) -> Option<&PageState> {
        if self.page_states.len() >= 2 {
            self.page_states.get(self.page_states.len() - 2)
        } else {
            None
        }
    }

    /// Get the current page state
    pub fn current_page_state(&self) -> Option<&PageState> {
        self.page_states.last()
    }

    /// Check if can retry
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry counter
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Get duration since start
    pub fn duration(&self) -> std::time::Duration {
        self.started_at.elapsed().unwrap_or_default()
    }
}
