# GLM Self-Review for #298 (Round 4 — Codex A01 / self-review R2-2 解消)

## スコープ

Round 4 は STEP 7.5 Codex final gate (A01) と STEP 6.7 Claude self-review (R2-2) が
独立に検出した「`SketchMirror` element-level gate の false-reject 既知制約が
コメント・回帰テストで文書化されていない」指摘の解消のみ。アーキテクチャ修正は
#331 に委譲済みのため**コメント追記 + 回帰テスト1本の追加のみ**がスコープ。

実装ロジック(`sketch_mirror.rs`, `feature_crud.rs` gate 本体)は Round 1-3 で
確定済み。本 self-review は追加コメント1件・追加テスト1件に限定する。

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

- 追加した回帰テスト `t_known_limitation_mirror_derived_elem_false_reject` は
  `sketch_chamfer_acceptance.rs::t10_crud_gate_known_limitation_false_reject` と
  同型: (a) build は current profile 基準で成功、(b) CRUD gate は元 CreateSketch
  profile 基準で `sketch_mirror_elem_not_found` 拒否。既存 `FeatureCrudError::SketchElementNotResolved`
  variant を再利用し、新規エラー型は導入していない
- コメント追記は `feature_crud.rs::check_refs_resolve_before` 内の既存 gate 直上。
  Fillet/Chamfer の同種コメント(行 307-312, 775-780)と同じ「Known limitation (false-reject)」形式で
  記載し、`#331` への参照を入れた。アーキテクチャ決定自体は本 Issue では変更しない

弱点 / リスク: `feature_crud.rs:821-836` のコメントが長い (>15行)。Fillet/Chamfer の
コメントが 4-6 行である点を考慮すると、後続読者が「重要な契約」と誤認する可能性がある。
実際には 既知の false-reject で #331 委譲なので、コメント簡素化は #331 解決時に行うべき

## contrarian 観点 (採用した実装方針の反論可能性)

- **テストを追加せずコメントのみで済ませる選択肢**もあった。却下: Fillet/Chamfer には
  `t10_crud_gate_known_limitation_false_reject`/`t13_*` (chamfer) の両方が存在し、
  Mirror だけ回帰テストがないと「なぜ Mirror はこの制約を文書化しないのか」と未来の
  レビュワーに疑義を立てる。テスト1本のコストは小さい
- **コメントではなく plan.md Non-Goals への参照のみで済ませる選択肢**もあった。却下:
  plan.md は実装時に読まれるが、CRUD gate を修正する未来の開発者は `feature_crud.rs`
  を開いて初めて当該コードに当たる。コード側のインラインコメントの方が到達性が高い
- 直前 Issue (#297 Chamfer R01 修正) で導入した element-level gate の semantic を
 退化させていないか → 否。本ラウンドはコメントとテストのみで gate ロジック自体は不変更

## migration 観点 (既存テスト互換 / 後方互換性)

- 触った public API なし。`Feature::SketchMirror` variant の shape も不変更
- 既存 acceptance test 21件は全て pass 維持(後退なし)。新規テスト1件追加のみで
  既存テストの改変は行っていない
- `examples/sketch_mirror.engawa` その他 golden ファイルも不変更

## 残課題 (scope-defer / 後続 Issue 候補)

- **根本修正は #331**: CRUD gate が元 CreateSketch profile 基準なのに対し build は
  current profile 基準、というズレ自体は Mirror 由来ではなく Fillet/Chamfer/Offset
  横断のアーキテクチャ起因。本 Issue では文書化のみ行い、#331 で横断対応予定
- **派生 id 命名衝突** (Mirror の連鎖、#332 委譲済み): `{elem_id}_mirror` 派生 id が
  固定なため、同じ要素を2回 Mirror すると `mirror_duplicate_id` で必ず fail する。
  本 Issue の Non-Goals に明記済みだが、#332 で feature id 込み命名に拡張予定

---

# GLM Self-Review for #298 (Round 3 — 項目 6・7 のみ)

## スコープ

Round 3 は残件の項目 6 (`golden_examples.rs` への `golden_sketch_mirror` 追加) と
項目 7 (`xtask` の tag assert 一覧へ `sketch_mirror` 追加) のみ。
実装ロジック(`sketch_mirror.rs`, `feature_crud.rs`)は Round 2 で確定済みのため
本 self-review は追加テスト2件に限定する。

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

- `golden_sketch_mirror` は既存 `golden_sketch_offset`/`golden_sketch_fillet`/`golden_sketch_chamfer`
  と同一パターンの byte-identical golden テスト。`assert_golden()` helper に頼り、
  新規 invariant や独自検証は導入していない
- `t02_feature_tagged_union` の assert 追加は単に `actual.contains(...)` を1行増やしただけ。
  tagged union の形式検証そのものは既存ロジックに一任

弱点 / リスク: なし — 既存パターンの機械的拡張で、新規契約は無し

## contrarian 視点 (採用した実装方針の反論可能性)

- YAML を `concat!` マクロでハードコードせず、`examples/sketch_mirror.engawa` から
  動的に読み込んで roundtrip 結果を別ファイルに snapshot として保持する方針もあった。
  あえて却下: 既存3本 (offset/fillet/chamfer) が byte-identical `concat!` 方式なので
  一貫性を優先した。insta 等の snapshot ライブラリ導入は本 Issue の scope 外
- golden YAML から `offset: 0.0` と `selection: []` が skip されるのは
  `CreateSketch.offset` の `skip_serializing_if = "is_zero"` と
  `SketchMirror.selection` の `skip_serializing_if = "Vec::is_empty"` に依存する。
  これらの serde attribute が将来緩和されると golden が壊れるが、それは本体側の契約変更で
  本テストの責務範囲外

## migration 観点 (既存テスト互換 / 後方互換性)

- 触った public API なし。`Feature::SketchMirror` variant 自体は Round 1 で追加済み
- 既存 acceptance test の改変なし。`examples_smoke.rs::sketch_mirror()` は前 round で
  追加済みで本ラウンドでは未触及

## 残課題 (scope-defer / 後続 Issue 候補)

- `selection` や `suppressed` が非デフォルト値の SketchMirror を含む golden example は
  本 Issue では追加していない (現在の `examples/sketch_mirror.engawa` は空 selection・false suppressed)。
  フィールドシリアライズの具体的検証は `feature.rs` 内の単体テスト (`t_sketch_mirror_roundtrip` 等) で担保済み
- Phase 10 の他 feature と同様、`examples_sketch_mirror` 相当の独立 acceptance test ファイルが
  将来的に golden を分割管理したい場合は検討余地あり (現状は `examples_smoke.rs` に統合)
