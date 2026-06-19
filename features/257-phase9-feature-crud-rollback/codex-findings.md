# Codex Findings (non-blocking) — #257 phase9-feature-crud-rollback

## STEP 7.5 r1 (verdict: pass, blocking: 0)

### C-F01 (medium, architect/contrarian aggregate)

- file: `crates/engawa-build/src/feature_crud.rs::rollback`
- finding: rollback は `truncate(idx)` + `Document::validate()` のみで、残った prefix の feature 参照整合性 (forward ref / missing body) を再検証していない。`Document::from_path()` は forward ref を拒否しないため、不正 `.engawa` を `entry rollback` に通すと surviving prefix が build 不能でも成功として書き戻す。
- 判断: **deferred to follow-up**。本 Issue scope (履歴 Document 純関数変換 + 基本 validate) を超える。Phase 9 後続の「履歴整合性 validator 強化」 issue (例: #270 拡張) で扱う。

### M-F01 (medium, migration)

- file: `crates/engawa-cli/tests/257_phase9_entry_rollback_cli.rs`
- finding: cli `--dry-run` 分岐が未テスト (T03 派生)。
- 判断: **deferred / 受容**。in-scope だが本 Issue では blocking でない (verdict pass)。Test 追加価値はあるが loop bandwidth 節約のためここでは見送り。
