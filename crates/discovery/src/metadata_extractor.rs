use serde::Serialize;

/// Extract metadata from images (EXIF-like), PDFs, and documents.
/// Phase 1: basic content-type detection and size tracking.
/// Phase 2: full EXIF and PDF metadata via dedicated crates.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentMetadata {
    pub url: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub metadata_type: String,
    pub fields: Vec<(String, String)>,
}

pub struct MetadataExtractor;

impl MetadataExtractor {
    /// Check if a content type is an image that might contain EXIF data.
    pub fn is_image(content_type: &str) -> bool {
        content_type.starts_with("image/jpeg")
            || content_type.starts_with("image/tiff")
            || content_type.starts_with("image/png")
    }

    /// Check if content is a PDF.
    pub fn is_pdf(content_type: &str) -> bool {
        content_type.contains("application/pdf")
    }

    /// Phase 1: Extract basic metadata from response.
    /// Returns metadata fields we can determine without specialized parsers.
    pub fn extract_basic(url: &str, content_type: &str, body: &[u8]) -> DocumentMetadata {
        let mut fields = Vec::new();

        // Check for PDF author field (simple grep)
        if Self::is_pdf(content_type) {
            if let Ok(text) = std::str::from_utf8(body) {
                // Very basic PDF metadata extraction from plaintext markers
                for marker in &["/Author", "/Creator", "/Producer", "/Title", "/Subject"] {
                    if let Some(pos) = text.find(marker) {
                        // Try to extract the value in parentheses after the marker
                        let after = &text[pos + marker.len()..];
                        if let Some(start) = after.find('(') {
                            if let Some(end) = after[start..].find(')') {
                                let value = &after[start + 1..start + end];
                                if !value.is_empty() && value.len() < 200 {
                                    fields.push((
                                        marker.trim_start_matches('/').to_string(),
                                        value.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check for EXIF markers in JPEG
        if Self::is_image(content_type) && body.len() > 12 {
            // JPEG SOI + APP1 EXIF marker
            if body[0] == 0xFF && body[1] == 0xD8 {
                fields.push(("format".to_string(), "JPEG".to_string()));
                // Check for EXIF APP1 marker
                if body.len() > 4 && body[2] == 0xFF && body[3] == 0xE1 {
                    fields.push(("has_exif".to_string(), "true".to_string()));
                }
            }
        }

        DocumentMetadata {
            url: url.to_string(),
            content_type: content_type.to_string(),
            size_bytes: body.len(),
            metadata_type: if Self::is_pdf(content_type) {
                "pdf".to_string()
            } else if Self::is_image(content_type) {
                "image".to_string()
            } else {
                "other".to_string()
            },
            fields,
        }
    }
}
