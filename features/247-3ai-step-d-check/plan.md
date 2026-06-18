# Plan: fix(3ai) GLM reviewer に層境界・公開 API ガイド注入 (Issue #241 false alarm 対策)

## 自律判断ログ (Claude 直接実装方針)

本 Issue は `.claude/skills/3ai/agents/glm-reviewer-*.md` (GLM 設計レビュアー prompt) の改訂。
`crates/**` を一切触らないため guard-crates 対象外、Claude が直接 Write/Edit する。

`project_3ailoop_implementation_style` の原則に従い 3ai prompt 改訂は循環リスクがあるため GLM dispatch は使わない (GLM が自分自身の prompt を書き換える self-modification 回避)。STEP 6/6.6 の GLM dispatch を Claude 直接 Write に置換、STEP 7 (GLM final review) と STEP 7.5 (Codex 独立 review) は通常通り走らせる (= 第三者 review は保持)。

### Issue 背景の事実確認 (#241 round 2 の検証)

`features/241-phase9-document-sketch-variable/review-invariant-r2.yaml` を Read:

```yaml
issues:
  - id: IN01
    severity: critical
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使った決定的 ID 生成になっている"
    suggestion: "IdGenerator::next() に置き換えること"
verdict: pass
```

- **finding と suggestion が自己矛盾**: 「IdGenerator を使った…になっている」(=既に OK) と書きながら suggestion で「IdGenerator::next() に置き換えること」と要求している。
- **層誤認**: `engawa-format` 層には `IdGenerator` は存在しない (`grep -rn "IdGenerator" crates/engawa-format/src/` の結果が空)。Variable は serde YAML schema 型で `Variable { name: String, expr: String }` (user 文字列)。`IdGenerator` は `engawa-kernel::brep::topology` 専用 (B-rep EntityID 採番)。

同 round の SC01 (review-scope-r1.yaml) も Issue 本文の ADR 番号誤記を「plan の問題」と誤認しており、同根の層越境 / context 混同。

### 根本原因

GLM reviewer prompt (`glm-reviewer-{scope,invariant,ambig,numeric,final,assembly}.md`) は CAD カーネルの一般論 (B-rep, IdGenerator, Euler-Poincaré 等) を観点として列挙しているが、**どの crate にどの型・API が存在するか**を限定列挙していない。結果として:

1. `engawa-format` の設計レビュー中に「`engawa-kernel` の IdGenerator を使え」など層越境の指摘を出す
2. 存在しない API (`IdGenerator::next()` を `Variable.name` に適用) を要求する空想 API 参照
3. Issue 本文に書かれた ADR 番号と plan が参照する ADR 番号の食い違いを「plan の問題」と誤認

