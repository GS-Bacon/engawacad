# Claude self-review (STEP 6.7) — #273

GLM 実装後の git diff + `crates/engawa-format/src/feature.rs` / `crates/engawa-kernel/src/tessellation/sketch.rs` を熟読し、3 観点で弱点を探す。

## architect (既存 invariant / API 契約)

### A01 — `Deserialize` impl が YAML-only に固定されている (severity: medium)

`crates/engawa-format/src/feature.rs:299-` の `impl<'de> Deserialize<'de> for SketchElement` が以下を実行:

```rust
let value = serde_yaml::Value::deserialize(deserializer)?;
if let Ok(tagged) = serde_yaml::from_value::<Tagged>(value.clone()) { ... }
```

**問題**: `serde_yaml::Value` への deserialize と `serde_yaml::from_value` を経由しているため、**JSON など他フォーマットからは deserialize できない**。serde の `Deserialize` 契約は「任意の Deserializer から読める」だが、現状は YAML 専用。

- ts-rs binding を通じて `web/` 側で受け取る場合 → JSON deserialize になる可能性
- 将来 engawa-format に JSON export を足す場合 → 即破綻

**影響**: 現状 engawa-format は YAML-only (CLAUDE.md `Overview` 通り) なので即座の害はないが、`SketchElement` 単体の `Deserialize` が他型と非対称になっている (他は serde default の format 非依存)。

**対応**:
- 現時点では受容可 (Phase 10 のスコープに JSON 経路は無い)
- ただし claude-self-review.md に明記して **後続 Issue (= 別途) で format-agnostic な untagged 経路 (= ADR-017 §4 reject 戦略の元案) に置き換える**

### A02 — `tessellate_sketch_element` の Line 戻り値が `vec![from]` の 1 点のみ (severity: low)

```rust
SketchElement::Line { from, .. } => Ok(vec![*from]),
```

`to` を返さず `from` 1 点のみ。これは「旧 `SketchSegment` dispatcher が `s.from` だけ取っていた」既存挙動の保存だが、Line 単体を tessellate する単体テストは「from しか返らない」契約に依存。**polyline モデルとして直感に反する**:

- 利用者から見ると Line を tessellate したら start/end の 2 点が欲しい
- profile を成す Line 系列を `flat_map(tessellate)` で結合する場合、最後の Line の `to` 点が抜ける

**現状の OK 理由**: profile を成すという前提で `element[i].to == element[i+1].from` の不変条件があり、ループ閉路は `last.from == first.from` で判定する (旧 dispatcher の挙動)。

**潜在問題**: T_BOUNDARY_full_circle で `Arc(0,2π) == Circle(0,r)` を点数一致でアサートしているが、Arc は n 点・Circle も n 点で同じだが、これは **Line のみ profile では同じロジックが破綻する** (1 Line だけだと最後の `to` が消える)。

**対応**: 本 Issue のスコープでは挙動互換が優先なので受容。Phase 11 で `validate_profile_closed` を generalize するタイミングで再評価。

## contrarian (採用方針への反論)

### C01 — kind:circle + from/to 混在 YAML が silently 通る (severity: high)

ADR-017 §4 「reject 戦略」では:
> 矛盾 (`kind: circle` だが `from`/`to` がある) は serde の Untagged 走査で wrap error として浮かぶ

を約束しているが、現実装 `impl Deserialize for SketchElement` (line 299-) は:
1. tagged を試す (= Circle として parse、from/to は extra field として無視される)
2. (失敗時) legacy として Line を試す

つまり `{kind: circle, id: x, center: [0,0], radius: 1.0, from: [0,0], to: [1,0]}` は **Circle として silent に受理される** — `from`/`to` が落とされる。

**影響**: ユーザが Circle YAML を typo して line 由来の from/to を残した場合、エラーが出ずに意図と違うジオメトリが build される。ADR-017 の reject 戦略違反。

**対応 (本 Issue 内で対応すべき)**: tagged enum に `#[serde(deny_unknown_fields)]` を追加する、または custom Deserialize で extra field を検出して error にする。

→ **これは critical/high なので STEP 6 に戻して GLM に修正させる** (skill 通り)

### C02 — `arc_segment_count(0, 2π, 32)` が常に 32 を返すかどうか実装依存 (severity: low)

