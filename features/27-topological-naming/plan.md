# Plan: #27 トポロジカル・ネーミング — EntityRef enum 化・派生名の合成（土台）

## Context（なぜやるか）

B-rep のトポロジー参照が壊れる「トポロジカル・ネーミング問題」を解くため、ADR-005 が
「要素マップ + 履歴ハッシュ」方式を採用方向として記録した。Phase 4(Boolean)に入る前に、
派生面へ安定名を付ける**土台**を作るのが本 Issue の役割（クリティカルパス `#27 → #33`）。

調査で判明した前提（事実）:
- `EntityRef` は現在 `{ feature_id: String, role: String }` の素の struct。`kind` も enum 化も未実装
  （`crates/mycad-format/src/feature.rs:5-12`）。ADR が #22 へ割当てた enum 化を #22 はやらず終えた。
- 読み込み経路に検証が無い（`document.rs:42-44` は素の serde）。命名用 `FormatError` バリアントも無し。
- `EntityRef` はどの `Feature` にも埋め込まれておらず、kernel からも未使用。参照箇所は
  `feature.rs`(定義+テスト) と `xtask`(TS export + golden) のみ → **改変の影響範囲は狭い**。
- Boolean(Cut/Fuse/Intersect) は Feature enum に存在するが kernel 未実装（#33 担当）。

### 確定した方針（壁打ちでユーザー合意済み）
1. **土台だけ作る**: 派生面を生む実操作への配線(generated/modified/deleted の実適用)と
   プリミティブへの role 付与は **#33 へ委譲**。実操作が無い今の伝播実装は推測になり書き直しリスク
   （ADR-005 行212-223 の警告）。
2. **A案: 明示グラフを永続化**: `Derived { op, from, selector }` の構造をそのまま `.mycad` に保存。
   不透明ハッシュ・別 lineage 台帳は持たない。内部の同一性判定用 canonical name は構造から決定的に導出。
3. **検証は Document の custom Deserialize に組み込む**: 全 load 経路で検証が必ず効く。

## 実装対象

- Issue: #27（Milestone: Phase 4、label: kernel / type: foundation）
- 影響クレート/ファイル:
  - `crates/mycad-format/src/feature.rs` — `EntityRef` enum 化 + `EntityKind` 追加 + canonical name + 検証
  - `crates/mycad-format/src/error.rs` — `FormatError` に命名検証バリアント追加
  - `crates/mycad-format/src/document.rs` — raw shadow + validated `Document`、`from_yaml`/`from_path`/`to_yaml` を `Result<_, FormatError>` 化
  - `crates/mycad-format/src/lib.rs` — 再エクスポート `pub use feature::{EntityKind, EntityRef, Feature, SketchPlane, SketchSegment};`（Codex R03）
  - `crates/xtask/src/main.rs` — `ENTITY_REF_GOLDEN` 更新（enum 化に伴う TS 再生成）
  - `web/src/generated/EntityRef.ts` — `cargo xtask gen-ts` で再生成しコミット
  - （必要なら）`Cargo.toml` workspace deps に `serde_json` を dev 追加（JsonSchema golden 用）

### 変更する型・シグネチャ

```rust
// feature.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind { Face, Edge, Vertex }   // canonical name では F / E / V に写像

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(tag = "ref")]            // 内部タグ: ref: named | derived（kind フィールドと衝突しない名前）
#[serde(rename_all = "snake_case")]
pub enum EntityRef {
    Named   { feature_id: String, kind: EntityKind, role: String },
    // Derived も kind を持つ（Codex R01: 派生参照が F/E/V のどれを指すか型保持。
    // 同一 provenance+selector の異種エンティティ衝突を防ぐ）。
    Derived { kind: EntityKind, op: String, from: Vec<EntityRef>, selector: String },
}

impl EntityRef {
    /// 内部 canonical stable name（同一性判定・map キー用。wire format ではない）。
    pub fn canonical_name(&self) -> String;
    /// 文字集合・空セグメント検証（再帰）。`Derived.from` が空なら `EmptyProvenance`（R02）。
    pub fn validate(&self) -> Result<(), FormatError>;
}
```

- `EntityRef` は **custom `Deserialize`**（raw shadow → `validate()` → 構築）で直接 deserialize 経路も塞ぐ。
- `Serialize` は derive のまま（内部タグ表現）。`Named`/`Derived` の wire 形は構造化（ADR F02 準拠）。

### canonical name 規則（決定性の核）

**重要（Codex R01/R02 反映）**: `from` は**順序が意味を持つ**（order-significant）。
`cut(target, tool)` のような非可換演算では「どの operand 由来か」を順序が表すため、
canonical_name で `from` を**ソートしてはならない**（ソートすると `cut(A,B)` と `cut(B,A)` が
衝突し provenance が消える）。

