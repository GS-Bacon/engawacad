## 自律判断ログ (autonomous mode)

本 Issue は `/3ailoop` 自律バッチモードから着手。STEP 4 ExitPlanMode を経ないため、以下の曖昧点を Claude が判断して確定した。

| 論点 | 判断 | 根拠 |
|------|------|------|
| build API シグネチャ — Issue body は `FeatureCrud::insert(&doc, feature_spec, at_index) -> Result<Document>` (純関数)、ADR-015 draft は `Document::apply_op(FeatureOp) -> Result<(), BuildError>` (in-place enum dispatch) と矛盾 | **Issue body 採用 (純関数版)** | ADR-015 (#246) は `needs-human` 退避中で未確定。Issue body は #244 子分割で確定したスコープ。enum 抽象は #256-#260 と束ねて将来 ADR ratify 後に導入可能 (関数を `FeatureCrud` namespace に置けば後方互換) |
| CLI コマンド名 — Issue body は `engawa entry add <feature-spec> --at <index>` | **そのまま採用**。`<feature-spec>` は **YAML ファイルパス** とする (positional arg) | ADR-016 (#245) も needs-human 退避だが、Phase 9 完了条件は `engawa entry add/edit/remove/reorder/suppress` 統一 CLI とある。`add` = Insert で確定 |
| 対象 Component — Component 階層のどこに insert するか | **Phase 9 最小は `root_component` のみ**。子 Component への insert は `--component <name-path>` フラグで *将来* 追加（本 Issue では Out-of-Scope） | ADR-006 §1「1 軸 × op」範囲を保つため。スコープ拡大は #256+ で連動して足す |
| 出力先 — CLI 実行後 .engawa をどう書き戻すか | **`-o/--output <path>` フラグ。指定なし時は `<input>` を上書き**。`--dry-run` はファイル書き込みを抑制し標準出力に YAML を吐く | UNIX 規約 (`-o`) + dry-run の意味として一般的 |
| at_index 範囲 | `0..=features.len()` 許可。`features.len()` (末尾) も OK。`> features.len()` は退化 (`OutOfRange` エラー) | `Vec::insert` 規約に揃える。境界 `_at_zero` / `_at_end` をテストで明示 |
| feature-spec 内 id 衝突 | **既存 Feature の id と一致したら `DuplicateFeatureId` エラー**。新規 id は新規割当 (caller 責任) | ID-stable 要件と engawa-format `validate_component()` が既に duplicate id を弾くため、エラー型を CRUD 層で先に出して原因明示する |

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `engawa-build::feature_crud` モジュールを新設し `FeatureCrud::insert(&Document, Feature, usize) -> Result<Document, FeatureCrudError>` を export | `FeatureCrud::edit / delete / reorder / suppress / rollback` (#256-#260 各 Issue で実装) |
| `FeatureCrudError` を `thiserror` で新設 (variants: `OutOfRange`, `DuplicateFeatureId`) | `FeatureOp` enum / `Document::apply_op` 中央ディスパッチ抽象 (ADR-015 ratify 後の別 Issue) |
| `engawa entry add <input.engawa> <feature.yaml> --at <index> [-o <path>] [--dry-run]` CLI subcommand | 子 Component への insert (`--component <name-path>`、将来) |
| Insert 後 Document を `validate()` してから return (duplicate id 等を二重検出) | inline feature-spec (`--feature 'type: create_box ...'` のような multi-line CLI 直書き) |
| 決定性: 同一入力で 2 回適用 → byte-identical な YAML 出力 | YAML 出力の整形改造 (既存 `to_yaml()` の挙動踏襲) |
| 境界テスト: `--at 0` (先頭) / `--at len` (末尾) | property-based test (proptest 導入は ADR-015 4 項目 = #239 系) |
| 退化テスト: `--at len+1` → 明示エラー / 重複 id → 明示エラー | Undo/Redo (ADR-015 ratify 後) |

## Non-Goals

- `FeatureOp` enum 抽象の導入 — ADR-015 (#246) が `needs-human` のため確定後の別 Issue
- 子 Component (`root_component.children[*]`) への insert — Phase 9 最小スコープ外
- inline YAML 引数 — file path のみ
- Undo/Redo / history stack — Phase 9 別 Issue or Phase 10+
- proptest / criterion 導入 — ADR-015 §4 別系統 (Phase 9 入口)
- `schema_version` migration 連動 — 別 Issue (Phase 9 入口)
- **history-dependent semantic validation** (sketch/target/tool ref 存在チェック + body lifetime 検証) — #263 で別 Issue 化 (Codex 7.5 r1 M-F01 を scope-defend)。format-layer `validate()` は duplicate id 等のみ。semantic 違反は `build_assembly` 時に `SketchNotFound` / `BodyNotFound` で発覚する現挙動を受容する

## 実装対象

<!-- Issue: #255 -->
影響クレート/ファイル:

- 新規: `crates/engawa-build/src/feature_crud.rs` (`FeatureCrud` namespace, `FeatureCrudError`, `insert` 関数)
- 編集: `crates/engawa-build/src/lib.rs` — `pub mod feature_crud;` + `pub use feature_crud::{FeatureCrud, FeatureCrudError};` 再エクスポート
- 編集: `crates/engawa-cli/src/main.rs` — `Commands` に `Entry { #[command(subcommand)] op: EntryOp }` 追加、`EntryOp::Add { input, feature, at, output, dry_run }` arm を実装
- 編集: `crates/engawa-cli/Cargo.toml` — `serde_yaml` workspace dep を `[dependencies]` に追加 (feature.yaml パース用)
- 新規: `crates/engawa-build/tests/feature_crud_insert_acceptance.rs` (integration test, T01-T_DEG_*)

変更する型・関数のシグネチャ:

```rust
// crates/engawa-build/src/feature_crud.rs
use engawa_format::{Document, Feature};
use thiserror::Error;

pub struct FeatureCrud;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FeatureCrudError {
    #[error("insert index {index} is out of range (root_component has {len} features)")]
    OutOfRange { index: usize, len: usize },
    #[error("feature id {id:?} already exists in root_component")]
    DuplicateFeatureId { id: String },
    #[error("document validation failed after insert: {source}")]
    Validation { #[from] source: engawa_format::FormatError },
}

impl FeatureCrud {
    /// Insert `feature` into `doc.root_component.features` at the given `at` index.
    /// Returns a NEW Document (in-place mutation禁止). Existing feature ids are preserved
    /// (ID-stable). `at == doc.root_component.features.len()` is allowed (append at tail).
    pub fn insert(doc: &Document, feature: Feature, at: usize) -> Result<Document, FeatureCrudError> {
        let len = doc.root_component.features.len();
        if at > len {
            return Err(FeatureCrudError::OutOfRange { index: at, len });
        }
        let new_id = feature.id().to_string();
        if doc.root_component.features.iter().any(|f| f.id() == new_id) {
            return Err(FeatureCrudError::DuplicateFeatureId { id: new_id });
        }
        let mut next = doc.clone();
        next.root_component.features.insert(at, feature);
        next.validate()?; // 二重ガード (Variable scope / RefPlane duplicate も検知)
        Ok(next)
    }
}
```

```rust
// crates/engawa-cli/src/main.rs — Commands に追加
#[derive(Subcommand)]
enum Commands {
    Export { .. },
    View { .. },
    /// Feature history CRUD operations on a .engawa file
    Entry {
        #[command(subcommand)]
        op: EntryOp,
    },
}

#[derive(Subcommand)]
enum EntryOp {
    /// Insert a new feature at the given index in root_component.features
    Add {
        /// Input .engawa file
        input: PathBuf,
        /// Path to a YAML file containing a single Feature
        feature: PathBuf,
        /// Insert position (0 = head, len = tail)
        #[arg(long)]
        at: usize,
        /// Output path (default: overwrite input)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Don't write to disk; emit resulting YAML to stdout
        #[arg(long)]
        dry_run: bool,
    },
}
```

## 設計方針

- **決定性**: `Document::clone()` + `Vec::insert(at, feature)` のみで非決定要素 (HashMap iter / 並列処理) なし。`to_yaml()` は既存の deterministic serialize 規約に乗る (T01 で 2 回実行 byte-identical を確認)。
- **B-rep トポロジー妥当性**: 本 Issue は format 層の Document 変換のみで brep を触らない。Euler-Poincaré 検証は N/A (#258 Suppress 等の生成・再生成系に登場)。
- **退化幾何の扱い**: 該当 op は Insert で「YAML データ追加」のみ。退化は (a) `at` index 範囲外、(b) 重複 id の 2 系統のみ。両方とも `FeatureCrudError` で明示エラー化。
- **derive 規約**: `FeatureCrud` は zero-sized 名前空間用 unit-like struct (= deriveなし)。`FeatureCrudError` は `Debug + Error`、`thiserror::Error` の `#[from]` で `FormatError` 透過。
- **エラーハンドリング**: `thiserror` 統一。CLI 側は `String` 変換で既存 `run_export` / `run_view` のスタイル踏襲 (`eprintln!("error: ...")` → `exit(1)`)。
- **workspace.dependencies**: `serde_yaml` は既に workspace に存在 (`engawa-format` で利用)。`engawa-cli/Cargo.toml` で `serde_yaml = { workspace = true }` を追加。`thiserror` も既存 workspace。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 `(doc, feature, at)` を 2 回 insert → `to_yaml()` byte-identical | `assert_eq!(yaml1, yaml2)` |
| T02 | 正常系 (build) | 既存 1-Feature Document の末尾に Feature 追加 → 順序 + 新 id が末尾 | `features[1].id() == "new"` |
| T03 | 正常系 (cli) | `engawa entry add simple_box.engawa new_box.yaml --at 1 --dry-run` → stdout に golden YAML。fixture/golden は `crates/engawa-build/tests/fixtures/insert/{input.engawa, new_box.yaml, expected.engawa}` に配置 | `assert_eq!(stdout, expected_yaml)` (golden file 比較) |
| T_BOUNDARY_at_zero | 境界 | `--at 0` で先頭挿入 → 既存 Feature は 1 番に押し下がる | `features[0].id() == "new"`, `features[1].id() == "box_1"` |
| T_BOUNDARY_at_end | 境界 | `--at len` で末尾挿入 (空 Document も含む `len=0` で `at=0` ケース) | `features.last().unwrap().id() == "new"` |
| T_DEG_at_out_of_range | 退化 | `--at len+1` → `FeatureCrudError::OutOfRange` | エラー type/値検証 |
| T_DEG_duplicate_id | 退化 | 既存 id と同じ id の Feature を挿入 → `FeatureCrudError::DuplicateFeatureId` | エラー type/値検証 |
| T04 | CLI 出力先 | `--output out.engawa` 指定で out ファイルに書き、input は変更されない | file 比較 |
| T05 | CLI 上書き | `--output` 未指定 → input ファイルが上書きされる | file 比較 |

## 幾何的不変条件チェックリスト

N/A — 本 Issue は engawa-format の Document YAML 変換のみで brep を触らない。Boolean / Partition / Assemble 系の不変条件は対象外。
