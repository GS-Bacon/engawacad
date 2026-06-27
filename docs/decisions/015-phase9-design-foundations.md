# ADR-015: Phase 9 設計基盤 (履歴 CRUD 抽象 + Variable スコープ + schema_version + 品質基盤)

**Date**: 2026-06-18
**Status**: Proposed
**Related**: ADR-002 (ロードマップ・ラベル運用), ADR-006 (Issue 粒度), ADR-007 (アセンブリ参照), ADR-010 (Sketch input model — serde default + 互換維持の先例), ADR-013 (ADR 自動 accept フロー), ADR-014 (Component RefPlane 隔離)
**Resolves**: Issue #237 (parent #194 split — Phase 9 起点)

---

## Context

Phase 9 (CAD カーネル成熟期の起点) で固める設計基盤を 1 つの ADR にまとめる。本 ADR の決定は、`#194` の split-child である #239 / #240 / #241 / #242 / #243 の各実装 Issue の前提となる。実装に着手する前に方針を確定しておかないと、各 Issue が個別に判断を持ち寄って後段の Phase で矛盾する。

具体的に固める 4 項目:

1. **Feature CRUD API 抽象** — Edit / Roll back / Suppress / Reorder / Delete / Insert の 6 op を `engawa-build` でどう表現するか
2. **Variable 2 段スコープ** — Document 全域 Variable と Sketch 内 Variable の名前空間分離規約と参照構文
3. **`schema_version` + MigrationHook 形状** — フォーマット進化の入口
4. **品質基盤ツール選定** — proptest / criterion / cargo-fuzz / cargo-llvm-cov / Playwright の役割と最小 setup 範囲

## Decision

本 ADR では Phase 9 設計基盤の 4 項目 (Feature CRUD API 抽象 / Variable 2 段スコープ / `schema_version` + MigrationHook / 品質基盤ツール選定) を決定する。各項目の採用 option と Trade-off を `## Decision Matrix` 表に集約しており、本セクションでは代表項目 (Feature CRUD API) の Options 要約をここで明示し、残り 3 項目は §1〜§4 のサブセクション + Decision Matrix を参照のこと。

### Options 要約 (Feature CRUD API 抽象)

- A. 個別関数 (`edit_feature()` / `delete_feature()` ...) — Pros: 既存実装の最小拡張。Cons: 呼び出し点が散らばり CRUD 横断の不変条件 (`feature_id` 一意性等) を強制しづらい
- B. `FeatureOp` enum + `Document::apply_op()` 中央ディスパッチ (採用) — Pros: Undo/Redo を統一 (各 op が `inverse(&self) -> FeatureOp` を返す)、CLI verb と 1:1 対応。Cons: variant 追加が breaking (`#[non_exhaustive]` で forward-compat 確保)
- C. `Box<dyn Command>` (Command pattern, trait object) — Pros: collection に持てる。Cons: dyn 越境で型情報欠落、Serialize/Deserialize と相性が悪い

**Trade-off**: Option B は variant 追加が breaking だが `#[non_exhaustive]` で forward-compat 確保可能、Option A/C より中庸 (B 採用)。

### 1. Feature CRUD API 抽象

採用: **`FeatureOp` enum + `Document::apply_op(op: FeatureOp) -> Result<(), BuildError>` 中央ディスパッチ**

```rust
pub enum FeatureOp {
    Insert { component_path: ComponentPath, index: usize, feature: Feature },
    Edit   { component_path: ComponentPath, feature_id: String, mutate: Box<dyn FnOnce(&mut Feature)> },
    Delete { component_path: ComponentPath, feature_id: String },
    Reorder { component_path: ComponentPath, feature_id: String, new_index: usize },
    Suppress { component_path: ComponentPath, feature_id: String, suppressed: bool },
    RollBack { component_path: ComponentPath, feature_id: String /* 直前まで含めて再生成 */ },
}
```

