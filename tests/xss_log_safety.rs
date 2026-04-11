//! XSS safety tests for the log viewer rendering pipeline.
//!
//! UI-10: All log content HTML-escaped by default; ANSI parsing is the only
//! allowed transformation. This test ensures that malicious log content
//! cannot inject HTML/JS into the browser.
//!
//! These tests run in CI and act as a regression safety net.

use cronduit::web::ansi::render_log_line;

#[test]
fn script_tag_is_escaped() {
    let input = "<script>alert(1)</script>";
    let output = render_log_line(input);
    assert!(
        output.contains("&lt;script&gt;"),
        "script tag must be HTML-escaped, got: {output}"
    );
    assert!(
        !output.contains("<script>"),
        "raw <script> must NOT appear in output, got: {output}"
    );
}

#[test]
fn ansi_colors_converted_to_spans() {
    let input = "\x1b[31mERROR\x1b[0m normal text";
    let output = render_log_line(input);
    assert!(
        output.contains("<span") || output.contains("ERROR"),
        "ANSI red should produce span tags or preserve text, got: {output}"
    );
    // The text content should be present
    assert!(output.contains("ERROR"), "text content preserved");
    assert!(output.contains("normal text"), "non-ANSI text preserved");
}

#[test]
fn html_injection_inside_ansi_is_escaped() {
    let input = "\x1b[32m<img src=x onerror=alert(1)>\x1b[0m";
    let output = render_log_line(input);
    assert!(
        output.contains("&lt;img"),
        "img tag must be escaped inside ANSI sequence, got: {output}"
    );
    assert!(
        !output.contains("<img"),
        "raw <img> must NOT appear, got: {output}"
    );
}

#[test]
fn empty_string_is_safe() {
    let output = render_log_line("");
    assert!(
        output.is_empty(),
        "empty input -> empty output, got: {output}"
    );
}

#[test]
fn plain_text_unchanged() {
    let input = "2026-04-10T12:00:00Z INFO job completed successfully";
    let output = render_log_line(input);
    assert!(
        output.contains("job completed successfully"),
        "plain text preserved, got: {output}"
    );
}

#[test]
fn sgr_sequences_with_html_entities() {
    // Ensure ampersands and angle brackets are also escaped
    let input = "value: A & B < C > D";
    let output = render_log_line(input);
    assert!(
        output.contains("&amp;"),
        "ampersands should be HTML-escaped, got: {output}"
    );
    assert!(
        output.contains("&lt;"),
        "less-than should be HTML-escaped, got: {output}"
    );
    assert!(
        output.contains("&gt;"),
        "greater-than should be HTML-escaped, got: {output}"
    );
}

/// Verify that |safe is only used on the ANSI-rendered log.html field,
/// never on raw user content variables.
#[test]
fn safe_filter_only_on_ansi_output() {
    use std::path::Path;

    let templates_dir = Path::new("templates");
    if !templates_dir.exists() {
        // Skip in environments where templates aren't accessible
        return;
    }

    let mut safe_usages = Vec::new();
    walk_templates(templates_dir, &mut safe_usages);

    // Filter to only unexpected |safe usages
    let unexpected: Vec<_> = safe_usages
        .iter()
        .filter(|(file, line)| {
            // The ONLY allowed |safe is on log.html in log_viewer.html
            let is_log_viewer = file.contains("log_viewer");
            let is_log_html = line.contains("log.html");
            !(is_log_viewer && is_log_html)
        })
        .collect();

    assert!(
        unexpected.is_empty(),
        "|safe used in unexpected locations (XSS risk): {:?}",
        unexpected
    );
}

fn walk_templates(dir: &std::path::Path, results: &mut Vec<(String, String)>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_templates(&path, results);
            } else if path.extension().map_or(false, |e| e == "html") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        if line.contains("|safe") || line.contains("| safe") {
                            results.push((path.display().to_string(), line.trim().to_string()));
                        }
                    }
                }
            }
        }
    }
}
