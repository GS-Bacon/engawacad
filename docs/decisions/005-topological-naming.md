# ADR-005: トポロジカル・ネーミング方式の決定

## Status

Accepted

## Context

### 問題: 生 index 参照の崩れシナリオ

現状、B-rep トポロジーエンティティ間の参照は `Solid` のフラット配列への生 `usize` インデックスで行われている
(`crates/mycad-kernel/src/brep/topology.rs`)。`EntityId`(u64)は各エンティティに付くが参照キーとしては
未使用であり、`IdGenerator` は 0 始まりの単調カウンタ(生成順=インデックスと同じ脆さ)になっている。

Feature 間でエンティティを生 index / `EntityId` で参照した場合、以下のシナリオで崩壊する:

```
F1: CreateBox        → faces[0..5] (例: 上面 = faces[5])
F2: <下流 Feature>   → "faces[5]" を参照
─ F1 の上流に F0 を挿入、または F1 を CreateCylinder に変更 ─
→ faces 配列の中身・長さが変わり "faces[5]" は別物か範囲外
```

これは FreeCAD が長年抱えた「トポロジカル・ネーミング問題」と同根である。

### 現状の資産

- `EntityRef { feature_id, role }` が `crates/mycad-format/src/feature.rs:6-12` に定義済みだが未使用。
- Feature 間は既に文字列 `id` で参照している(`Extrude { sketch: String }` 等)。
- ADR-001/004 の原則: **Feature history が真実の源で、B-rep は Feature から再生成される**。
  `.mycad` は Feature 列のみ永続化 → ID スキーム変更にファイル移行不要。

### 堅牢性の対象範囲（スコープ明示）

本 ADR が守る不変性は「Feature **再生成・再番号付け**に対する参照の安定性」である。
ユーザによる明示的な `feature_id` の rename は「文書全体の協調的リファクタ」(全参照を同時に書き換える
明示編集)であり、本 ADR のスコープ外とする。

また、参照スコープは**現在の文書の単一 Component 内**に限定する。Component 階層越し・同一部品の
複数 occurrence を跨ぐ参照は **Phase 5(アセンブリ/部品参照)** で `(component/occurrence path,
feature_id, kind, role)` へ前方拡張する(既存の基底名は書き換えない、path を prefix として加算)。

### Extrude (#22) との関係

Phase 3 の `Extrude { sketch, depth }` 最小実装は既存ソリッドの面を参照しない(スケッチ + 深さの
スカラーのみ)。面選択の実需は Phase 4(Boolean / フィレット / 面上スケッチ)で初めて発生する。
ただし本 ADR を #22 の前提として先に確定し、命名規約を Extrude の実装に反映させる。

## Decision

### 1. Feature 間の生 index / `EntityId` 参照を禁止

Feature 間で B-rep エンティティを生 `usize` インデックスまたは `EntityId` で永続参照しない。
クロス Feature の参照は以下で定義する安定名のみとする。

### 2. `feature_id` を不変 identity anchor と明文化

`Feature.id` を「文書履歴内で不変の machine identity」と位置づける。
feature→feature 参照も `EntityRef` もすべて `feature_id` を key とする。
表示用の別名が必要な場合は将来別 field に分離する(本 ADR では format 変更をしない)。

**Component 内一意性を必須化(F01)**: `Feature.id` は所属する **Component の feature リスト内**で
一意でなければならない(文書全体ではなく Component スコープ)。兄弟 Component がそれぞれ `box_1` を
持つことは許容される。重複・再利用は `FormatError` とする。これにより Phase 5 のアセンブリ再利用で
不要な全体 rename を強制しない。

**未検証 Document をパブリック API から逃がさない**: 構築済みの `Document` は常に valid であることを
不変条件とする。実装選択肢は二択のいずれか — (a) `Document` に custom `Deserialize` impl を与え
バリデーションをデシリアライズ内に組み込む、または (b) raw/validated を分けた newtype ラッパーを用意し
unvalidated な raw 型をパブリックに公開しない。具体的な API 設計と `FormatError` の
エラー型統一は実装 issue の責務とする。

**不変条件は mutation を通じても維持すること(F01)**: load path の封鎖だけでは不十分。
`Document`/`Component` のフィールドを非公開にするか、全 mutation API (add_feature 等) に検証を
組み込む(validated builder/setter)。未検証状態を公開 API に決して露出しない。

