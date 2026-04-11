//! ANSI SGR to HTML conversion for the log viewer.
//!
//! D-06: Server-side ANSI parsing into sanitized <span> tags.
//! UI-10: All log content HTML-escaped; ANSI parsing is the only transformation.
//!
//! SECURITY: `ansi_to_html::convert()` HTML-escapes input FIRST,
//! then wraps ANSI-styled segments in <span> tags. The output is safe
//! to render with `| safe` in askama because no raw HTML from log content
//! can survive the escaping step.

/// Convert a raw log line (potentially containing ANSI SGR codes) to safe HTML.
///
/// Returns HTML string with <span> tags for ANSI colors.
/// The input is HTML-escaped before ANSI processing, so XSS is impossible.
pub fn render_log_line(raw: &str) -> String {
    ansi_to_html::convert(raw).unwrap_or_else(|_| {
        // Fallback: HTML-escape without ANSI processing
        html_escape(raw)
    })
}

/// Simple HTML escaping fallback.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through() {
        let result = render_log_line("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn html_is_escaped() {
        let result = render_log_line("<script>alert('xss')</script>");
        assert!(!result.contains("<script>"), "HTML should be escaped: {result}");
        assert!(result.contains("&lt;script&gt;"), "got: {result}");
    }

    #[test]
    fn ansi_color_produces_span() {
        // \x1b[31m = red text, \x1b[0m = reset
        let result = render_log_line("\x1b[31mERROR\x1b[0m: something failed");
        assert!(result.contains("<span"), "ANSI should produce span: {result}");
        assert!(result.contains("ERROR"), "text should be preserved: {result}");
    }
}
