# Codex Non-Blocking Findings — #93

## Round 3 (final pass)

| ID | Severity | File | Finding | Action |
|----|----------|------|---------|--------|
| F01 | medium | handler.rs:63 | `Json<Feature>` rejection が `ErrorResponse` JSON でなく plain-text 4xx を返す（axum 既定）。既存 API の JSON エラー契約と不整合。 | 将来の Issues で custom extractor / `JsonRejection` 捕捉を検討。本 Issue スコープ外（Non-Goals: 全バリデーション網羅）。 |
| F02 | medium | post_features_acceptance.rs:262 | E03/E04 が 500 も許容しており退化入力回帰検知として弱い。 | 将来 Issue で `assert_eq!(422)` に絞る。`make_cuboid(0.0,...)` の KernelError→422 パスは既存 From impl で保証済み。 |
