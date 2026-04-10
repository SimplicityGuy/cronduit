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
pub fn interpolate(input: &str) -> (String, Vec<InterpolationError>) {
    static VAR_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap());
    static DEFAULT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{[^}]*:-").unwrap());

    let mut errors = Vec::new();

    for m in DEFAULT_RE.find_iter(input) {
        errors.push(InterpolationError {
            kind: ErrorKind::DefaultSyntaxForbidden,
            byte_range: m.range(),
        });
    }

    let result = VAR_RE.replace_all(input, |caps: &regex::Captures| {
        let var = &caps[1];
        match std::env::var(var) {
            Ok(v) => v,
            Err(_) => {
                errors.push(InterpolationError {
                    kind: ErrorKind::MissingVar(var.to_string()),
                    byte_range: caps.get(0).unwrap().range(),
                });
                String::new()
            }
        }
    });

    (result.into_owned(), errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_var_collected() {
        // Ensure the var is not in env
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
}