### 3. `EntityId` をビルド内ハンドルに格下げ

`EntityId` は単一ビルド内の配列相関ハンドルとしてのみ使用する。`.mycad` に永続化せず、
Feature 間参照には使わない。`IdGenerator` の positional counter はビルド内決定性のためにのみ機能する。

> **ADR-001:30 の更新**: ADR-001 Implementation Details の「各エンティティは `EntityId` を持ち、
> Feature からの参照に使用」という記述を本 ADR が改める。`EntityId` はビルド内ハンドルであり、
> Feature からの永続参照には使わない。

### 4. 永続参照型 = 安定名 `EntityRef`(拡張前提)

`EntityRef` を将来 enum 化できる前提として扱う。今は `Named { feature_id, kind, role }` 相当
のみを扱い、実装は #22 / 専用実装 issue で行う。Phase 4 で `Derived` variant を加算的に追加する。

```
// 将来形(Phase 4 で実装)
enum EntityRef {
    Named { feature_id: String, kind: EntityKind, role: String },
    Derived { op: String, from: Vec<EntityRef>, selector: String },
}
```

### 5. 採用方向性: 要素マップ + 履歴ハッシュ(FreeCAD 1.0 流)

堅牢化の最終形として**要素マップ + 履歴ハッシュ方式**を採用方向として記録する。

- **基底名**は今(Phase 3)実装する種(seed)。生成操作のエンティティには直接安定名を付ける。
- **履歴伝播**(generated/modified/deleted マップ)は Phase 4 Boolean が要求する時点で実装する。
  今実装しない理由: 派生エンティティを生む操作が未存在のため、伝播ルールを推測実装すると
  本物の Boolean 着手時に書き直しになる。基底名は完全版の部分集合なので後付けで書き直し不要。
- **derived name の決定性制約**: 複数 source name から derived name を作る際は、入力 stable name を
  **canonical order に正規化してから連結/ハッシュ**する(unordered container を seed に直接使わない)。
  これは Phase 4 実装の必須制約として今記録する。

Phase 4 実装詳細は Issue #27 / #31 を参照。

### 6. 基底名の canonical grammar と charset

**表現の分離(F02)**: grammar 文字列 `<feature_id>;<kind>:<role>` は**内部 canonical stable name**
(ハッシュ seed・同一性判定に使う)であり、`.mycad` 上の wire format ではない。
`.mycad` / serde / TS / JsonSchema の on-disk format は **構造化形式**(`{ feature_id, kind, role }`)
を使う。canonical name 文字列はビルド時に構造化フィールドから導出する。

**フォーマット(内部 canonical name)**: `<feature_id>;<kind>:<role>`

| フィールド | 例 | 説明 |
|-----------|-----|------|
| `feature_id` | `box_1` | Feature の id |
| `kind` | `F` / `E` / `V` | face / edge / vertex |
| `role` | `top` / `seam` | 各 maker が割り当てる安定役割名 |

**文字集合**: 各セグメントの許容文字は `[A-Za-z0-9_-]`。区切り文字 `;` と `:` は予約とし
segment 内で禁止する。`feature_id` と sketch element stable id も同じ charset に従う。

**バリデーション責務(F03)**:
- `mycad-format` が parse 時に `feature_id` の charset と Component 内一意性を検証(`FormatError`)。
- `EntityRef` を永続化する際は `role` の charset も同様に format 層が検証する(`[A-Za-z0-9_-]` 以外は `FormatError`)。
- sketch element id の charset/一意性検証責務は sketch 型を定義する #22。

**例**:
```
box_1;F:top              # CreateBox の上面
cyl_1;F:lateral          # CreateCylinder の側面
cyl_1;E:seam             # CreateCylinder の seam エッジ
ext_1;F:cap_end          # Extrude の終端キャップ面
ext_1;F:side_seg_0       # Extrude のスケッチ要素 seg_0 から生じた側面
```

### 7. role 付与ルール

**基本規約**: 生成順カウンタは禁止(index と同じ脆さ)。各 maker が face/edge/vertex すべてに
**一意・決定的な role を明示列挙する**。隣接面 role からの導出式は採らない(辞書順・方位順・
列挙順で別名が生じ、自己隣接/周期面で衝突するため)。

