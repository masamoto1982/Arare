//! Conformance runner.
//!
//! Ajisai's identity is defined language-independently by the HTML conformance
//! suite at `tests/conformance/`, not by this Rust implementation. Each
//! `<section class="ajisai-case">` pairs an Ajisai source program with its
//! expected final result and the expected ordered sequence of host effects.
//!
//! Contract (see `tests/conformance/index.html` and `PORTABILITY.md`):
//!   1. The `ajisai-source` is executed as an Ajisai program.
//!   2. The observed final result (the whole stack, rendered value-by-value)
//!      must equal `ajisai-expect-result` after normalization.
//!   3. The fired `HostEffect`s must equal the `ajisai-effect` elements in
//!      order — both `data-kind` and `data-payload`.
//!
//! Observation target for effects is `Interpreter::host_effects()` — the
//! structured channel — NOT the human-readable `output_buffer`.
//!
//! Normalization (deliberately minimal, §"正規化規則"): only whitespace
//! BETWEEN tokens (spaces / tabs / newlines) is normalized, because whitespace
//! is a separator in the value model, not a value. Everything else — numeric
//! notation (`n/d`), element order, nesting, the spelling of `NIL`/`UNKNOWN` —
//! is matched exactly. If this line ever conflates phenomenon with notation it
//! must only be loosened, with a comment recording the reason.
//!
//! The parser is strict: a case missing a required element, or an
//! `ajisai-effect` missing `data-kind`/`data-payload`, aborts with a panic. We
//! never silently skip a malformed case.

use crate::interpreter::{Interpreter, RecordingHostEnv};
use std::sync::Arc;

const SUITE: &str = include_str!("../../tests/conformance/index.html");

#[derive(Debug)]
struct ExpectedEffect {
    kind: String,
    payload: String,
}

#[derive(Debug)]
struct Case {
    id: String,
    source: String,
    expect_result: String,
    expect_error: Option<String>,
    effects: Vec<ExpectedEffect>,
}

// ── tiny strict HTML extraction ───────────────────────────────────────────
// A targeted scanner for the fixed `class`/`data-*` structure of the suite.
// Not a general HTML parser; it assumes the suite is well-formed and well
// quoted, and fails loudly when a required piece is absent.

/// Decode the small set of HTML entities the suite may contain.
fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        // `&amp;` must be last so it does not re-trigger the others.
        .replace("&amp;", "&")
}

/// Collapse inter-token whitespace to single spaces and trim. This is the only
/// normalization applied to results. Host-effect payload strings are exact.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct StartTag {
    name: String,
    attrs: String,
    /// Byte index just past the closing `>` of this start tag.
    content_start: usize,
}

/// Parse a start tag beginning at `lt` (which must index a `<`).
fn parse_start_tag(html: &str, lt: usize) -> Option<StartTag> {
    let bytes = html.as_bytes();
    debug_assert_eq!(bytes[lt], b'<');
    let mut i = lt + 1;
    let name_start = i;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    if i == name_start {
        return None; // `</...`, `<!--`, etc. — not a start tag.
    }
    let name = html[name_start..i].to_string();
    let attrs_start = i;
    // Walk to the closing `>`, skipping over quoted attribute values so a `>`
    // inside an attribute does not end the tag prematurely.
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == b'"' || c == b'\'' {
                    quote = Some(c);
                } else if c == b'>' {
                    let attrs = html[attrs_start..i]
                        .trim()
                        .trim_end_matches('/')
                        .to_string();
                    return Some(StartTag {
                        name,
                        attrs,
                        content_start: i + 1,
                    });
                }
            }
        }
        i += 1;
    }
    None
}

/// Find the matching close tag for `name`, accounting for nested same-name
/// tags. Returns `(inner_html, outer_end)` where `outer_end` is just past the
/// closing `</name>`.
fn find_matching_close(html: &str, content_start: usize, name: &str) -> Option<(String, usize)> {
    let bytes = html.as_bytes();
    let mut i = content_start;
    let mut depth = 1usize;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                // Possible close tag.
                let after = &html[i + 2..];
                if after
                    .strip_prefix(name)
                    .map(|rest| rest.trim_start().starts_with('>'))
                    .unwrap_or(false)
                {
                    depth -= 1;
                    let gt = i + 2 + html[i + 2..].find('>').unwrap();
                    if depth == 0 {
                        return Some((html[content_start..i].to_string(), gt + 1));
                    }
                    i = gt + 1;
                    continue;
                }
            } else if let Some(st) = parse_start_tag(html, i) {
                if st.name == name {
                    depth += 1;
                }
                i = st.content_start;
                continue;
            }
        }
        i += 1;
    }
    None
}

struct Element {
    attrs: String,
    inner: String,
    /// Byte index just past this element's close tag.
    outer_end: usize,
}

/// Find the next element (any tag) whose `class` attribute contains `class`,
/// searching `html[from..]`.
fn find_element_by_class(html: &str, from: usize, class: &str) -> Option<Element> {
    let mut i = from;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(st) = parse_start_tag(html, i) {
                if class_attr_contains(&st.attrs, class) {
                    let (inner, outer_end) = find_matching_close(html, st.content_start, &st.name)?;
                    return Some(Element {
                        attrs: st.attrs,
                        inner,
                        outer_end,
                    });
                }
                i = st.content_start;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Read the value of attribute `name` from a start-tag attribute string.
fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let mut search = 0;
    let bytes = attrs.as_bytes();
    while let Some(rel) = attrs[search..].find(name) {
        let at = search + rel;
        // Must be a whole attribute name: preceded by start/space, followed by `=`.
        let before_ok = at == 0 || attrs.as_bytes()[at - 1].is_ascii_whitespace();
        let mut j = at + name.len();
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if before_ok && j < bytes.len() && bytes[j] == b'=' {
            j += 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                let q = bytes[j];
                let val_start = j + 1;
                let end = attrs[val_start..].find(q as char)? + val_start;
                return Some(decode_entities(&attrs[val_start..end]));
            }
        }
        search = at + name.len();
    }
    None
}

