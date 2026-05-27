# Plan: ADR-005 トポロジカル・ネーミング方式の決定 (#14)

## Context（なぜやるか）

Feature が参照する B-rep エンティティ(面・辺・頂点)の同一性を、上流 Feature の**再生成・再番号付け後も**
安定して保つ命名方式を決め、`docs/decisions/005-topological-naming.md` として記録する。
Phase 3 の Extrude (#22) の前提要件。

現状の事実(調査済み):
- トポロジ間参照は `Solid` のフラット配列への生 `usize` index。`EntityId`(u64)は全エンティティに
  付くが参照キーとして未使用(死んでいる)。`crates/mycad-kernel/src/brep/topology.rs`。
- `IdGenerator` は 0 始まりの単調カウンタ(`topology.rs:268-286`)。順序依存=index と同じ脆さ。
- Feature 間は文字列 `id` 参照のみ(例 `Extrude { sketch }`, `Cut { target, tool }`)。
  トポロジ参照はまだ存在しない。`EntityRef { feature_id, role }`
  (`crates/mycad-format/src/feature.rs:6-12`)は定義済み・未使用。
- Feature history = 真実の源(ADR-001/004)。`.mycad` は Feature のみ永続化 → 同一性スキーム変更に
  ファイル migration 不要。
- **Extrude (#22) の MVP は既存ソリッドの面を参照しない**(`Extrude { sketch, depth }`)。
  面選択の実需は Phase 4(Boolean / フィレット / 面上スケッチ)で発生する。

**堅牢性の対象を明確化(R01 対応)**: 本 ADR が守るのは「Feature 再生成・トポロジ再番号付け」に対する
参照の不変性であって、「ユーザによる明示的な `feature_id` rename」ではない。後者は文書全体の協調的
リファクタ(全参照を同時に書き換える明示編集)であり、本 ADR のスコープ外とする。

これは **docs 専用 Issue**。crates のコード変更は行わない(実装は #22 および Phase 4 フォローアップへ)。

## 実装対象
- 新規ファイル: `docs/decisions/005-topological-naming.md`（ADR、ADR-001/004 と同じ章構成）
- 後続: Phase 4 フォローアップ Issue を新規作成（GitHub Issue）
- crates 変更: **なし**（影響範囲は ADR に「明記」するのみ）

## 設計方針（ADR-005 の決定内容）

### Decision
1. **生 index 参照の禁止(cross-feature)**: Feature 間で B-rep エンティティを生 index / `EntityId`
   で参照しない。永続的なクロス Feature 参照は「安定名」のみ。
2. **`feature_id` を不変 identity anchor と明文化(R01 対応)**: `Feature.id` を「ドキュメント履歴内で
   不変の machine identity」と ADR/format 上で位置づける。feature→feature 参照も `EntityRef` も
   すべて `feature_id` を key とする。表示用の別名が必要になった場合は別 field に分離する(本 #14 では
   format 変更はしない、方針記載のみ)。
   - **文書内一意性を必須化(R01)**: `mycad-format` の load/parse で `Feature.id` の文書内一意性を検証し、
     重複は `FormatError`。一度使った id の再利用も禁止(rename/duplicate/import 時の不変・再利用禁止規則を
     ADR に明記、重複検出テストを後続 acceptance に予約)。
   - **参照スコープを単一 Component 内に限定(R06)**: 本 ADR の `EntityRef`/安定名は**現在の文書の単一
     Component 内**の feature グラフ内参照のみを対象とする。Component 階層越し・同一部品の複数 occurrence を
     跨ぐ参照は**スコープ外(Phase 5: アセンブリ/部品参照)**。Phase 5 で `(component/occurrence path,
     feature_id, kind, role)` へ拡張する際は **path を prefix として加算**し、既存の基底名は書き換えない
     (enum/grammar の前方拡張で吸収)。子 Component・複数インスタンスの受け入れテストは Phase 5 Issue に予約。
3. **`EntityId` をビルド内ハンドルに格下げ**: 単一ビルド内の配列相関ハンドルとしてのみ使用。
   `.mycad` に永続化せず、Feature 間参照には使わない。
   （ADR-001:30「各エンティティは EntityId を持ち、Feature からの参照に使用」を本 ADR が改める。）
4. **永続参照型 = 安定名 (`EntityRef`)**: `EntityRef` を将来 enum 化できる前提とし、
   今は `Named { feature_id, kind, role }` 相当のみを扱う(型変更の実装は #22／専用 issue)。
5. **採用する方向性 = 要素マップ + 履歴ハッシュ(FreeCAD 1.0 流)**: 堅牢化の最終形として記録。
   段階導入 — Phase 3 は基底名のみ、履歴伝播(generated/modified/deleted)は Phase 4 へ先送り。
   - **履歴ハッシュの決定性制約(R08)**: 複数 source name から derived name を作る際は、入力 stable name を
     **canonical order に正規化してから連結/ハッシュ**する(unordered container を seed に直接使わない)を
     今のうちに必須制約として記録。multi-parent seed 決定性テストは Phase 4 Issue に明記。
6. **基底名の canonical grammar + charset を確定(R01/R02 対応)**: ハッシュ互換・機械解析可能な形
   `<feature_id>;<kind>:<role>`。**`kind` を必須要素とする**(kind ∈ {F=face, E=edge, V=vertex})。
   `EntityRef::Named` は `kind` を保持する(または grammar 上 `F:/E:/V:` を必ず含める)。
   例: `box1;F:top`, `box1;E:edge_top_front`, `box1;V:vtx_tfl`。
   - **各セグメントの許容文字集合を `[A-Za-z0-9_-]` に固定**。区切り文字 `;` と `:` は予約とし
     segment 内で禁止。`feature_id` と sketch element stable id も同 charset に従う。
   - **バリデーション責務**: format 層(`mycad-format`)が parse 時に `feature_id` の charset を検証し、
     違反は `FormatError`。sketch element id の charset 検証責務は sketch 型を定義する #22。
   - これが将来の履歴ハッシュの seed になる(後から書き換え不要にするための核心)。
7. **role は各 maker の明示的決定表で付与(R02/R03 対応)**: 生成順カウンタ禁止(index と同じ脆さ)。
   **隣接面 role からの導出式は採らない**(辞書順・方位順・列挙順で別名が生じ、自己隣接/周期面で衝突するため)。
   代わりに各 maker が face/edge/vertex すべてに**一意・決定的な role を明示列挙**する。本 ADR は規約のみ固定し、
   具体表は各 primitive の role 付与実装 issue で確定する:
   - face role 例: box=`top/bottom/front/back/left/right`、cylinder=`lateral/cap_top/cap_bottom`、
     sphere=`surface`、extrude=`cap_start/cap_end/side_<sketch要素安定名>`
   - edge/vertex role は maker が明示命名する(導出しない)。同一 Solid 内で一意であること。
   - **自己隣接・周期トポロジーの明示規約(R03)**: 1 面が両側で接する seam、極などの特異点は
     **専用 role を必ず割り当てる**(sphere=`seam`/`north_pole`/`south_pole`、cylinder=`seam`/
     `cap_top_rim`/`cap_bottom_rim` 等)。多重度は保持し、面 role だけからの導出で潰さない。
   - extrude の `side_<...>` の `<...>` は **sketch feature が割り当てる安定 element id** を使う。
   - **canonical local frame の必須化(R03)**: 各 maker は role を導く基準となる canonical local frame
     (座標系・面法線の向き・loop 巻き方向・周期面の seam 原点)を、feature パラメータから**一意・決定的**に
     導く規則を定義しなければならない。role 表はこの frame に従って固定する(内部リファクタで別名化させない)。
     具体 frame は各 primitive の role 付与実装 spec で確定(本 ADR は要件のみ固定)。
8. **sketch element stable id の契約(R04 対応)**: extrude の naming seed が依存するため、本 ADR で契約を固定:
   (a) 同一 sketch 内で一意、(b) 再読込・再生成で不変、(c) `.mycad` に永続化され format 層で検証される。
   実装(sketch 型・id 付与・検証)は **#22** が担い、上記 3 条件を #22 の acceptance に落とす。
9. **topology materialization の決定性(R02 対応)**: 安定名の決定性は、その土台である topology 構築の
   決定性に依存する。各 maker は `HashMap`/`HashSet` 等の**順序非決定な反復を禁止**し、入力要素は stable id
   または明示ソート順で走査する。これにより同一入力で `Solid` フラット配列順・`EntityId` 発番順・座標まで
   一致する(CLAUDE.md の決定性原則の再確認)。2 回 build 一致テストを後続 acceptance に予約。
10. **退化エンティティの命名方針(R05 対応)**: 命名は topology validation 後の**非退化**エンティティに
   のみ付与する。退化入力(zero-depth extrude、ゼロ長 sketch edge、面積ゼロ面など)は `KernelError`
   とし、名前集合に決して現れさせない。比較公差は ADR-004(数値モデルは Phase 4 で決定)に従うが、
   それまでは **kernel 共通の epsilon 定数/比較関数を 1 箇所に定義**し全 maker・validator で共有する
   (maker ごとの別閾値を禁止)。境界値テストを後続 acceptance に予約。

### Rationale
- 安い破損(上流の再生成・再番号付けのみで参照面は不変)はゼロにする価値があり、安定名で潰せる。
  本質的破損(参照面が実際に分裂・消滅)は OCC/Parasolid でも推測か失敗 → 既知の限界として受容。
- 履歴伝播を今作らない理由: 派生エンティティを生む操作(Boolean/フィレット)が未存在のため、
  伝播ルールを推測実装すると本物の Boolean 着手時に書き直しになる(過剰な先回り設計)。
  基底名は完全版の部分集合なので、後から伝播を載せても今の名前は書き換え不要。
- enum 化により Phase 4 は variant 追加(加算的、ADR-004 の方針と整合)。

### Implementation Details / 影響範囲(ADR に明記する内容)
- **単一 validated load path(R07)**: `mycad-format` の公開ロード経路を 1 本に集約し、`feature_id` 一意性・
  charset 検証が `from_path`/`from_yaml`/直接 deserialize のどの経路でも必ず効く契約にする(`from_yaml` を
  `FormatError` 化、または `Document::validate()` 必須呼び出し)。実装と各経路の検証テストは実装 issue へ予約。
- **`EntityRef` の派生 derive 維持(R11)**: `Debug, Clone, Serialize, Deserialize, JsonSchema, TS` を維持。
  `kind` 追加/enum 化時に `EntityRef.ts`・schema が壊れないよう、schema/TS golden を後続 acceptance に予約。
- `crates/mycad-format/src/feature.rs`: `EntityRef` を将来 enum 化(`Named { feature_id, kind, role }`
  + 後で `Derived`)。`Feature.id` の不変 identity 性を doc で明文化。本 #14 では型変更しない。
- `crates/mycad-kernel/src/brep/topology.rs`: `EntityId` を「ビルド内ハンドル・非永続」と位置づけ。
  `IdGenerator`(positional)は据え置きだが「永続性を持たない」前提を明記。
- 各 primitive maker(`primitives/cuboid.rs` 等): 将来 role タグ付け(face/edge/vertex)を追加(#22 以降)。
- ADR-001:30 の記述を本 ADR が更新(EntityId はクロス Feature 参照に使わない)。
- 未決(Phase 4 再検討): 履歴ハッシュのアルゴリズム、要素マップの伝播ルール、
  明示グラフ式 vs ハッシュ名式の最終選択、pcurve(ADR-004 と連動)、数値モデル(トレラント vs 厳密)。

## 将来方針の記録（user 要求: Issue に確実に残す）
Phase 4 フォローアップ Issue を作成する:
- タイトル例: 「トポロジカル・ネーミング: 要素マップ + 履歴ハッシュ伝播の実装(Boolean 着手時)」
- 内容: `EntityRef` の `Derived` variant 追加、generated/modified/deleted 伝播、
  基底名→履歴ハッシュ名の昇格、明示グラフ vs ハッシュ名の最終決定、pcurve 連動。
- ラベル: `kernel, type: foundation`。Milestone: Phase 4(存在すれば紐付け、なければ本文に明記)。
- ADR-005 本文からこの Issue 番号を相互リンク。

## 検証計画
### docs 検証（#14 の直接成果）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| V01 | 構成 | ADR が 001/004 と同じ章立て(Status/Context/Decision/Rationale/Implementation Details) | 目視一致 |
| V02 | リンク | ADR 内の相互参照(ADR-001/004、#14、Phase 4 Issue)が有効 | リンク切れなし |
| V03 | 完了条件 | Issue #14 の完了条件(方式記録 + 影響範囲明記)を満たす | 突合 OK |
| V04 | フォローアップ | Phase 4 Issue が作成され ADR から参照されている | Issue URL 確認 |
| V05 | コード不変 | crates に変更がない(docs 専用) | `git diff --stat crates/` 空 |

### 後続実装 issue に予約する acceptance tests（R04 対応、#22 / Phase 4 で実施）
- 同一 feature 列を 2 回 build → stable name に加え `Solid` 配列順・`EntityId`・座標まで完全一致(R02)
- 重複/再利用 `Feature.id` を含む文書 → `FormatError`(R01)
- grammar 負例 → `FormatError`(予約区切り `;`/`:`、許可外文字、空 segment、missing `kind`。R10)
- `EntityRef` の YAML golden roundtrip + 生成 `EntityRef.ts`・JsonSchema の exact golden(R11)
- 退化入力(zero-depth extrude 等)→ **必ず `KernelError`**(成功経路で通さない)。失敗時に name が
  生成されないことを別 assertion で確認。`validate_manifold()`/Euler 検証が退化成功を許さないことも確認(R09)
- 境界値入力(epsilon 近傍)で name 集合の有無が決定的に一致(共有 epsilon、R05)
- box/cylinder/sphere/extrude の expected names(face/edge/vertex)を固定する golden テスト
- kind/role の一意性(同一 Solid 内で `<feature_id>;<kind>:<role>` が衝突しない)
- **role 付与後も B-rep 妥当性が保たれる回帰(R04)**: 各 primitive で `validate_manifold()` が通り、
  `V - E + F = 2(S - H)` を満たす。周期トポロジーの sphere/cylinder も対象に含める。

## /3ai フロー上の扱い（適応）
- crates 実装が無いため STEP 6(GLM 実装)・`cargo xtask ci` は実質スキップ。
  「実装」= ADR markdown 執筆 + Phase 4 Issue 作成 + main へ直 commit(`Closes #14`)。
  docs/ は `guard-crates.sh` の対象外のため Claude が直接執筆可。
- STEP 3 Codex 設計レビュー: 本プラン(= ADR 設計)をレビュー。Critical/High を潰してから ExitPlanMode。
- STEP 7 Codex 最終レビュー: 執筆済み ADR markdown を対象に実施。
