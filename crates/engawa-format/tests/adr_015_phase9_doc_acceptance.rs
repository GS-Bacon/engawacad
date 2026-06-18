//! Acceptance tests for #237: ADR-015 (Phase 9 design foundations) draft.
//!
//! テスト ID は features/237-phase9-adr-adr-draft-crud/plan.md と対応。

use std::path::PathBuf;

fn adr_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("decisions")
        .join("015-phase9-design-foundations.md")
}

fn adr_content() -> String {
    std::fs::read_to_string(adr_path()).expect("ADR-015 file must exist")
}

#[test]
fn t01_determinism_read_twice() {
    let a = adr_content();
    let b = adr_content();
    assert_eq!(a, b, "ADR-015 file content must be stable across reads");
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
            "ADR-015 must contain section: {section}"
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
    let path = adr_path();
    assert!(
        path.exists(),
        "ADR-015 file must exist at {}",
        path.display()
    );
}

#[test]
fn t_boundary_status_accepted() {
    let content = adr_content();
    let has_accepted = content.contains("**Status**: Accepted");
    let has_proposed = content.contains("**Status**: Proposed");
    assert!(
        has_accepted || has_proposed,
        "ADR-015 Status must be Accepted or Proposed"
    );
}