- `Named` → `N(<feature_id>;<K>:<role>)`（K は F/E/V）。例: `N(box_1;F:top)`
- `Derived` → `kind` を含め `from` を**与えられた順序のまま**連結: `D(<K>;<op>;<selector>;[<child1>,<child2>,...])`。
  子は各 `canonical_name()`（`Named`/`Derived` が再帰的にネスト）。**`from` は非空必須**（空は `EmptyProvenance`、R02）。
- **決定性の所在**: `canonical_name` は構造の純粋関数（同一構造→同一文字列を保証）。
  `from` 順を決定的に並べる責務は**それを生む操作側（#33）**にある:
  非可換 op は slot 順（target→tool 等）を保持、可換 op で genuinely 無順序な親集合を持つ場合のみ
  op が slot 内で canonical sort してから構築する。#27 はこの契約を文書化し、純粋関数性のみ保証する。
- **区切り文字の安全性（Codex R03 反映）**: 全セグメント（`feature_id`/`op`/`role`/`selector`）は
  `[A-Za-z0-9_-]` のみ許可で検証される。構造区切り `( ) ; : , [ ]` はこの charset に**含まれない**ため
  セグメント内に出現し得ず、エスケープ不要で一意にパース可能。
- ハッシュは使わない（A案）。将来 B 案へ移行する場合も wire 形は加算的に拡張可能。

## 設計方針

- **決定性要件**: `canonical_name` は構造の純粋関数（同一構造を 2 回 → 同一文字列）。`from` 順は
  **保持**（ソートしない、R01/R02）。`HashMap`/`HashSet` を seed に使わない（ADR Decision 5/9）。
  `from` を決定的順序で構築する責務は producing op（#33）。`Serialize` は derive のままで、
  order-significant な `from` がそのまま wire に出る（同一構造→同一 YAML を 2 回実行で保証するテストを追加）。
- **B-rep トポロジー妥当性**: 本 Issue は format 層のみ。Euler-Poincaré 検証は kernel maker（#33以降）の責務。
- **退化幾何の扱い**: 命名は非退化エンティティのみ（ADR Decision 10）だが、退化判定は kernel 側。
  format 層では「空セグメント／不正文字を `FormatError`」に限定。
- **derive 規約**: `Debug, Clone, Serialize, Deserialize, JsonSchema, TS`（+ `EntityKind` に `Copy, PartialEq, Eq`）。
- **エラーハンドリング**: `thiserror`。新バリアント案:
  - `FormatError::InvalidName { value: String, reason: &'static str }`（文字集合・空・区切り混入）
  - `FormatError::DuplicateFeatureId { id: String, component: String }`（Component 内重複）
  - `FormatError::EmptyProvenance { op: String }`（`Derived.from` が空、Codex R02）
- **workspace.dependencies 規約**: `serde_json` を dev 用に使う場合は `[workspace.dependencies]` 経由で
  `{ workspace = true }` 参照（CLAUDE.md 規約）。
- **検証経路（論点C、Codex R01/R03 反映）**: serde は **typed error を消す**（Deserialize 内で検証すると
  `FormatError::InvalidName` 等が `serde_yaml::Error` に潰れる）。そこで raw shadow + validated 型に分離:
  - `RawDocument`（`pub(crate)`、plain `#[derive(Deserialize)]`、検証なし）= wire を素直に受ける。
  - `Document::validate(&self) -> Result<(), FormatError>` = 全 Component 再帰で
    (a) `feature_id` 文字集合 (b) Component 内一意性 (c) 埋込 `EntityRef::validate()` を実施し**typed error** を返す。
  - `from_yaml`/`from_path` → `Result<Document, FormatError>`: raw deserialize 後に `validate()?`。**typed error 取得可**（T05/T07）。
  - `Document` 自身の `Deserialize` は custom 実装で「RawDocument へ deserialize → validate → 失敗は
    `serde::de::Error::custom`」とし、**直接 `serde_yaml::from_str::<Document>` の bypass も拒否**（ただし error は untyped）。
    → 直接 deserialize 経路は「検証は効くが型付きエラーは得られない」扱い（T09 は rejection を確認）。
  - `to_yaml(&self) -> Result<String, FormatError>`: serialize 前に `validate()?`。**自分で不正ファイルを書けない**（R03）。
    既存呼出側（cli/api/tests）の戻り値型変更に追従が必要。