**canonical local frame の必須化**: 各 maker は role を導く基準となる canonical local frame
(座標系・面法線の向き・loop 巻き方向・周期面の seam 原点)を、feature **パラメータから一意・決定的**
に導く規則を定義しなければならない。内部リファクタで別名化させないため、role 表はこの frame に従って
固定する。具体 frame と role 表は各 primitive の role 付与実装 issue で確定する。

**位置パラメータの扱い**: canonical local frame を導く feature パラメータには、形状パラメータ
(radius/height 等) に加え位置パラメータ (`CreateCylinder` の origin、`CreateSphere` の center 等) を含む。
position は frame の**原点を決めるだけ**で、role 名 (`lateral`/`cap_top`/`seam`/`surface` 等) や
内部 canonical name grammar `<feature_id>;<kind>:<role>` には影響しない。position を省略した場合は
canonical 原点 (0,0,0) にデフォルトし、既存の example YAML は不変のまま有効である。
回転 (rotation) の扱いは別 Issue で決定する。

**face role の例**:

| Feature | face role |
|---------|-----------|
| CreateBox | `top` / `bottom` / `front` / `back` / `left` / `right` |
| CreateCylinder | `lateral` / `cap_top` / `cap_bottom` |
| CreateSphere | `surface` |
| Extrude | `cap_start` / `cap_end` / `side_<sketch要素安定名>` |

**edge/vertex role**: maker が明示命名する(導出しない)。同一 Solid 内で一意であること。

**自己隣接・周期トポロジー**: 1 面が両側で接する seam や極(sphere の北極・南極、cylinder の
seam)は専用 role を必ず割り当てる。面 role だけからの導出で名前を潰してはならない。

| エンティティ | 専用 role 例 |
|------------|-------------|
| sphere 極 edge | `north_pole`, `south_pole` |
| sphere seam edge | `seam` |
| cylinder seam edge | `seam` |
| cylinder rim edges | `cap_top_rim`, `cap_bottom_rim` |

### 8. sketch element stable id の契約

`Extrude` の側面 role が依存するため、sketch element stable id に次の契約を課す:

1. 同一 sketch 内で一意
2. 再読込・再生成で不変
3. `.mycad` に永続化され、format 層で検証される

実装(sketch 型・id 付与・バリデーション)は **#22** が担い、上記 3 条件を #22 の acceptance とする。

### 9. topology materialization の決定性

安定名の決定性は、その土台となる topology 構築の決定性に依存する。各 maker は
`HashMap`/`HashSet` 等の**順序非決定な反復を禁止**し、入力要素は stable id または明示ソート順で
走査する。同一入力で `Solid` フラット配列順・`EntityId` 発番順・座標まで一致すること
(CLAUDE.md の決定性原則の再確認)。

### 10. 退化エンティティの命名方針

命名は topology validation 後の**非退化**エンティティにのみ付与する。退化入力
(zero-depth extrude、ゼロ長 sketch edge、面積ゼロ面など)は **`KernelError`** とし、
名前集合に決して現れさせない。

比較公差は ADR-004(数値モデルは Phase 4 で決定)に従うが、それまでは
**kernel 共通の epsilon 定数/比較関数を 1 箇所に定義**し全 maker・validator で共有する
(maker ごとの別閾値を禁止)。

## Rationale

### 生 index を安定名に変える価値

ムダな破損(上流の再番号付けのみで参照面は不変)はゼロにできる。安い破損を潰す価値は高い。
本質的な破損(参照面が実際に分裂・消滅)は OCC/Parasolid でも推測か失敗であり、Phase 4 で
`Derived` variant を加えて段階的に対処する。

### FreeCAD 流を採用しつつ伝播を今作らない理由

| 方式 | 今の実装コスト | Phase 4 での作り直しリスク |
|------|-------------|--------------------------|
| 幾何ハッシュ | 低 | 高(寸法変更で破綻、Boolean 派生面を表せない) |
| 生成元+役割名(today) | 低 | 無(基底名は完全版の部分集合) |
| 完全 lineage(OCC TNaming) | 高 | 低 |
| 要素マップ+履歴ハッシュ(採用) | 中〜高 | 低 |

**段階導入**が最適: 基底名(=生成元+役割名)を今実装し、Phase 4 で履歴伝播を加算的に追加する。
基底名は完全版の seed であり書き換え不要。伝播ルールを今設計すると、対象操作(Boolean)が
存在しない状態で推測実装になり、本物の Boolean 着手時に書き直すリスクが高い。

### enum 化により Phase 4 はゼロリライト

