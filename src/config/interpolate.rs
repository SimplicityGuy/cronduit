use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug)]
pub struct InterpolationError {
    pub kind: ErrorKind,
    pub byte_range: std::ops::Range<usize>,
}

#[derive(Debug)]
pub enum ErrorKind {
    MissingVar(String),
    DefaultSyntaxForbidden,
}

/// Expand `${VAR}` references in `input`. Collects all errors (missing vars,
/// forbidden `${VAR:-default}` syntax) into a Vec; never early-exits.
///
/// TOML comments (lines starting with optional whitespace then `#`) are
/// skipped so that documentation examples containing `${VAR}` don't trigger
/// missing-variable errors.
pub fn interpolate(input: &str) -> (String, Vec<InterpolationError>) {
    static VAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap());
    static DEFAULT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{[^}]*:-").unwrap());

    let mut errors = Vec::new();

    // Strip TOML comments before interpolation so that `${VAR}` inside
    // comments is not expanded. We rebuild the string line-by-line,
    // preserving comment lines verbatim and only interpolating value lines.
    let mut result = String::with_capacity(input.len());
    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            // Comment line — pass through unchanged.
            result.push_str(line);
        } else {
            // Check for inline comment: find `#` that isn't inside a string.
            // For simplicity, interpolate only the portion before an inline
            // comment. TOML inline comments start at `#` outside of quotes.
            let content = strip_inline_comment(line);

            for m in DEFAULT_RE.find_iter(content) {
                errors.push(InterpolationError {
                    kind: ErrorKind::DefaultSyntaxForbidden,
                    byte_range: offset_range(input, line, m.range()),
                });
            }

            let expanded = VAR_RE.replace_all(content, |caps: &regex::Captures| {
                let var = &caps[1];
                match std::env::var(var) {
                    Ok(v) => v,
                    Err(_) => {
                        errors.push(InterpolationError {
                            kind: ErrorKind::MissingVar(var.to_string()),
                            byte_range: offset_range(input, line, caps.get(0).unwrap().range()),
                        });
                        String::new()
                    }
                }
            });
            result.push_str(&expanded);
            // Append the inline comment portion unchanged.
            if content.len() < line.len() {
                result.push_str(&line[content.len()..]);
            }
        }
        result.push('\n');
    }
    // Remove trailing newline added by the loop if input didn't end with one.
    if !input.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    (result, errors)
}

/// Return the portion of `line` before any inline TOML comment.
/// A `#` is an inline comment only if it's outside quoted strings.
fn strip_inline_comment(line: &str) -> &str {
    let mut in_basic = false;
    let mut in_literal = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' if !in_literal => in_basic = !in_basic,
            '\'' if !in_basic => in_literal = !in_literal,
            '#' if !in_basic && !in_literal => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Convert a range relative to `line` into a range relative to `input`.
fn offset_range(input: &str, line: &str, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let line_offset = line.as_ptr() as usize - input.as_ptr() as usize;
    (line_offset + range.start)..(line_offset + range.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialize tests that mutate the process environment.
    /// `std::env::set_var` / `remove_var` are unsafe in Rust 1.82+ because
    /// concurrent env reads from other threads are UB. Holding this mutex
    /// ensures only one env-mutating test runs at a time.
    pub(crate) static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn missing_var_collected() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: ENV_MUTEX guarantees no concurrent env access from tests.
        unsafe {
            std::env::remove_var("CRONDUIT_TEST_MISSING");
        }
        let (out, errs) = interpolate("foo=${CRONDUIT_TEST_MISSING}");
        assert_eq!(errs.len(), 1);
        match &errs[0].kind {
            ErrorKind::MissingVar(v) => assert_eq!(v, "CRONDUIT_TEST_MISSING"),
            _ => panic!("expected MissingVar"),
        }
        assert_eq!(out, "foo="); // substituted with empty placeholder
    }

    #[test]
    fn present_var_substituted() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: ENV_MUTEX guarantees no concurrent env access from tests.
        unsafe {
            std::env::set_var("CRONDUIT_TEST_PRESENT", "hello");
        }
        let (out, errs) = interpolate("x=${CRONDUIT_TEST_PRESENT}");
        assert!(errs.is_empty());
        assert_eq!(out, "x=hello");
    }

    #[test]
    fn default_syntax_rejected() {
        let (_out, errs) = interpolate("x=${FOO:-bar}");
        assert!(matches!(errs[0].kind, ErrorKind::DefaultSyntaxForbidden));
    }

    #[test]
    fn comments_not_interpolated() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("NONEXISTENT");
        }
        let input = "# reference ${NONEXISTENT} in a comment\nkey = \"value\"";
        let (out, errs) = interpolate(input);
        assert!(errs.is_empty(), "comment ${{VAR}} should not trigger error");
        assert!(out.contains("${NONEXISTENT}"), "comment should be preserved verbatim");
    }

    #[test]
    fn inline_comment_not_interpolated() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("NONEXISTENT");
        }
        let input = "key = \"value\" # see ${NONEXISTENT}";
        let (out, errs) = interpolate(input);
        assert!(errs.is_empty(), "inline comment ${{VAR}} should not trigger error");
        assert!(out.contains("${NONEXISTENT}"));
    }
}