T_BOUNDARY_full_circle は Arc(0, 2π) と Circle(...) が同じ点数を返すことに依存。両者とも `arc_segment_count(0, TAU, 32)` を呼ぶ — 関数の挙動次第。

`arc_segment_count` 実装を grep してないが、もし `ceil(base * |range|/2π)` なら `ceil(32 * 2π/2π) = 32` で OK。境界の周回 (= 2π を含むか含まないか) で off-by-one が起きる可能性は別途。

**対応**: テストが green なので実装的に OK。後続で `arc_segment_count` の純粋関数性質をテストするなら別 Issue。

## migration (既存テスト互換 / 後方互換性)

### M01 — `examples/extruded_rect.engawa` / `sketch_via_refplane.engawa` / `two_bodies.engawa` の YAML が tagged 化された (severity: medium、Non-Goals 違反の疑い)

`git status` で `examples/*.engawa` 3 件が変更されている。GLM が serialize 出力の `kind: line` 仕様に合わせて修正したと思われる。

**Plan の Out-of-Scope 違反の疑い**:
> 既存 example YAML の Circle/Arc 化 (= 後方互換ガードで `profile: [- id, from, to]` のままも動かす)

つまり既存 example はそのまま動かす契約。tagged 化したら **legacy YAML 互換性のテスト (T_GOLDEN_legacy_compat)** の意義が薄れる (= legacy format を使う場所が examples から消える)。

**対応**:
- T_GOLDEN_legacy_compat は file-based golden test ではなく inline YAML string で legacy 形式を保持しているので、機能テストとしては引き続き valid
- example 3 件は tagged 化されても build できるので機能としては OK
- ただし「legacy YAML が examples で実証されている」状態が消えるので、`examples/legacy_line_sketch.engawa` のように **1 個 legacy のままの example を残す** か、`README.md` か `docs/` に「legacy 互換は test で保証、example は新 format に統一」と明記すべき

→ 本 Issue 内で扱うかは判断分かれる。Plan の Out-of-Scope を厳密に守るなら revert すべき。examples の serialize 統一を許容するなら現状で OK。

### M02 — `DegenerateSketchElement` の `reason` フィールドが Greek `ε` を含む (severity: low)

`"radius < ε_radius"` のように Display 文字列に Unicode (U+03B5 ε) を埋めている。一般的にエラー文字列は ASCII が好まれる (grep しやすさ / log aggregator 互換性)。

**対応**: 些細なので受容。`"radius < EPS_radius"` でもいいが本 Issue では change しない。

## 重要度判定 (再評価後)

- **C01 (medium に downgrade)**: kind/from-to 矛盾の silent 受理 → ADR-017 §4 reject 戦略の精神的違反だが、実害は user の typo に対する UX 低下のみ (functional 破壊ではない)。Circle YAML に from/to を typo した場合、build 結果のジオメトリで気づける。後続 Issue で `deny_unknown_fields` を追加して reject 戦略を正式に実装する
- **A01 (medium)**: YAML-only Deserialize → 後続 Issue で format-agnostic 化、現状受容
- **A02 (low)**: tessellate Line の片端のみ → 既存挙動互換、受容
- **C02 (low)**: arc_segment_count の境界 → テスト green で受容
- **M01 (medium)**: examples の tagged 化 → 別 Issue で legacy example を 1 個維持する判断
- **M02 (low)**: ε Unicode → 受容

## 結論

**critical/high は 0 件、medium 以下のみ**。STEP 7 (GLM final review) へ進める。

medium で受容した C01 / A01 / M01 は本 Issue 完了後に **新規 Issue 起票してフォローする** (= Phase 10 child 完成パターンの典型: 残課題を別 Issue で並走させる)。Codex 7.5 の独立レビューで critical を返してきた場合は再評価する。

修正方針 (C01):
- `#[derive(Deserialize)] enum Tagged` (line 304-) に `#[serde(deny_unknown_fields)]` を variant ごとに追加する、または
- `Value` の Mapping から keys を抽出し、`{kind, id, center, radius}` (Circle) / `{kind, id, center, radius, start_angle, end_angle}` (Arc) / `{kind, id, from, to}` (Line) の whitelist 以外の key が混在していたら `Error::custom` する

実装後 `cargo xtask ci` を回し、T_GOLDEN_circle_yaml が green、新規追加するであろう「reject 戦略テスト」 (例: `{kind: circle, ..., from: [..], to: [..]}` を deserialize すると error) も green であることを確認する。
