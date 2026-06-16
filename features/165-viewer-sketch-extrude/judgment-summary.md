## Round 1

- IN01 (invariant, critical): 採用 → plan §uniqueId 戦略: Math.random() 廃止、Phase 6 `nextId(prefix, existing)` を再利用する決定的 ID 戦略に書き換え
- IN02 (invariant, high): 採用 → plan §テスト計画: T08_unit_id_determinism (nextId 決定性) を追加
- NU01 (numeric, medium): 採用 → plan §テスト計画: T04 を depth=0/負値/非数値/Infinity の 4 パターンガード網羅に拡張
- AM01 (ambig, low): 採用 (= 自動解決) → 決定的 ID 戦略採用に伴い「リトライ上限」概念が消滅、AM01 は無効化

## Round 2

- IN01 (invariant, critical): 棄却 → rejection.md 記載。本 Issue は web/ のみで Rust IdGenerator は適用外、誤指摘
