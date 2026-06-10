# Codex Low-Severity Findings (non-blocking)

## Loop 3 — verdict: pass

### F01 (low): unused imports in test code
- `crates/mycad-kernel/src/tessellation/mod.rs:2769`: `use crate::geometry::Vec3;` unused in test_adjacent_face_idx_sphere_cap
- `crates/mycad-kernel/tests/cross_face_nu_acceptance.rs:23`: `use serde_yaml;` redundant (crate accessible without explicit use)
- Impact: `cargo clippy --tests -- -D warnings` fails; `cargo xtask ci` (no --tests) passes
- Action: Record for future cleanup; not blocking this merge
