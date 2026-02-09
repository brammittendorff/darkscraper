use scraper::{Html, Selector, ElementRef};
use crate::core::types::*;
use crate::multilingual::MultilingualDetector;

pub struct FieldClassifier {
    confidence_threshold: f32,
}

impl Default for FieldClassifier {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.5,
        }
    }
}

impl FieldClassifier {
    pub fn new(confidence_threshold: f32) -> Self {
        Self { confidence_threshold }
    }

    /// Classify all fields in a form
    pub fn classify_fields(&self, form_html: &str, form_selector: &str) -> Vec<ClassifiedField> {
        let document = Html::parse_document(form_html);
        let mut classified_fields = Vec::new();

        // Find all input fields
        let input_selector = Selector::parse("input").unwrap();
        for input in document.select(&input_selector) {
            if let Some(classified) = self.classify_input(&input, form_selector) {
                if classified.confidence >= self.confidence_threshold {
                    classified_fields.push(classified);
                }
            }
        }

        // Find all textarea fields
        let textarea_selector = Selector::parse("textarea").unwrap();
        for textarea in document.select(&textarea_selector) {
            if let Some(classified) = self.classify_textarea(&textarea, form_selector) {
                if classified.confidence >= self.confidence_threshold {
                    classified_fields.push(classified);
                }
            }
        }

        // Find all select fields
        let select_selector = Selector::parse("select").unwrap();
        for select in document.select(&select_selector) {
            if let Some(classified) = self.classify_select(&select, form_selector) {
                if classified.confidence >= self.confidence_threshold {
                    classified_fields.push(classified);
                }
            }
        }

        classified_fields
    }

    /// Classify an input element
    fn classify_input(&self, input: &ElementRef, _form_selector: &str) -> Option<ClassifiedField> {
        let input_type = input.value().attr("type").unwrap_or("text");
        let name = input.value().attr("name");
        let id = input.value().attr("id");
        let placeholder = input.value().attr("placeholder");
        let required = input.value().attr("required").is_some();
        let aria_label = input.value().attr("aria-label");

        // Skip hidden, submit, and button types
        if matches!(input_type, "hidden" | "submit" | "button" | "reset" | "image") {
            return None;
        }

        // Get label text
        let label = self.find_label_for_element(input);

        // Build selector
        let selector = if let Some(id_val) = id {
            format!("#{}", id_val)
        } else if let Some(name_val) = name {
            format!("input[name='{}']", name_val)
        } else {
            return None;
        };

        // Classify field type with multiple strategies
        let (field_type, confidence, evidence) = self.determine_field_type(
            name,
            id,
            placeholder,
            aria_label.as_ref().map(|s| s.as_ref()),
            label.as_deref(),
            input_type,
        );

        let input_type_enum = self.map_input_type(input_type);

        Some(ClassifiedField {
            selector,
            name: name.map(String::from),
            id: id.map(String::from),
            field_type,
            input_type: input_type_enum,
            confidence,
            evidence,
            required,
            placeholder: placeholder.map(String::from),
            label,
            aria_label: aria_label.map(String::from),
        })
    }

    /// Classify a textarea element
    fn classify_textarea(&self, textarea: &ElementRef, _form_selector: &str) -> Option<ClassifiedField> {
        let name = textarea.value().attr("name");
        let id = textarea.value().attr("id");
        let placeholder = textarea.value().attr("placeholder");
        let required = textarea.value().attr("required").is_some();
        let aria_label = textarea.value().attr("aria-label");

        let label = self.find_label_for_element(textarea);

        let selector = if let Some(id_val) = id {
            format!("#{}", id_val)
        } else if let Some(name_val) = name {
            format!("textarea[name='{}']", name_val)
        } else {
            return None;
        };

        // Textareas are usually for longer text (bio, comments, etc.)
        let (field_type, confidence, evidence) = self.determine_field_type(
            name,
            id,
            placeholder,
            aria_label,
            label.as_deref(),
            "textarea",
        );

        Some(ClassifiedField {
            selector,
            name: name.map(String::from),
            id: id.map(String::from),
            field_type,
            input_type: InputType::Textarea,
            confidence,
            evidence,
            required,
            placeholder: placeholder.map(String::from),
            label,
            aria_label: aria_label.map(String::from),
        })
    }