`EntityRef::Named` は Phase 4 以降も変更なし。`Derived` variant を **加算的に追加**するだけ。
ADR-004 の「加算的に追加」原則と整合する。

## Implementation Details

### 直接の影響範囲(本 ADR 実装: docs のみ)

本 ADR #14 の成果物はこのドキュメントのみ。コード変更は後続 issue で行う。

### 後続実装 issue への影響

| 対象 | 内容 | 担当 issue |
|------|------|-----------|
| `mycad-format/src/feature.rs` | `EntityRef` enum 化(`Named { feature_id, kind, role }` + `Derived`)。`Feature.id` の不変性を doc コメントに明記。`Debug, Clone, Serialize, Deserialize, JsonSchema, TS` の維持 | **DONE** (#22 で実装、#27 で `Derived` 追加) |
| `mycad-format` load path | 単一 validated load path の実装。`feature_id` charset・Component 内一意性・`role` charset の検証。custom `Deserialize` または validated newtype でバイパス経路を塞ぐ | **DONE** (#22 で実装済み) |
| `mycad-kernel/src/brep/topology.rs` | `EntityId` の役割を「ビルド内ハンドル・非永続」と doc コメントに明記 | **DONE** (#22 で実装済み) |
| 各 primitive maker | canonical local frame 定義 + face/edge/vertex の role タグ付け実装 | **DONE** (cuboid/cylinder/extrude: #22、sphere: #38) |
| `mycad-kernel` epsilon | kernel 共通の epsilon 定数/比較関数を 1 箇所に定義 | **DONE** (ADR-004 / #31 で実装済み) |

### 後続 issue の acceptance tests（予約）

以下のテストを後続の実装 issue の acceptance criteria として予約する(F04):

| ID | 内容 | 期待結果 |
|----|------|----------|
| T01 | 同一 feature 列を 2 回 build | stable name・`Solid` 配列順・`EntityId`・座標が完全一致 |
| T02 | 重複 `Feature.id` を含む文書を load | `FormatError` |
| T03 | grammar 負例(`;`/`:` 含む feature_id・role、許可外文字、空 segment、missing `kind`) | `FormatError` |
| T04 | `EntityRef` の YAML golden roundtrip | デシリアライズ後に完全一致 |
| T05 | 生成 `EntityRef.ts`・JsonSchema の exact golden | kind 追加・enum 化でスキーマ回帰しない |
| T06 | 退化入力(zero-depth extrude、ゼロ長 sketch edge 等) | 必ず `KernelError`、name 生成されないことを別 assertion で確認 |
| T07 | 境界値入力(epsilon 近傍) | name 集合の有無が 2 回実行で一致(共有 epsilon) |
| T08 | box/cylinder/sphere/extrude の expected names(F/E/V)を固定 | golden テストが一致 |
| T09 | 同一 Solid 内で `<feature_id>;<kind>:<role>` が衝突しない | 一意性 assert |
| T10 | role 付与後も `validate_manifold()` が通り V − E + F = 2(S − H) を満たす | sphere/cylinder 含む |
| T11 | 公開 API から未検証の `Document` を取得できない(validated newtype・custom Deserialize どちらの実装でも成立) | unvalidated 状態が漏れないこと |
- role 付与後も `validate_manifold()` が通り `V - E + F = 2(S - H)` を満たす(sphere/cylinder 含む)
- `from_path`/`from_yaml`/直接 deserialize 各経路で validation が必ず効くテスト

### Phase 4 での再検討事項

以下は今決定せず Phase 4(Boolean 着手時)に本 ADR を改訂して確定する。
Issue を 2 本に分割して粒度を管理する。

**#27 — 平面 Boolean 用 派生名伝播(Phase 4 の入口)**:
- 履歴ハッシュのアルゴリズム(合成名の正規化・連結・ハッシュ関数)
- 要素マップの伝播ルール(generated / modified / deleted の定義と実装)
- 明示グラフ式 vs ハッシュ名式の最終選択
- `EntityRef::Derived` の実装・`EntityRef.ts` golden

**#31 — 曲面 Boolean 用 pcurve + 数値モデル(#27 の後続)**:
- pcurve 対応 (ADR-004 参照)
- 数値モデル(トレラント vs 厳密、ADR-004 参照)

**Phase 5 へ延期** (Issue 化は Phase 5 着手時):
- Component 階層越し・複数 occurrence 参照の命名とエラー化(Phase 5 参照)
