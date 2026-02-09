/// Detect and click registration links on login pages

use tracing::info;

/// Common patterns for registration links
pub fn get_register_link_selectors() -> Vec<&'static str> {
    vec![
        // By href attribute
        "a[href*='register']",
        "a[href*='signup']",
        "a[href*='sign-up']",
        "a[href*='sign_up']",
        "a[href*='join']",
        "a[href*='create']",
        "a[href*='ucp.php?mode=register']",  // phpBB forums

        // By text content (case-insensitive via CSS)
        "a:contains('Sign up')",
        "a:contains('Sign Up')",
        "a:contains('SIGN UP')",
        "a:contains('Register')",
        "a:contains('REGISTER')",
        "a:contains('Create account')",
        "a:contains('Create Account')",
        "a:contains('Join')",
        "a:contains('JOIN')",

        // By class/id
        "a.register",
        "a.signup",
        "a#register",
        "a#signup",
        ".register-link a",
        ".signup-link a",

        // Buttons that look like links
        "button[onclick*='register']",
        "button[onclick*='signup']",

        // Multilingual
        "a:contains('Регистрация')",  // Russian
        "a:contains('Registrieren')",  // German
        "a:contains('Inscription')",  // French
        "a:contains('Registrarse')",  // Spanish
        "a:contains('注册')",  // Chinese
        "a:contains('登録')",  // Japanese
    ]
}

/// Check if page looks like a login page (not registration page)
pub fn is_login_page(html: &str) -> bool {
    let html_lower = html.to_lowercase();

    // Has login indicators
    let has_login = html_lower.contains("sign in") ||
                   html_lower.contains("log in") ||
                   html_lower.contains("login");

    // Has register link/text
    let has_register_text = html_lower.contains("sign up") ||
                           html_lower.contains("register") ||
                           html_lower.contains("create account");

    // Has login form (single password field)
    let password_count = html_lower.matches("type=\"password\"").count() +
                        html_lower.matches("type='password'").count();

    // Login page if: has login + has register text + only 1 password field
    has_login && has_register_text && password_count <= 1
}