    /// Classify a select element
    fn classify_select(&self, select: &ElementRef, _form_selector: &str) -> Option<ClassifiedField> {
        let name = select.value().attr("name");
        let id = select.value().attr("id");
        let required = select.value().attr("required").is_some();
        let aria_label = select.value().attr("aria-label");

        let label = self.find_label_for_element(select);

        let selector = if let Some(id_val) = id {
            format!("#{}", id_val)
        } else if let Some(name_val) = name {
            format!("select[name='{}']", name_val)
        } else {
            return None;
        };

        let (field_type, confidence, evidence) = self.determine_field_type(
            name,
            id,
            None,
            aria_label,
            label.as_deref(),
            "select",
        );

        Some(ClassifiedField {
            selector,
            name: name.map(String::from),
            id: id.map(String::from),
            field_type,
            input_type: InputType::Select,
            confidence,
            evidence,
            required,
            placeholder: None,
            label,
            aria_label: aria_label.map(String::from),
        })
    }

    /// Determine field type using multiple signals
    fn determine_field_type(
        &self,
        name: Option<&str>,
        id: Option<&str>,
        placeholder: Option<&str>,
        aria_label: Option<&str>,
        label: Option<&str>,
        html_type: &str,
    ) -> (FieldType, f32, Vec<Evidence>) {
        let mut evidence = Vec::new();
        let mut scores: Vec<(FieldType, f32)> = Vec::new();

        // Strategy 1: HTML type
        if html_type == "email" {
            scores.push((FieldType::Email, 0.9));
            evidence.push(Evidence {
                source: "html_type".to_string(),
                confidence: 0.9,
                details: "type=\"email\"".to_string(),
            });
        } else if html_type == "password" {
            // Check if it's password confirmation
            if let Some(name_val) = name {
                let name_lower = name_val.to_lowercase();
                if name_lower.contains("confirm") || name_lower.contains("repeat") || name_lower.contains("again") {
                    scores.push((FieldType::PasswordConfirm, 0.9));
                    evidence.push(Evidence {
                        source: "name_pattern".to_string(),
                        confidence: 0.9,
                        details: format!("Password confirmation: {}", name_val),
                    });
                } else {
                    scores.push((FieldType::Password, 0.9));
                    evidence.push(Evidence {
                        source: "html_type".to_string(),
                        confidence: 0.9,
                        details: "type=\"password\"".to_string(),
                    });
                }
            } else {
                scores.push((FieldType::Password, 0.9));
            }
        } else if html_type == "tel" {
            scores.push((FieldType::Phone, 0.9));
            evidence.push(Evidence {
                source: "html_type".to_string(),
                confidence: 0.9,
                details: "type=\"tel\"".to_string(),
            });
        } else if html_type == "checkbox" {
            // Analyze checkbox purpose
            if let Some(name_val) = name {
                let name_lower = name_val.to_lowercase();
                if name_lower.contains("terms") || name_lower.contains("agree") || name_lower.contains("accept") {
                    scores.push((FieldType::TermsCheckbox, 0.8));
                } else if name_lower.contains("newsletter") || name_lower.contains("subscribe") {
                    scores.push((FieldType::NewsletterCheckbox, 0.8));
                }
            }
        }

        // Strategy 2: Name attribute
        if let Some(name_val) = name {
            self.analyze_field_name(name_val, &mut scores, &mut evidence);
        }

        // Strategy 3: ID attribute
        if let Some(id_val) = id {
            self.analyze_field_name(id_val, &mut scores, &mut evidence);
        }

        // Strategy 4: Label text
        if let Some(label_text) = label {
            self.analyze_label_text(label_text, &mut scores, &mut evidence);
        }

        // Strategy 5: Placeholder
        if let Some(placeholder_text) = placeholder {
            self.analyze_label_text(placeholder_text, &mut scores, &mut evidence);
        }

        // Strategy 6: ARIA label
        if let Some(aria_text) = aria_label {
            self.analyze_label_text(aria_text, &mut scores, &mut evidence);
        }

        // Aggregate scores
        self.aggregate_scores(scores, evidence)
    }