fn class_attr_contains(attrs: &str, class: &str) -> bool {
    match attr_value(attrs, "class") {
        Some(v) => v.split_whitespace().any(|c| c == class),
        None => false,
    }
}

fn parse_cases() -> Vec<Case> {
    let html = SUITE;
    let mut cases = Vec::new();
    let mut from = 0;
    while let Some(section) = find_element_by_class(html, from, "ajisai-case") {
        let next_from = section.outer_end;

        let id = attr_value(&section.attrs, "id")
            .expect("ajisai-case section is missing required `id` attribute");
        // Required by the suite's structure, though no assertion reads it:
        // a case without a category is malformed and fails loudly here.
        if attr_value(&section.attrs, "data-category").is_none() {
            panic!("ajisai-case `{id}` is missing required `data-category` attribute");
        }

        let inner = &section.inner;

        let source_el = find_element_by_class(inner, 0, "ajisai-source")
            .unwrap_or_else(|| panic!("case `{id}` is missing `ajisai-source`"));
        let result_el = find_element_by_class(inner, 0, "ajisai-expect-result")
            .unwrap_or_else(|| panic!("case `{id}` is missing `ajisai-expect-result`"));
        let error_el = find_element_by_class(inner, 0, "ajisai-expect-error");
        let effects_el = find_element_by_class(inner, 0, "ajisai-expect-effects")
            .unwrap_or_else(|| panic!("case `{id}` is missing `ajisai-expect-effects`"));

        // Source: decode entities, trim outer whitespace. Internal whitespace
        // (including newlines, which are significant Ajisai tokens) is kept.
        let source = decode_entities(&source_el.inner).trim().to_string();
        let expect_result = decode_entities(&result_el.inner);
        let expect_error = error_el.map(|el| decode_entities(&el.inner).trim().to_string());

        // Effects: every `ajisai-effect` inside `ajisai-expect-effects`, in order.
        let mut effects = Vec::new();
        let mut eff_from = 0;
        while let Some(eff) = find_element_by_class(&effects_el.inner, eff_from, "ajisai-effect") {
            eff_from = eff.outer_end;
            let kind = attr_value(&eff.attrs, "data-kind").unwrap_or_else(|| {
                panic!("case `{id}` has an `ajisai-effect` missing `data-kind`")
            });
            let payload = attr_value(&eff.attrs, "data-payload")
                .unwrap_or_else(|| panic!("case `{id}` effect `{kind}` is missing `data-payload`"));
            effects.push(ExpectedEffect { kind, payload });
        }

        cases.push(Case {
            id,
            source,
            expect_result,
            expect_error,
            effects,
        });
        from = next_from;
    }
    cases
}

async fn run_case(case: &Case) -> std::result::Result<(), String> {
    // The corpus observes the ordered effect sequence directly rather than
    // through rendered output text (LANG.CONFORMANCE.CORPUS).
    let mut interp = Interpreter::with_host(Arc::new(RecordingHostEnv::new()));

    let execution = interp.execute(&case.source).await;
    if let Some(expected_error) = &case.expect_error {
        match execution {
            Ok(()) => {
                return Err(format!(
                    "expected execution error containing {expected_error:?}, got success"
                ));
            }
            Err(err) => {
                let actual = err.to_string();
                if !actual.contains(expected_error) {
                    return Err(format!(
                        "error mismatch:\n  expected substring: {expected_error:?}\n  actual:             {actual:?}"
                    ));
                }
            }
        }
    } else {
        execution.map_err(|e| format!("execution failed: {e}"))?;

        // Final result = the whole stack, each slot rendered through the
        // shared surface (LANG.OBSERVATION.PROTOCOL) the CLI observes.
        let actual_result = crate::types::display::render_stack(interp.get_stack()).join(" ");
        let expected_norm = normalize_ws(&case.expect_result);
        let actual_norm = normalize_ws(&actual_result);
        if expected_norm != actual_norm {
            return Err(format!(
                "result mismatch:\n  expected: {expected_norm:?}\n  actual:   {actual_norm:?}"
            ));
        }
    }

    // Host effects in order.
    let actual_effects = interp.host_effects();
    if actual_effects.len() != case.effects.len() {
        return Err(format!(
            "effect count mismatch: expected {}, got {} ({:?})",
            case.effects.len(),
            actual_effects.len(),
            actual_effects
        ));
    }
    for (i, (exp, act)) in case.effects.iter().zip(actual_effects.iter()).enumerate() {
        if exp.kind != act.kind() {
            return Err(format!(
                "effect[{i}] kind mismatch: expected {:?}, got {:?}",
                exp.kind,
                act.kind()
            ));
        }
        if exp.payload != act.payload() {
            return Err(format!(
                "effect[{i}] payload mismatch:\n  expected: {:?}\n  actual:   {:?}",
                exp.payload,
                act.payload()
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn conformance_suite_passes() {
    let cases = parse_cases();
    assert!(
        !cases.is_empty(),
        "conformance suite parsed zero cases — the suite file is empty or malformed"
    );

    let mut failures = Vec::new();
    for case in &cases {
        if let Err(e) = run_case(case).await {
            failures.push(format!("[{}] {}", case.id, e));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} conformance case(s) failed:\n\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n\n")
        );
    }
    eprintln!("conformance: {} case(s) passed", cases.len());
}
