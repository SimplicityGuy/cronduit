//! Double-submit CSRF token protection.
//!
//! D-11: Random token in HttpOnly; SameSite=Strict cookie + hidden form field.
//! Server validates they match. No server-side session store needed.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use rand::RngCore;

/// Cookie name for the CSRF token.
pub const CSRF_COOKIE_NAME: &str = "cronduit_csrf";

/// Form field name for the CSRF token.
pub const CSRF_FIELD_NAME: &str = "csrf_token";

/// Generate a random 32-byte CSRF token as a 64-char hex string.
pub fn generate_csrf_token() -> String {
    let mut token = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut token);
    hex::encode(token)
}

/// Validate that cookie token and form token match.
/// Uses constant-time comparison to prevent timing attacks.
pub fn validate_csrf(cookie_token: &str, form_token: &str) -> bool {
    if cookie_token.is_empty() || form_token.is_empty() {
        return false;
    }
    let a = cookie_token.as_bytes();
    let b = form_token.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Extract the CSRF token from the cookie jar, or generate a new one.
pub fn get_token_from_cookies(cookies: &CookieJar) -> String {
    cookies
        .get(CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .unwrap_or_else(generate_csrf_token)
}

/// Axum middleware that ensures the CSRF cookie is set on every response.
pub async fn ensure_csrf_cookie(
    cookies: CookieJar,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    let has_cookie = cookies.get(CSRF_COOKIE_NAME).is_some();
    let mut response = next.run(request).await;

    if !has_cookie {
        let token = generate_csrf_token();
        let cookie = format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/; Secure",
            CSRF_COOKIE_NAME, token
        );
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, cookie.parse().unwrap());
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_is_64_hex_chars() {
        let token = generate_csrf_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_is_unique() {
        let t1 = generate_csrf_token();
        let t2 = generate_csrf_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn matching_tokens_validate() {
        assert!(validate_csrf("abc123def456", "abc123def456"));
    }

    #[test]
    fn mismatched_tokens_reject() {
        assert!(!validate_csrf("abc123", "xyz789"));
    }

    #[test]
    fn empty_cookie_rejects() {
        assert!(!validate_csrf("", "abc123"));
    }

    #[test]
    fn empty_form_field_rejects() {
        assert!(!validate_csrf("abc123", ""));
    }

    #[test]
    fn different_length_tokens_reject() {
        assert!(!validate_csrf("short", "muchlongertoken"));
    }
}
