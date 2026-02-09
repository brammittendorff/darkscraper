/// Detect if a page response is a waiting screen / DDoS protection
/// These pages need headless browser with wait + mouse movements

pub fn is_waiting_screen(body_text: &str, html_size: usize) -> bool {
    // Very short responses are usually waiting screens
    if html_size < 500 {
        let body_lower = body_text.to_lowercase();

        if body_lower.contains("wait") ||
           body_lower.contains("loading") ||
           body_lower.contains("redirect") {
            return true;
        }
    }

    let body_lower = body_text.to_lowercase();

    // Check for common waiting screen indicators
    let waiting_keywords = vec![
        "please wait",
        "bitte warten",
        "proszę czekać",
        "пожалуйста, подождите",
        "just a moment",
        "checking your browser",
        "ddos protection",
        "cloudflare",
        "verifying you are human",
        "protection layer",
        "secure redirect",
        "preparing redirect",
    ];

    for keyword in waiting_keywords {
        if body_lower.contains(keyword) {
            return true;
        }
    }

    // Check for countdown timers (5 4 3 2 1)
    if body_lower.contains("5") &&
       body_lower.contains("4") &&
       body_lower.contains("3") &&
       body_lower.contains("redirecting") {
        return true;
    }

    // Check for very short pages with redirect
    if html_size < 1000 && body_lower.contains("redirect") {
        return true;
    }

    false
}

/// Determine wait time based on content (some need 30s, some need 5 minutes)
pub fn estimate_wait_time(body_text: &str) -> u64 {
    let body_lower = body_text.to_lowercase();

    // If countdown timer visible, extract the number
    if body_lower.contains("redirecting in") || body_lower.contains("wait") {
        // Look for numbers that might indicate seconds
        for i in 1..=60 {
            if body_lower.contains(&i.to_string()) {
                // Add buffer time
                return (i + 10) as u64;
            }
        }
    }

    // DDoS protection typically takes 5-30 seconds
    if body_lower.contains("ddos") || body_lower.contains("cloudflare") {
        return 30;
    }

    // Markets with "verifying" can take longer
    if body_lower.contains("verifying") || body_lower.contains("checking") {
        return 60;
    }

    // Default: wait 30 seconds
    30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_waiting_screen() {
        assert!(is_waiting_screen("Please Wait...", 14));
        assert!(is_waiting_screen("Bitte warten...", 34));
        assert!(is_waiting_screen("DDoS Protection Active", 100));
        assert!(is_waiting_screen("5 4 3 2 1 Redirecting…", 90));

        assert!(!is_waiting_screen("This is a normal page with lots of content", 5000));
    }

    #[test]
    fn test_estimate_wait_time() {
        assert_eq!(estimate_wait_time("Redirecting in 5 seconds"), 15);
        assert_eq!(estimate_wait_time("DDoS protection"), 30);
        assert_eq!(estimate_wait_time("Verifying your browser"), 60);
    }
}
