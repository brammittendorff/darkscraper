/// Multilingual keyword detection for forms
/// Supports: English, Russian, German, French, Spanish, Chinese, Japanese, Italian, Portuguese

use std::collections::HashMap;

pub struct MultilingualDetector;

impl MultilingualDetector {
    /// Detect if text contains registration keywords in any language
    pub fn is_registration_text(text: &str) -> bool {
        let text_lower = text.to_lowercase();

        // English
        if Self::contains_any(&text_lower, &[
            "register", "sign up", "signup", "create account", "join",
            "new account", "registration"
        ]) {
            return true;
        }

        // Russian (Cyrillic)
        if Self::contains_any(&text_lower, &[
            "регистрация", "регистрироваться", "зарегистрироваться",
            "создать аккаунт", "новый аккаунт", "присоединиться"
        ]) {
            return true;
        }

        // German
        if Self::contains_any(&text_lower, &[
            "registrieren", "anmelden", "konto erstellen", "registrierung",
            "neues konto"
        ]) {
            return true;
        }

        // French
        if Self::contains_any(&text_lower, &[
            "inscription", "s'inscrire", "créer un compte", "rejoindre",
            "nouveau compte"
        ]) {
            return true;
        }

        // Spanish
        if Self::contains_any(&text_lower, &[
            "registrarse", "registro", "crear cuenta", "inscribirse",
            "nueva cuenta", "unirse"
        ]) {
            return true;
        }

        // Italian
        if Self::contains_any(&text_lower, &[
            "registrazione", "registrati", "crea account", "nuovo account"
        ]) {
            return true;
        }

        // Portuguese
        if Self::contains_any(&text_lower, &[
            "registrar", "cadastro", "criar conta", "inscrever-se",
            "nova conta"
        ]) {
            return true;
        }

        // Chinese (Simplified)
        if Self::contains_any(&text_lower, &[
            "注册", "註冊", "登记", "创建账户", "新账户", "加入"
        ]) {
            return true;
        }

        // Japanese
        if Self::contains_any(&text_lower, &[
            "登録", "新規登録", "アカウント作成", "新規アカウント"
        ]) {
            return true;
        }

        false
    }

    /// Detect login keywords in any language
    pub fn is_login_text(text: &str) -> bool {
        let text_lower = text.to_lowercase();

        // English
        if Self::contains_any(&text_lower, &["login", "log in", "sign in", "signin"]) {
            return true;
        }

        // Russian
        if Self::contains_any(&text_lower, &["вход", "войти", "авторизация"]) {
            return true;
        }

        // German
        if Self::contains_any(&text_lower, &["anmelden", "einloggen", "login"]) {
            return true;
        }

        // French
        if Self::contains_any(&text_lower, &["connexion", "se connecter", "login"]) {
            return true;
        }

        // Spanish
        if Self::contains_any(&text_lower, &["iniciar sesión", "acceder", "login", "entrar"]) {
            return true;
        }

        // Italian
        if Self::contains_any(&text_lower, &["accedi", "login", "accesso"]) {
            return true;
        }

        // Portuguese
        if Self::contains_any(&text_lower, &["entrar", "login", "acessar"]) {
            return true;
        }

        // Chinese
        if Self::contains_any(&text_lower, &["登录", "登錄", "登入"]) {
            return true;
        }

        // Japanese
        if Self::contains_any(&text_lower, &["ログイン", "サインイン"]) {
            return true;
        }

        false
    }

    /// Detect username field in any language
    pub fn is_username_field(name: &str) -> bool {
        let name_lower = name.to_lowercase();

        Self::contains_any(&name_lower, &[
            // English
            "user", "username", "login", "account",
            // Russian
            "пользователь", "имя", "логин",
            // German
            "benutzername", "benutzer",
            // French
            "utilisateur", "nom",
            // Spanish
            "usuario", "nombre",
            // Italian
            "utente", "nome",
            // Portuguese
            "usuário", "nome",
            // Chinese
            "用户", "用戶名",
            // Japanese
            "ユーザー", "ユーザー名",
        ])
    }

    /// Detect email field in any language
    pub fn is_email_field(name: &str) -> bool {
        let name_lower = name.to_lowercase();

        Self::contains_any(&name_lower, &[
            // English
            "email", "e-mail", "mail",
            // Russian
            "почта", "эл. почта", "емейл",
            // German
            "e-mail", "email", "post",
            // French
            "courriel", "email", "e-mail",
            // Spanish
            "correo", "email", "e-mail",
            // Italian
            "email", "posta",
            // Portuguese
            "email", "e-mail", "correio",
            // Chinese
            "邮箱", "邮件", "电子邮件",
            // Japanese
            "メール", "電子メール", "Eメール",
        ])
    }

    /// Detect password field in any language
    pub fn is_password_field(name: &str) -> bool {
        let name_lower = name.to_lowercase();

        Self::contains_any(&name_lower, &[
            // English
            "password", "pass", "pwd",
            // Russian
            "пароль",
            // German
            "passwort", "kennwort",
            // French
            "mot de passe", "mdp",
            // Spanish
            "contraseña", "clave",
            // Italian
            "password", "parola chiave",
            // Portuguese
            "senha", "password",
            // Chinese
            "密码", "密碼",
            // Japanese
            "パスワード",
        ])
    }

    /// Detect CAPTCHA in any language
    pub fn is_captcha_text(text: &str) -> bool {
        let text_lower = text.to_lowercase();

        Self::contains_any(&text_lower, &[
            // English
            "captcha", "verification", "security code", "prove you're human",
            // Russian
            "капча", "проверка", "код безопасности",
            // German
            "captcha", "sicherheitscode", "verifizierung",
            // French
            "captcha", "vérification", "code de sécurité",
            // Spanish
            "captcha", "verificación", "código de seguridad",
            // Italian
            "captcha", "verifica", "codice di sicurezza",
            // Portuguese
            "captcha", "verificação", "código de segurança",
            // Chinese
            "验证码", "驗證碼", "安全码",
            // Japanese
            "認証コード", "キャプチャ",
        ])
    }

    fn contains_any(text: &str, keywords: &[&str]) -> bool {
        keywords.iter().any(|keyword| text.contains(keyword))
    }
}

/// Get common form field names for each language
pub fn get_field_name_patterns() -> HashMap<String, Vec<String>> {
    let mut patterns = HashMap::new();

    // Username patterns
    patterns.insert(
        "username".to_string(),
        vec![
            "user", "username", "login", "account", "name",
            "пользователь", "логин", "имя",
            "benutzername", "benutzer",
            "utilisateur", "nom",
            "usuario", "nombre",
            "utente", "nome",
            "usuário",
            "用户", "用戶名",
            "ユーザー", "ユーザー名",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    );

    // Email patterns
    patterns.insert(
        "email".to_string(),
        vec![
            "email", "e-mail", "mail",
            "почта", "емейл",
            "post", "e-mail",
            "courriel",
            "correo",
            "posta",
            "correio",
            "邮箱", "邮件",
            "メール",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    );

    // Password patterns
    patterns.insert(
        "password".to_string(),
        vec![
            "password", "pass", "pwd", "passwd",
            "пароль",
            "passwort", "kennwort",
            "mot de passe", "mdp",
            "contraseña", "clave",
            "parola",
            "senha",
            "密码", "密碼",
            "パスワード",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    );

    patterns
}