理由:
- 6 op を 1 enum でまとめると **Undo/Redo を統一的に扱える** (各 op が `inverse(&self) -> FeatureOp` を返せばよい)
- `engawa-cli` の `engawa entry <op>` コマンド (#242) が enum variant に 1:1 対応するので、CLI ⇔ build を疎結合に保てる
- 個別関数 (Option A) は呼び出し点が散らばって CRUD 横断の不変条件 (e.g. `feature_id` 一意性) を強制しづらい

### 2. Variable 2 段スコープ

採用: **Document Variable は `${var_name}`、Sketch Variable は `${var_name}` (同構文)。lookup 順は Sketch (近) → Document (遠) で shadowing**

参照構文: `${name}` のみ。`@`/`{{}}`/`$()` 等は候補から外す (YAML 既存予約と衝突しない `${}` を採用)。

Sketch 内で同名 Variable を定義した場合、Sketch ローカルが優先される (近スコープ優先 = 一般的なレキシカルスコープ規約)。

理由:
- 2 段スコープで足りる: Component を越えるグローバル変数は Phase 9 では Out-of-Scope (Phase 10+ で `Document.shared_variables` 等を導入する余地)
- `${}` は YAML/Rust/JavaScript すべてで substitution 構文として認知され、独自記法より人間が読める
- shadowing は「Sketch ローカルで簡単に上書きできる」UX を可能にする

### 3. `schema_version` + MigrationHook

採用: **`Document.schema_version: u32` + `MigrationHook { fn migrate(&self, from: u32, to: u32, doc: &mut Document) -> Result<(), FormatError> }`**

`schema_version` 命名規則:
- u32 単調増加 (v1 → v2 → v3)
- 未指定 = v1 (`INITIAL_SCHEMA_VERSION = 1` 固定)
- `CURRENT_SCHEMA_VERSION` < `schema_version` → `FormatError::UnknownSchemaVersion` で reject (downgrade migration はサポートしない)

**bump triggering condition** (Phase 9-18 ルール):
- bump する: `.engawa` YAML の **外部表現の意味的変更** (フィールド削除、フィールド名変更、既存値の意味変更、必須フィールド追加で legacy YAML が読めなくなる場合)
- bump **しない**: Rust 内部のデータ構造変更 (enum 内包化、struct リファクタ、フィールド追加で `#[serde(default)]` で吸収できる範囲) で legacy YAML が同じ意味で読める場合
- 先例: ADR-010 で `CreateSketch.plane_ref` を新規追加した際、`plane` / `offset` を互換のため残して legacy YAML を読めるようにし、schema_version 概念導入前ではあるが「外部互換が取れる範囲は bump しない」思想で進めた
- multi-version 管理 (複数 schema_version pair の MigrationHook 連鎖、並行 schema 改定の衝突解消) の本格運用は **Phase 19 STEP I/O で再設計**。Phase 9-18 の Sketch / Feature 拡張は原則 v1 据え置きで進める

MigrationHook trait:
- 1 メソッド `migrate(&self, from, to, doc)` の最小形 (`from()`/`to()` getter は driver 側で扱う)
- driver (loader が hook を選択して連鎖適用する関数) は本 ADR では未定 (Phase 9 では入口のみ、driver は次 ADR で)

理由:
- 単純 u32 にすることで `cargo run` で見える number を bump するだけで version 概念が伝わる
- driver と trait を分離すると trait は破壊的変更なしに driver 戦略を変えられる (chain 適用 / 双方向 / parallel など)

### 4. 品質基盤ツール選定

採用: **proptest (property test) + criterion (bench) + cargo-fuzz (fuzz) + cargo-llvm-cov (coverage) + Playwright (E2E)**

- **proptest**: B-rep 系の Boolean / Tessellation の不変量 (Euler-Poincaré, manifold) を property-based で検証。Phase 10+ の boolean 安定化で必須。
- **criterion**: Tessellation / Boolean / SDF サンプリングなど bench を継続観測。回帰 (10% 以上の遅化) を CI で検出する基準値を取る。
- **cargo-fuzz**: `from_yaml` parser fuzzing。crash/panic 0 を継続。
- **cargo-llvm-cov**: line/branch coverage 計測。Phase 12 (Refactor Pass 1) の dead code 整理で根拠を取る。
- **Playwright**: `engawa-viewer` (web) の E2E。既に Phase 7 で導入済みのため、本 Phase では bench baseline + screenshot regression を追加する。

最小 setup 範囲 (本 Phase 9 内で完了):
- 各ツールの `Cargo.toml` / `package.json` 依存追加
- 1 ファイルずつ smoke test を書いて CI で実行できる状態にする
- bench baseline JSON を `bench-results/baseline-phase9.json` に commit

理由:
- 5 ツール並列導入は heavy だが、Phase 12 の Refactor Pass までに揃えないと "品質基盤がないので refactor 怖い" 状態になる
- 1 ツール 1 PR で逐次より、まとめて入口を作る方が context switch が少ない

## Decision Matrix

| 項目 | Option A | Option B | Option C | 採用 | Trade-off | 採用前提崩壊 trigger |
|------|----------|----------|----------|------|-----------|--------------------|
| Feature CRUD API | 個別関数 (`edit_feature()`, `delete_feature()`, ...) | `FeatureOp` enum + `apply_op()` 中央 | `Box<dyn Command>` (Command pattern, trait object) | **Option B (enum)** | enum は variant 追加が breaking、trait object は dyn 越境が型情報失われる。Option B が中庸 | Undo/Redo を実装した結果、`Box<dyn Command>` でないと collection に持てない事例が出た場合 (Phase 10+ で再評価) |
| Variable 構文 | `${var}` | `@var` | `{{var}}` (mustache-style) | **Option A (`${}`)** | YAML 既存予約と衝突しない、Rust/JS 文化圏で広く認知 | YAML パーサが `${}` を unescape する挙動を持つことが判明した場合 |
| Variable 解決順 | Sketch → Document (近スコープ優先) | Document → Sketch (明示優先) | フラット名前空間 (collision = error) | **Option A (Sketch 優先)** | レキシカルスコープ規約と一致、shadowing UX 強い | ユーザーから「shadowing で意図しない値が入った」フィードバックが累積 |
| `schema_version` 型 | `u32` 単調増加 | semver (`major.minor.patch`) | hash (e.g. SHA-256 of schema) | **Option A (`u32`)** | semver は 3 軸の意味付け不要、hash は人間が読めない | 並行ブランチで同 schema 改定が独立に走る事態 (Phase 19+ STEP I/O で外部 schema を混ぜる時に再評価) |
| MigrationHook 形 | `migrate(&mut Document)` (driver が判断) | `migrate(from, to, doc)` (hook が自己同定) | `from()/to()/migrate(doc)` 3 メソッド | **Option B (`migrate(from, to, doc)`)** | driver が中央集権、hook は純関数 | Hook 1 つで複数 version pair を扱いたい事例 (例: noop migration) が増えたら再評価 |
| 品質基盤導入順序 | 1 ツール / Phase で逐次 | Phase 9 で 5 ツールまとめて | proptest+criterion を Phase 9、残りは Phase 12 | **Option B (まとめて)** | heavy だが context switch が少ない | Phase 9 が 5 ツール導入で延びすぎる場合 (Issue 8 件以上消化に 2 サイクル超) |

## Trade-off

- **`FeatureOp` enum 採用**: Undo/Redo を統一できるが、enum variant 追加が breaking change になる。`#[non_exhaustive]` で forward-compat を確保する。
- **`${var}` 構文**: 文字数が長め (`@x` より 4 文字多い) だが、YAML との衝突回避と認知度を優先。
- **u32 schema_version**: 並行 schema 改定が起きると衝突するが、現状は単一 maintainer のため許容。Phase 19 STEP I/O で再評価する。
- **5 ツールまとめて導入**: Phase 9 が重くなるが、Phase 12 Refactor Pass までに揃えないと quality gate が機能しない。

## 採用前提崩壊 trigger

各 Decision Matrix 行の "採用前提崩壊 trigger" を参照。具体:

- **FeatureOp enum** → `Box<dyn Command>` 必要事例が累積したら Phase 10+ で再評価
- **`${}` 構文** → YAML パーサ干渉が判明したら `@var` (Option B) に移行
- **Sketch 優先解決順** → shadowing 起因のユーザーバグが累積したら Document 優先 (Option B) に切り替え
- **`u32` schema_version** → 並行改定衝突が発生したら semver (Option B) へ
- **`migrate(from, to, doc)`** → noop migration / 複数 version pair の hook 需要が累積したら driver-based (Option A) へ
- **5 ツール一括導入** → Phase 9 が 2 サイクル超かかったら proptest+criterion 優先 (Option C) へ縮退

## 既存 ADR との関係

- **ADR-002 (ロードマップ・ラベル運用)**: 本 ADR は ADR-002 の "type:foundation Issue は Phase 完了判定の対象外" に該当 (#237 自身は foundation)
- **ADR-006 (Issue 粒度)**: 本 ADR 自体は 1 ADR で 4 項目を扱うため "ADR-006 §1 1 Issue = 1 軸" の例外 (ADR は粒度ガード対象外)
- **ADR-007 (アセンブリ参照)**: Variable 2 段スコープは Component を越えないため、ADR-007 のアセンブリ階層と直交
- **ADR-013 (ADR 自動 accept フロー)**: 本 ADR は `gate:adr-review` 経由で auto-accept される (Decision Matrix lint + Codex 3 ペルソナ並列)
- **ADR-014 (Component RefPlane 隔離)**: 本 ADR の決定はすべて RefPlane を変えないため直交

## Open Questions

- **Q1**: Variable 評価で循環参照を検出した時、エラーに含めるべき context (どこで cycle が閉じたか) はどの粒度か? → 実装 #241 で詰める
- **Q2**: `${var}` を string literal にネストできるか (`"prefix-${var}-suffix"`)? → 実装 #241 で「YES だが ADR-016 で確定」と扱う
- **Q3**: 品質基盤 5 ツールの CI 実行時間が許容範囲を超えた場合、どれを on-demand に回すか? → Phase 9 末で計測してから決定