### スコープ外（#33 / 他 Issue）
- generated/modified/deleted 進化マップの**実操作への適用**（本物の Cut/Fuse が要る）→ #33
- 各 primitive の canonical local frame + role 付与 → #33 もしくは専用 issue（**現状 open issue 無し**、要確認）
- Component 階層越し・複数 occurrence 参照 → Phase 5
- mutation API 全体の検証封鎖（ADR F01 の field 非公開化）: 既存テスト/`mycad-build` への波及大。
  本 Issue は **load 経路 + `to_yaml` 書込ゲート**で「不正ファイルの入出力」を封鎖する（R03 の write 非対称を解消）。
  公開フィールドへの直接代入を完全に塞ぐ field 非公開化は波及が大きいため別 issue 候補（Codex 最終判断）。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一構造の `Derived` を 2 回構築し `canonical_name` 一致（純粋関数性） | `assert_eq!` |
| T02 | provenance | `from` の順序が異なる 2 つの `Derived`（例: cut(A,B) と cut(B,A)）で `canonical_name` が**異なる**こと（R01: 非可換 provenance 保持） | `assert_ne!` |
| T03 | 正常系 | `Named { box_1, Face, top }` の `canonical_name` == `N(box_1;F:top)` | 一致 |
| T04 | 正常系 | `Named`/`Derived` の YAML ラウンドトリップ（serialize→deserialize→再 serialize） | byte 一致 |
| T04b | 決定性(wire) | 同一構造を 2 回 serialize して YAML byte 一致（`from` 順保持で揺れない） | `assert_eq!` |
| T05 | 負例(charset/EntityRef) | `EntityRef::validate()` 直接呼出で `role`/`op`/`selector` の許可外文字・`;`・`:`・空 | `FormatError::InvalidName` |
| T05b | 負例(EntityRef deserialize) | 不正 `EntityRef` を直接 deserialize（`TryFrom<RawEntityRef>` / custom Deserialize 経由） | 拒否（`is_err()`） |
| T06 | 負例(grammar) | enum 判別子(`ref`)欠落、未知 variant | deserialize error |
| T07 | 負例(uniqueness) | 同一 Component 内で重複 `feature_id` を含む Document を load | `FormatError::DuplicateFeatureId` |
| T08 | 正常系(uniqueness) | 兄弟 Component がそれぞれ同名 `feature_id` を持つのは許可 | Ok |
| T09a | 経路(typed) | `from_yaml`/`from_path` で **feature_id 不正・重複**が typed `FormatError` で返る（R02: EntityRef は未埋込のため Document 経路の typed 対象は feature_id に限定） | variant 一致 |
| T09b | 経路(bypass) | 直接 `serde_yaml::from_str::<Document>` でも不正 feature_id が **拒否**される（error は untyped 可） | `is_err()` |
| T10 | golden(TS) | 再生成した `EntityRef.ts` が `ENTITY_REF_GOLDEN` と一致（enum tagged union） | `assert_eq!` |
| T11 | golden(JsonSchema) | `schema_for!(EntityRef)` の JSON が exact golden と一致（スキーマ回帰防止） | `assert_eq!` |
| T12 | 決定性(100回) | `canonical_name` を 100 回生成して全一致 | 全一致 |
| T13 | 負例(provenance) | `Derived { from: [], .. }` を validate / load | `FormatError::EmptyProvenance` |
| T14 | 書込封鎖(R03) | 不正な `feature_id` を直接代入した Document を `to_yaml()`（EntityRef は未埋込のため対象外） | `FormatError`（書けない） |

## 検証（エンドツーエンド）

1. `cargo test -p mycad-format` — 上記 T01-T09, T11, T13, T14
2. `cargo test -p xtask` — T10（TS golden）+ 既存 determinism テスト
3. `cargo build --workspace` — `to_yaml`/`from_yaml` の戻り値型変更に追従して cli/api/build が通ること
4. `cargo xtask gen-ts` 実行 → `web/src/generated/EntityRef.ts` が更新・コミット済みで CI drift check 通過
5. `cargo xtask ci` green（fmt/clippy/test/build/TS drift/release smoke すべて）

## 未解決（Codex レビューで詰める）
- enum の serde タグ名（`ref` 案）と `EntityKind` の on-disk 表記（`face/edge/vertex` 読み形 vs `F/E/V`）
- ADR-005 改訂の文面（A案採用・`from` order-significant・伝播実装と op 別 `from` 順序契約は #33 へ、の追記）
- ADR F01 mutation 封鎖を本 Issue でどこまでやるか

## #33 へ引き継ぐ契約（本 Issue で文書化のみ）
- `Derived.from` を決定的順序で構築する責務は producing op（Cut/Fuse/Intersect）。
  非可換 op は slot 順（target→tool 等）を保持、可換 op で無順序親集合を持つ場合のみ slot 内 canonical sort。
- generated/modified/deleted 進化マップの実適用、プリミティブ role 付与。
