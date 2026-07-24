# claude-self-review: #295 Sketch Offset

## architect レンズ (既存 invariant / API 契約)

- `SketchElement` に `PartialEq` derive を追加した (T01 決定性テストで `assert_eq!` に必要)。既存の Serialize/JsonSchema/TS derive と組み合わせ問題なし。他 crate から `SketchElement` を比較する用途は現状ない (grep 済) ため意味的影響なし。
- `built_sketch_profiles: HashMap<String, Vec<SketchElement>>` 状態を build ループに追加。既存の `sketches` HashMap は immutable, `built_sketch_profiles` は SketchOffset 適用時のみ insert。Extrude/ExtrudeCut は `.get(sketch)` で None → `entry.profile` にフォールバック。**副作用なし** (SketchOffset を使わない既存 feature は挙動不変)。
- `Feature::SketchOffset` は body を produce しない (BuiltBodies に register しない)。既存の `is_body_producer` は変更不要 (SketchOffset は body producer ではない)。実装で確認済 (`crates/engawa-build/src/lib.rs:498-` の `is_body_producer` match arm に SketchOffset は追加されていない = 正しい)。

## contrarian レンズ (採用方針の反論可能性)

- `selection: Vec<String>` 空 = 全要素 offset のセマンティクス: CAD 業界標準に合致 (SolidWorks/Fusion360 で "Offset entire sketch" 操作は selection なし相当)。棄却案 (空 = エラー) は API surface が煩雑になる。plan §In-Scope で明記済。
- `distance` が sketch plane で "left" = 正の方向。逆 (右 = 正) の慣習を採る CAD 実装もある。ADR-017 の spec に符号規約が明示されていない (見落とし)。**medium 懸念**: 別 Issue で ADR-017 addendum を書くべきか。現状 plan.md 内で明記しているが、外部利用者向けドキュメントには載せていない。
- ADR-018 の `<=` 統一規約に従い `distance.abs() <= LENGTH_TOLERANCE` で no-op 判定。境界テスト T_DEG_zero_distance_boundary で `distance = LENGTH_TOLERANCE` ちょうどが no-op になることを確認済。

## migration レンズ (既存テスト互換 / public API 破壊)

- 新 Feature variant 追加は既存 YAML doc の parse を破壊しない (variant 追加は additive)。ADR-017 で bump 済の schema_version v2 のまま。
- `SketchElement` の `PartialEq` 追加は additive derive (既存コード非破壊)。web/src/generated/Feature.ts への波及も maybe-commit-generated-ts.ts で処理済。
- xtask の `FEATURE_GOLDEN` に sketch_offset variant を追加した (line 1041, 1045, 1049 に trailing space を含む)。他 golden test への影響なし。
- 既存 example fixture (`examples/*.engawa`) の smoke test はすべて緑。`examples_smoke.rs` に sketch_offset.engawa 用エントリを追加済。

## 結論

- architect: 弱点なし (state 追加は opt-in、副作用なし)
- contrarian: **medium 1件**: 符号規約が ADR-017 に明記されていない。**受容** (別 Issue で ADR-017 addendum。今回の plan.md § 数値モデルで plan-local 明示済)
- migration: 弱点なし (variant 追加は backwards-additive)

**弱点なし → Codex 7.5 へ進む**。medium 1 件は plan-level で吸収済、Codex 3 persona の refute には至らない見込み。
