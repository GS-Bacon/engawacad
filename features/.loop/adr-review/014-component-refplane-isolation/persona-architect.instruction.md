# Role: ADR architect (過去 ADR との整合 / 設計階層の安定性で refute せよ)

## Task
以下の ADR draft を adversarial に review してください。
REFUTE を default とし、明確に refute できなければ approved を返してください。
理由が浅い (1 文以下、根拠なし) refute は失敗判定とし approved を返してください。

## 出力フォーマット (必須)
最終行に必ず以下のいずれかを記載:
  verdict: approved
  verdict: refuted

refuted の場合、その直前に 200 字以上の refute 理由を記載してください。

## ADR draft

# ADR-014: Component RefPlane の独立 coordinate frame 化

**Date**: 2026-06-17
**Status**: Accepted
**Related**: ADR-002 (ロードマップ・ラベル運用), ADR-006 (Issue 粒度), ADR-007 (アセンブリと部品参照), ADR-013 (ADR 自動 accept フロー)
**Resolves**: Issue #158 STEP 7.5 Codex F01 (r2/r4 oscillation), Issue #162

---

## Context

Phase 5 で導入したアセンブリ (`Component.children` + `Component.reference`) と Phase 7 で導入した `RefPlane` (`Component.ref_planes`) を組み合わせると、**「child Component が空 `ref_planes` を持つときの解決ルール」** が未定義だった。

Issue #158 (`feat(format): RefPlane を Component 階層に格納`) の STEP 7.5 Codex 独立レビューで本論点が浮上し、r2 round と r4 round で **逆の指摘** が出た:

- **r2 (Option B 推奨)**: 空 child を親に継承させると、親が custom-only `ref_planes` (例: `TopDeck` 1 件) を持つとき child の `plane_ref: "Front"` が `UnknownRefPlane` で失敗する。child は canonical three (Front/Top/Right) に fallback すべき。
- **r4 (Option A 推奨)**: 空 child は親の `_ref_planes` を継承して `build_bodies_from_features()` に渡すべき。canonical fallback は nested component の `plane_ref` が root_component の custom datum を参照できない欠陥を生む。

Phase 7 の `examples/*.engawa` は root_component のみで child component を使わないため、Phase 7 のスコープでは決定できず Issue #162 で Phase 8 持ち越しとなった。Phase 8 以降の assembly では child component で sketch を描く経路が増えるため、本サイクルで正式に決定する必要がある。

---

## Decision

### Options

#### Option A: 親継承 (parent inheritance)

child の `ref_planes` が空のとき、親の `effective_ref_planes` を再帰的に継承する。

**Trade-off**:
- 利点: root_component で宣言した custom datum (例: `TopDeck`) が nested child の sketch から `plane_ref: "TopDeck"` で参照できる。assembly 全体で datum を共有する直感的な「データム共有」モデル。
- 欠点: 親が custom-only `ref_planes` を持ち、child が標準名 `Front` を参照する場合に `UnknownRefPlane` で失敗する。child が無関係な親のスコープに引きずられ、coordinate frame の独立性が崩れる。stdlib 部品 (M5x20 等) を inline child として配置するとき、親の datum 命名規約に巻き込まれる。

#### Option B: canonical fallback (採用)

child の `ref_planes` が空のとき、親を継承せず canonical three (Front/Top/Right) を local fallback として使う。各 Component は独立した coordinate frame を持つ。

**Trade-off**:
- 利点: child の `plane_ref: "Front"` が親の `ref_planes` 内容に依存せず常に解決可能。各 Component の coordinate frame が独立で、stdlib 部品を inline child としても置きやすい。`Document::from_yaml` が root の空 `ref_planes` を canonical 自動補完するため、parser-level 不変と一致する。custom datum が必要な child は自分で宣言する明示性。
- 欠点: root で宣言した custom datum を child から参照できない。assembly で「全体共有データム」を表現するには各 child で重複宣言が必要 (または Phase 9+ で `face_ref` 等のトポロジカル参照に切替)。

#### Option C: parent + canonical マージ

child の `ref_planes` が空のとき、親の `effective_ref_planes` と canonical three をマージして使う (id 重複時は canonical 優先 or 親優先など要追加決定)。

**Trade-off**:
- 利点: custom datum 継承 (A) + 標準名常時解決 (B) の両立。
- 欠点: マージ規則 (id 衝突時の優先、順序) を追加で決定する必要があり ADR が複雑化。child から見える ref_planes の集合がコンテキスト依存になり、設計者からの予測可能性が落ちる。「親の `TopDeck` が child から見えるが孫からは見えない (孫が中間 child の独立宣言で上書きされている)」のような階層性バグの原因となる。

### 採用: **Option B (canonical fallback)**

#### 採用理由

1. **parser-level 不変との整合**: `Document::from_yaml` / `Document::new` が root の空 `ref_planes` を canonical three に補完する既存挙動 (`crates/engawa-format/src/document.rs:68-70, 95-97`) と整合する。root だけ canonical 自動補完して child は親継承、という非対称が無くなる。
2. **coordinate frame の独立性**: Component は assembly 階層の局所単位という ADR-007 の設計と一致する。stdlib 部品が親の datum 命名規約に依存しないため、`stdlib://fasteners/jis_b1176/M5x20.engawa` のような再利用部品が安定する。
3. **Codex F01 r2 の堅牢性**: 親 custom-only + child 標準名 `Front` パターンで `UnknownRefPlane` が出ない。エラーケースの surface area が小さい。
4. **既存実装・既存テスト整合**: `crates/engawa-build/src/lib.rs:497-504` の現行コードと `t15_child_component_uses_own_ref_planes` / `t16_empty_child_falls_back_to_canonical_not_parent` の regression テストが既に Option B を enforce している。本 ADR で追加実装は不要で、宣言と dead parameter 除去のみ。
5. **Phase 8+ の将来拡張点との直交性**: 「親 datum を child から参照したい」用途は Phase 8 のモデル面選択 (model face selection) や Phase 9+ の `face_ref` (ADR-005 トポロジカル命名) で表現する道がある。ref_planes 継承で代用する必要はない。

