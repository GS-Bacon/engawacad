//! Acceptance tests for #238: ADR-016 (engawa CLI naming) draft.

use std::path::PathBuf;

fn adr_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("decisions")
        .join("016-engawa-cli-naming.md")
}

fn adr_content() -> String {
    std::fs::read_to_string(adr_path()).expect("ADR-016 file must exist")
}

#[test]
fn t01_determinism_read_twice() {
    let a = adr_content();
    let b = adr_content();
    assert_eq!(a, b, "ADR-016 file content must be stable across reads");
}

#[test]
fn t_doc_required_sections() {
    let content = adr_content();
    for section in &[
        "## Context",
        "## Decision",
        "## Decision Matrix",
        "## Trade-off",
        "## 採用前提崩壊 trigger",
        "## 既存 ADR との関係",
    ] {
        assert!(
            content.contains(section),
            "ADR-016 must contain section: {section}"
        );
    }
}

#[test]
fn t_doc_decision_matrix_has_options() {
    let content = adr_content();
    assert!(
        content.contains("Option A")
            && content.contains("Option B")
            && content.contains("Option C"),
        "Decision Matrix must reference Option A / B / C"
    );
}

#[test]
fn t_deg_file_missing() {
    assert!(
        adr_path().exists(),
        "ADR-016 file must exist at {}",
        adr_path().display()
    );
}

#[test]
fn t_boundary_status_accepted() {
    let content = adr_content();
    assert!(
        content.contains("**Status**: Accepted")
            || content.contains("**Status**: Proposed")
            || content.contains("**Status**: Withdrawn"),
        "ADR-016 Status must be Accepted, Proposed or Withdrawn (Issue #286)"
    );
}