これらは GLM が context として持っているのは plan.md と Issue 本文のみで、crate 構造を持っていないことに起因する。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `glm-reviewer-{scope,invariant,ambig,numeric,final,assembly}.md` の 6 ファイルに共通の「クレート層境界 & 公開 API（必ず参照すること）」セクションを追加 | Codex reviewer (`codex-design-reviewer.md` / `codex-final-reviewer.md`) への同セクション追加 (別 Issue) |
| 各 crate の責務 / 主要公開型 / 「この crate に存在しないもの」(誤認頻発項目) を限定列挙 | dispatch スクリプト (`dispatch-glm-review.ts`) 側での crate 構造自動注入 (静的 markdown で固定) |
| `Variable.name` が user 文字列で IdGenerator は engawa-format に存在しない旨を明示 | persona ごとの観点定義そのものの再設計 (現行観点はそのまま) |
| 既存 `## スコープ規律` の severity 規律と整合する追加文言 (層越境指摘も「過剰指摘の禁止」に含める) | GLM judge 集約ロジック (`state.ts judge` / `check-early-stop`) の改訂 |
| 6 ファイルの diff を `cargo xtask ci` で markdown 構文影響無いことだけ確認 (実プロンプトの A/B 比較は別 Issue) | 既存 `features/*/review-*.yaml` の retro-active 修正 (Issue #241 round 履歴はそのまま) |

## Non-Goals

- Codex reviewer prompt 側への同セクション追加 (別 Issue で扱う)
- 層越境を機械的に検出する lint script (静的 prompt 改善で済むため過剰)
- `engawa-api` / `engawa-viewer` の責務再定義 (Phase 21+ で別途 ADR)
- per-persona の crate 限定 (全 persona に同じセクションを置く。invariant が `engawa-kernel` に集中し scope が `engawa-format` を見るのは現状の prompt 観点で十分制御されている)
- GLM が prompt を読み飛ばす確率の定量評価 (#241 のような false alarm が再発しないかを後続 cycle で観察すれば足りる)
- `crates/<crate>/tests/<feature>_acceptance.rs` の skeleton 追加 (本 Issue は `crates/` を一切触らないため Rust acceptance test の concept が適用できない。代わりに state shim で `acceptance_skeleton passed` をセットし STEP 6 ゲートを通す)

## 実装対象

### A. 共通「クレート層境界 & 公開 API」セクション (新規追加)

各 reviewer prompt の `## スコープ規律` の **直前** に挿入する (= 観点列挙の後、severity 規律の直前)。これにより観点を読んだ後に「ただし以下の層境界を踏み外すな」が来る順序になる。

セクション本文 (6 ファイル共通テンプレート、完全同一文字列で注入):

```markdown
## クレート層境界 & 公開 API（必ず参照すること）

EngawaCAD は以下 6 crate のフラット構成。レビュー対象 plan / diff がどの crate を触っているかを必ず特定し、**その crate に存在しない型・API を要求する指摘は出さないこと**。

| crate | 責務 | 主要公開型・関数 (必須に限定) |
|-------|------|--------------------------------|
| `engawa-format` | `.engawa` YAML schema + parser (serde 層) | `Document`, `Component`, `Feature` (enum), `Variable { name: String, expr: String }`, `RefPlane`, `EvalError`, `FormatError`, `from_yaml`, `to_yaml`, `schema_version` |
| `engawa-kernel` | B-rep トポロジー + 幾何 + tessellation | `IdGenerator`, `Solid`, `Shell`, `Face`, `Loop`, `Edge`, `HalfEdge`, `Vertex`, `Point`, `Vec3`, `Plane`, `Curve`, `Surface`, `make_cuboid`, `make_cylinder`, `make_sphere`, `tessellate_solid`, `TriangleMesh`, Boolean op (`boolean_*`) |
| `engawa-build` | Feature ディスパッチャ (format → kernel) | `build_document(&Document) -> Result<Vec<Solid>, _>`, 各 Feature variant に対する dispatch arm |
| `engawa-api` | HTTP API server (axum) | `Router`, `handler::*`, `state::AppState`, `transport::*` (Phase 8+ で導入) |
| `engawa-cli` | CLI バイナリ (`engawa`) | `main.rs`, `view.rs`。ロジックは持たない (kernel/build/format に委譲) |
| `engawa-viewer` | 3D viewer (未実装) | (Phase 21+ で実装) |

### この層には存在しないもの (誤認頻発項目)

- `engawa-format` には **`IdGenerator` は存在しない**。`Variable.name` / `Component.name` / `Sketch.name` 等の文字列はすべて user 入力 (YAML から読まれる文字列)。決定性が要求されるのは **`engawa-kernel` で生成される EntityID** であり、`engawa-format` 層では「YAML を 2 回 parse → 同一 Rust 構造体」が決定性の定義 (= serde の決定性に委ねる)。`engawa-format` の plan に対して「`IdGenerator::next()` を使え」と指摘するのは層越境の誤り。
- `engawa-kernel` は **レンダリング非依存**。`TriangleMesh` は出力するが GPU buffer / wgpu / winit には触らない (`engawa-viewer` の責務)。
- `engawa-build` は **Feature → kernel の薄い dispatcher**。ここに B-rep ロジックや YAML 解釈ロジックを書くのは越権。
- `engawa-cli` は **ロジックを持たない**。アルゴリズム実装の plan / diff がここに集中していたら層越境を疑う。

### Issue 本文 vs plan の参照齟齬の扱い

Issue 本文が古い ADR 番号 / 廃止 API を参照していて plan 側が正しく更新済みの場合、それは **plan の問題ではない** (= 指摘しない)。Issue 本文の修正は別途 user / loop が行う。レビュー対象は **plan の整合性** であり、Issue 本文の正誤ではない。
```

### B. 既存「スコープ規律（過剰指摘の禁止）」への追記

各 reviewer prompt の `## スコープ規律（過剰指摘の禁止）` 末尾に 1 行追加:

```markdown
- 上記「クレート層境界 & 公開 API」を踏み外した指摘 (存在しない型 / API の要求、層越境の責務押し付け) は出さない
```

### C. 適用ファイル一覧

```
.claude/skills/3ai/agents/glm-reviewer-scope.md
.claude/skills/3ai/agents/glm-reviewer-invariant.md
.claude/skills/3ai/agents/glm-reviewer-ambig.md
.claude/skills/3ai/agents/glm-reviewer-numeric.md
.claude/skills/3ai/agents/glm-reviewer-final.md
.claude/skills/3ai/agents/glm-reviewer-assembly.md
```

6 ファイルに同一の Section A を追加 (差分が完全に同じになるため diff レビューも容易)。

## 数値モデル

該当なし (markdown プロンプト改訂のため数値不変条件 / tolerance / 退化判定は無関係)。

## 幾何的不変条件チェックリスト

- Boolean op: N/A
- Partition: N/A
- Assemble: N/A

(本 Issue は prompt 改訂のため B-rep に触れない)

## テスト計画 (ID 付き)

GLM reviewer prompt の改訂は **runtime テストできない** (実際の GLM 出力を A/B 比較するには別 Issue 規模の harness が必要)。本 Issue は静的整合性を以下で担保する:

| ID | 種別 | 検証 | 方法 |
|----|------|------|------|
| T01_determinism | 決定性 (N/A) | prompt は静的 markdown でランダム性なし | 該当なし — markdown ファイルの diff のみ |
| T02_files_inject | 構造 | 6 ファイル全てに Section A が **完全に同一文字列で** 注入されている | `for f in glm-reviewer-{scope,invariant,ambig,numeric,final,assembly}.md; do grep -c "クレート層境界" "$f"; done` で全 1 を確認 |
| T03_scope_rule_appended | 構造 | 6 ファイル全てに Section B の追記 1 行 (`上記「クレート層境界 & 公開 API」を踏み外した指摘`) が存在 | `grep -l "「クレート層境界 & 公開 API」を踏み外した指摘" .claude/skills/3ai/agents/glm-reviewer-*.md` で 6 件 |
| T04_yaml_format_unchanged | 構造 | 6 ファイルの「出力フォーマット（厳守）」コードブロックは無変更 | `grep -c "出力フォーマット（厳守）" *.md` で全 1 + diff で当該ブロックが変わってないこと |
| T_DEGEN_no_collateral | 退化境界 | 既存の「PRIOR REJECTIONS / PRIOR JUDGMENTS / SCOPE DEFENSE」関連の規律文言が削除されていない | 各ファイルで `grep -c "PRIOR REJECTIONS\|PRIOR JUDGMENTS\|SCOPE DEFENSE"` を改訂前後で同一 (3) |
| T_BOUNDARY_no_idgen_for_format | 境界 | Section A 本文に「`engawa-format` には `IdGenerator` は存在しない」旨が明示されている | `grep "engawa-format.*IdGenerator は存在しない" .claude/skills/3ai/agents/glm-reviewer-invariant.md` でヒット |

## 検証手順

```bash
# 1. CI 確認 (markdown のため Rust ビルドに影響しないことを確認)
cargo xtask ci  # 既存と同じ green

# 2. 構造テスト
for f in scope invariant ambig numeric final assembly; do
  c=$(grep -c "クレート層境界" .claude/skills/3ai/agents/glm-reviewer-$f.md)
  [ "$c" = "1" ] || { echo "FAIL T02 $f=$c"; exit 1; }
done; echo PASS-T02

n=$(grep -l "「クレート層境界 & 公開 API」を踏み外した指摘" .claude/skills/3ai/agents/glm-reviewer-*.md | wc -l)
[ "$n" = "6" ] && echo PASS-T03 || { echo "FAIL T03 n=$n"; exit 1; }

grep "engawa-format.*IdGenerator は存在しない" .claude/skills/3ai/agents/glm-reviewer-invariant.md > /dev/null \
  && echo PASS-T_BOUNDARY || { echo "FAIL T_BOUNDARY"; exit 1; }
```

これらは shell 1 行で完了するため `crates/<crate>/tests/` への acceptance skeleton 追加は不要 (Rust テスト harness が存在しないファイルの構造検証)。STEP 5.5 の acceptance_skeleton ゲートは **state shim** で passed をセットして通す:

```bash
bun .claude/skills/3ai/scripts/state.ts set \
  features/247-3ai-step-d-check/state.json acceptance_skeleton passed
```

## STEP 7.5 (Codex) で見るべき観点

- 6 ファイル間で Section A の文字列が **完全に一致** しているか (typo / 抜けで層境界ガイドが片寄ると効果半減)
- Section A の追加位置が `## スコープ規律` の **直前** (= prompt の最後の指示) に来ているか確認 (中盤に挿入すると後続セクションに埋もれる)
- 既存 `## スコープ規律` の severity 規律と矛盾しないか
- 6 crate の責務記述が CLAUDE.md / `crates/*/Cargo.toml` と整合しているか