### 採用前提崩壊 trigger

以下のいずれかが観測されたら本 ADR を見直す:

1. **datum 重複宣言が assembly 1 件あたり 5 件超** が 3 件以上の examples で observed (= 「親で 1 回宣言したら子で参照」が現実的に必要なユースケース増加の証拠)。
2. **child component の `ref_planes` 明示宣言を要求するパターンが Phase 9 履歴 CRUD で UX 課題化** (例: Variable で datum offset を動かしたいが 各 child で別変数になり同期できない)。
3. **stdlib 部品の `ref_planes` が canonical three 以外を持つケースが 3 件以上発生** (= 「stdlib 部品が自分用の custom datum を持つ」設計が広がり、Option B でも canonical 統一が崩れる)。

trigger 発火時は ADR-014 見直し Issue を起票し、Option A / C への移行コスト (既存 examples 影響範囲、既存 Document 互換性) と合わせて再評価する。

### 既存 ADR との関係

- **ADR-005 (トポロジカル命名)**: 「親 datum を child から参照」用途は将来的に `face_ref` / `entity_ref` (Phase 9+) で表現する。本 ADR は ref_planes 継承による代用を否定するが、ADR-005 の拡張経路を塞ぐものではない。
- **ADR-006 (Issue 粒度)**: 本 ADR は決定のみ、実装 (dead parameter 除去) は同 Issue #162 内で行うが、決定本体が小さく実装も 1 関数のシグネチャ変更のみなので「ADR + 実装混在」とはみなさない (実装側の判断が ADR 本文から一意に導出されるため)。
- **ADR-007 (アセンブリと部品参照)**: Component 階層の transform 適用ルールは無変更。本 ADR は ref_planes 解決ルールのみを扱う。reference-loaded subtree (`ComponentRef::StdLib` / `ComponentRef::File`) は元から独立した `root_component.ref_planes` を持つため Option B と整合する。
- **ADR-013 (ADR 自動 accept フロー)**: 本 ADR は L-5.6 auto-accept の最初の本格適用例となる。Decision Matrix lint (Options A/B/C 必須 / Trade-off / 採用 trigger / 既存 ADR 関係) と Multi-LLM Review (architect / contrarian / migration) の対象。

---

## Alternatives Considered

### (a) ADR を起こさず inline コメントのみで Option B を文書化

簡潔だが、F01 r2/r4 のような逆指摘が再発したときに「過去 ADR で決定済み」と差し戻せない。Codex / GLM レビューは ADR を参照する仕様 (`dispatch-glm-review.ts` の adr-context 機能) なので、ADR 不在は構造的に再 oscillation を許す。

### (b) Option A を採用し examples を全て canonical 明示宣言で書き直す

Phase 7 までの examples (約 8 件) を一斉に書き換える必要がある。`extruded_rect.engawa` / `two_bodies.engawa` 等の root-only example は影響無いが、Phase 5 で追加した `assembly.engawa` (もし child 経路あり) では破壊的変更となる。Phase 7 で既存テストが Option B を regression している以上、Option A 移行は無関係な test 書き換え + 既存 Codex F01 r2 指摘の再発リスクを抱える。

### (c) Option C (merge) を採用

マージ規則 (id 衝突時の優先、順序、深いネスト時の集約) を追加で決定する必要があり、ADR-014 のスコープが本 ADR の範囲を超える。`features/.dashboard.md` の "child から見える ref_planes" がコンテキスト依存になり、設計者の予測可能性が落ちる。Phase 8 でこのリスクを抱える根拠は無い。

---

## 影響を受けるファイル

| ファイル | 変更内容 |
|---|---|
| `crates/engawa-build/src/lib.rs` | `build_component_tree` の dead パラメータ `_ref_planes` を削除し caller 3 箇所を更新。inline コメントを ADR-014 参照に置換。 |
| `crates/engawa-build/tests/refplane_acceptance.rs` | T17 (grandchild canonical fallback regression) を追加。 |
| `docs/decisions/013-adr-auto-accept-flow.md` | Related に ADR-014 を追記 (任意、cross-link) |

---

## ADR auto-accept フロー (ADR-013) の通過条件確認

- **Decision Matrix lint**: Options A/B/C 列挙 ✓ / 各 option Trade-off ✓ / 採用 option + 理由 ✓ / 採用前提崩壊 trigger (3 件) ✓ / 既存 ADR との関係 (ADR-005/006/007/013) ✓
- **Multi-LLM Review**: architect (ADR-005/007 整合) / contrarian (Option A の利点強調) / migration (既存テスト T15/T16 互換性) — 次サイクル L-5.6 で実行予定
- **暴走防止 cap**: 本 ADR が初回 draft (regen=0)、token は Issue #162 サイクル内で計測。