    /// Analyze field name or ID
    fn analyze_field_name(&self, text: &str, scores: &mut Vec<(FieldType, f32)>, evidence: &mut Vec<Evidence>) {
        let text_lower = text.to_lowercase();

        // Use multilingual detector
        if MultilingualDetector::is_username_field(text) {
            scores.push((FieldType::Username, 0.8));
            evidence.push(Evidence {
                source: "name_pattern".to_string(),
                confidence: 0.8,
                details: format!("Username field: {}", text),
            });
        }

        if MultilingualDetector::is_email_field(text) {
            scores.push((FieldType::Email, 0.8));
            evidence.push(Evidence {
                source: "name_pattern".to_string(),
                confidence: 0.8,
                details: format!("Email field: {}", text),
            });
        }

        // Additional patterns
        if text_lower.contains("first") && text_lower.contains("name") {
            scores.push((FieldType::FirstName, 0.7));
        } else if text_lower.contains("last") && text_lower.contains("name") {
            scores.push((FieldType::LastName, 0.7));
        } else if text_lower.contains("fullname") || text_lower == "name" {
            scores.push((FieldType::FullName, 0.7));
        } else if text_lower.contains("country") {
            scores.push((FieldType::Country, 0.7));
        } else if text_lower.contains("city") {
            scores.push((FieldType::City, 0.7));
        } else if text_lower.contains("phone") || text_lower.contains("mobile") {
            scores.push((FieldType::Phone, 0.7));
        } else if text_lower.contains("birth") || text_lower.contains("dob") {
            scores.push((FieldType::DateOfBirth, 0.7));
        } else if text_lower.contains("captcha") {
            scores.push((FieldType::Captcha, 0.9));
        }
    }

    /// Analyze label text
    fn analyze_label_text(&self, text: &str, scores: &mut Vec<(FieldType, f32)>, evidence: &mut Vec<Evidence>) {
        self.analyze_field_name(text, scores, evidence);
    }

    /// Aggregate multiple scores for the same field
    fn aggregate_scores(&self, scores: Vec<(FieldType, f32)>, evidence: Vec<Evidence>) -> (FieldType, f32, Vec<Evidence>) {
        use std::collections::HashMap;

        if scores.is_empty() {
            return (FieldType::Other("unknown".to_string()), 0.3, evidence);
        }

        // Sum scores for each field type
        let mut type_scores: HashMap<String, (FieldType, f32)> = HashMap::new();

        for (field_type, score) in scores {
            let key = format!("{:?}", field_type);
            type_scores.entry(key.clone())
                .and_modify(|(_, s)| *s += score)
                .or_insert((field_type, score));
        }

        // Find highest scoring type
        let (field_type, total_score) = type_scores.values()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(ft, s)| (ft.clone(), *s))
            .unwrap();

        // Normalize score to 0-1 range (max possible score is around 3.0 from multiple signals)
        let normalized_score = (total_score / 3.0).min(1.0);

        (field_type, normalized_score, evidence)
    }

    /// Find associated label for an element
    fn find_label_for_element(&self, element: &ElementRef) -> Option<String> {
        // Try to find label by "for" attribute
        if let Some(id) = element.value().attr("id") {
            // Look for <label for="id">
            let html = element.html();
            let document = Html::parse_document(&html);
            let label_selector = Selector::parse(&format!("label[for='{}']", id)).ok()?;

            if let Some(label) = document.select(&label_selector).next() {
                return Some(label.text().collect::<String>().trim().to_string());
            }
        }

        // Check if element is wrapped in a label
        // This is harder without parent traversal, so we'll skip for now

        None
    }

    /// Map HTML input type to InputType enum
    fn map_input_type(&self, html_type: &str) -> InputType {
        match html_type {
            "text" => InputType::Text,
            "email" => InputType::Email,
            "password" => InputType::Password,
            "checkbox" => InputType::Checkbox,
            "radio" => InputType::Radio,
            "number" => InputType::Number,
            "tel" => InputType::Tel,
            "url" => InputType::Url,
            "date" => InputType::Date,
            "file" => InputType::File,
            "hidden" => InputType::Hidden,
            _ => InputType::Text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_email_field() {
        let html = r#"<input type="email" name="email" id="user-email" />"#;
        let classifier = FieldClassifier::default();
        let fields = classifier.classify_fields(html, "form");

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_type, FieldType::Email);
        assert!(fields[0].confidence > 0.8);
    }

    #[test]
    fn test_classify_password_confirmation() {
        let html = r#"<input type="password" name="password_confirm" />"#;
        let classifier = FieldClassifier::default();
        let fields = classifier.classify_fields(html, "form");

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_type, FieldType::PasswordConfirm);
    }
}
