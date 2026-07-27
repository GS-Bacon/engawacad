<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R01: 採用 → plan の「...」節を修正 -->
<!-- - R02: 棄却 → Non-Goals に記載済みの蒸し返し -->
<!-- - R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue に委譲 -->

## Round 1

- scope: pass (issues なし)
- invariant: pass (issues なし。決定性・B-rep非該当・既存Featureへの副作用なし・アーキテクチャ整合性・退化判定/duplicate id検出のいずれも適切と判定)
- ambig: pass (issues なし)
- numeric: pass (issues なし)

採用 0 / 棄却 0。2 round 連続 C/H=0 の確認のため round 2 を実行する。

## Round 2

- scope: pass (issues なし)
- invariant: pass (issues なし)
- ambig: pass (issues なし)
- numeric: pass (issues なし)

採用 0 / 棄却 0。round 1・round 2 とも Critical/High = 0 → 2 round 連続収束、STEP 3-F へ。

## STEP 3.5 Codex

verdict: fail (high × 2)。両件とも **採用**。

- **R01 (high, blocking): 採用** → 実装対象 §4 の element-level gate を `count >= 2` 限定に修正
  - 検証: `crates/engawa-build/src/feature_crud.rs:838-861` の Mirror gate は `if !selection.is_empty()` のみを条件としており、count 相当の概念を持たない。これをそのまま Pattern に流用すると、kernel 側疑似コード step 3/4（`count == 0` → エラー / `count == 1` → selection 未参照で即 `Ok(source)`）と矛盾する。
  - 影響の実体: `count=1` + 未知 id の Document は `build_bodies_from_features` では成功し `FeatureCrud::insert` でのみ拒否される。これは #331 で追跡中の「元 profile vs current profile」アーキテクチャ起因 false-reject とは異なり、**gate に 1 条件足すだけで解消できる純粋な設計ミス**なので受容ではなく修正が妥当。
  - 反映: (a) 実装対象 §4 に発動条件 `*count >= 2 && !selection.is_empty()` と理由・`count == 0` を gate 対象外にする根拠を明記、(b) Non-Goals の count=1 no-op 行に「build/CRUD 両経路で一貫」を追記、(c) Non-Goals の CRUD gate 制約行に「`count >= 2` のときのみ発動」を追記、(d) 自律判断ログ §6 に採用追記、(e) テスト計画に `T_CRUD_pattern_n1_no_validation`（count=1 + 未知 id → insert が `Ok`）と `T_CRUD_pattern_unknown_element`（count=3 + 未知 id → `SketchElementNotResolved { reason: "sketch_pattern_linear_elem_not_found" }`）の 2 行を追加し gate 条件の両側を固定。
- **R02 (high, blocking): 採用（提案 (a) のみ。式の変更は棄却）** → テスト計画 T04 の期待値を修正
  - 独立検証: `step = total_angle / count = π / 2 = π/2 (90°)`、複製は `k = 1..count` すなわち `k=1` の 1 個のみ、`angle_1 = step * 1 = π/2`。よって Arc `center=(2,0)` は `(0,2)` へ、`start_angle: 0 → π/2`、`end_angle: π/2 → π`。旧記述の「180° 回転 / `start_angle=π` / `end_angle=3π/2`」は誤り。
  - 同表 T03（`total_angle=2π, count=4` → 90°/180°/270°）は `total_angle/count` 規約と整合しており、T04 単独の記述ミスであることを裏付ける。
  - Codex 提案 (b)（`total_angle/(count-1)` への変更）は**棄却**: 自律判断ログ §4 で count=1 のゼロ除算回避のため意図的に等間隔規約を選択済み。R02 は実装方針の誤りではなくテスト期待値の計算ミスであり、規約変更は不要かつ count=1 特例分岐を新たに要求するため不採用。
  - 反映: T04 行を `step` の明示・複製 1 個・`center=(0,2)` / `start_angle=π/2` / `end_angle=π` に修正。

採用 2 / 棄却 0（R02 は提案 (b) のみ部分棄却）。

## STEP 6.7/7.5 合流

STEP 6.6.5 の 3 並列 final review を合流し、Claude (Opus) が採用/棄却を判定した。

| 系統 | 結果 | 判定 |
|---|---|---|
| STEP 6.7 Claude self-review | critical 0 / high 0 / medium 4 / low 6 | medium 4 件すべて採用 (A1 は実装、C3/C6 はテスト、C1 は plan 記述修正) |
| STEP 7 GLM final review | pass, blocking 0 | 対応不要 |
| STEP 7.5 Codex final gate | fail, high 1 (A01) | **部分採用** (id 解決は修正、kind 検証は #331 委譲) |

**合計: 採用 4 / 部分採用 1 / 棄却 0（low 6 件は本 Issue 対象外として据え置き）。**

### Codex A01 (high) — 部分採用 → (a) コード修正

詳細は `codex-findings.md`。要点:

- `refs_resolve_in_state` の Mirror/Pattern arm が sketch id の存在しか見ておらず、
  `CreateSketch` の要素を rename/削除する `edit` が `EditBreaksConsumer` を出さずに通っていた
  (false-accept)。build で初めて `unknown_element_id` になる。
- **同じファイル内の `SketchFillet`/`SketchChamfer` arm は既に element-level 検証済み**で、
  その契約は `t10_crud_gate_rejects_rename_breaking_fillet` が固定している。つまり
  「系統的なアーキテクチャ制約」ではなく **Mirror/Pattern だけが前例から外れた実装漏れ**。
  これが (b) ドキュメント化ではなく (a) 修正を選んだ決め手。
- Mirror (#298, クローズ済み) 分も同時に修正した。理由: Codex が両方を名指ししている /
  修正箇所が同一 match の隣接 arm で Pattern だけ直すと新たな不整合を作る /
  再現テスト付きバグ修正なので CLAUDE.md の「複雑さが減らないリファクタ」には当たらない。
- 棄却部分: `selection=[]` 時の element kind 検証は insert 経路にも同じだけ存在する別軸の
  false-accept のため、片側だけ直すと逆向きの非対称を生む → Non-Goals 明記 + #331 委譲。

### self-review A1 (medium) — 採用 → `MAX_PATTERN_COUNT`

`count: u32` が無制限で桁ミス 1 つで OOM する。`sketch_pattern.rs` に
`pub const MAX_PATTERN_COUNT: u32 = 10_000` を追加し、`count == 0` チェックの直後・
`count == 1` の early return の前で `sketch_pattern_{linear,circular}_count_too_large` を fail-fast する。
Non-Goals に「幾何的意味の無いリソースガード」「上限が縛るのは倍率であって profile サイズではない」旨を明記。

### self-review C3 (medium) — 採用 → Circular gate のテスト追加

`t_crud_pattern_unknown_element_circular` を追加。Circular gate は Linear gate の 34 行コピペブロックで、
reason 文字列を取り違えても現行スイートでは検出できない状態だった。

### self-review C6 (medium) — 採用 → 未テスト error kind 4 種を全て固定

`sketch_pattern_linear_distance_invalid` / `sketch_pattern_circular_angle_invalid` /
`sketch_pattern_circular_center_degenerate` / `pattern_circular_duplicate_id` の 4 種。
NaN/Inf 系は 3 値 (NaN / +Inf / -Inf) をループで回し、Mirror (#298) のテスト水準に揃えた。

### self-review C1 (medium) — 採用 → plan.md の記述のみ修正 (コード変更なし)

「パラメータ検証で複製の座標一致を完全に防げる」は偽 (反例 `total_angle=4π, count=2` → `step=2π`)。
**挙動自体は Non-Goals の「多重回転による座標重複は許容」に含まれる仕様どおり**なので、
検査強化 (提案 2) は棄却し、根拠の記述だけを修正した。
事後幾何比較を不要とする真の根拠は「Non-Goals で明示的に許容しているから」である旨を明記し、
将来の Pattern 系 Issue が誤った論拠を引用しないようにした。

### 据え置き (low 6 件)

C2 (doc の「真の no-op」精緻化) のみ、A1 で同じ doc ブロックを触るため併せて修正した。
残る A4 (Arc 絶対角度の非正規化) / A7 (`mode: &'static str` 分岐) / A8 (`rename_to` の `unreachable!()`) /
C7 (空振り determinism assertion) / M7 (ADR-017 §3 の乖離) は本 Issue では対応しない。
いずれも self-review 自身が「本 Issue 内で直すか = 不要/任意」と判定済み。M7 は横断ドキュメント Issue 化が妥当。

### 追加した回帰テスト一覧

| テスト | ファイル | 対応指摘 |
|---|---|---|
| `t_crud_pattern_edit_breaks_selection_linear` | `tests/sketch_pattern_acceptance.rs` | Codex A01 |
| `t_crud_pattern_edit_breaks_selection_circular` | 同上 | Codex A01 |
| `t_crud_pattern_edit_n1_selection_ignored` | 同上 | Codex A01 (R01 契約の維持を固定) |
| `t11_crud_gate_edit_breaks_mirror_selection` | `tests/sketch_mirror_acceptance.rs` | Codex A01 (Mirror 分) |
| `t_crud_pattern_unknown_element_circular` | `tests/sketch_pattern_acceptance.rs` | C3 |
| `t_deg_distance_invalid` | 同上 | C6 |
| `t_deg_angle_invalid` | 同上 | C6 |
| `t_deg_center_degenerate` | 同上 | C6 |
| `t_deg_duplicate_id_circular` | 同上 | C6 |
| `t_deg_count_too_large` | 同上 | A1 |
| `t_boundary_count_at_max` | 同上 | A1 |

修正前に 5 テスト (A01 系 3 + A1 系 2) が赤であることを実測してから実装した
(CLAUDE.md「バグ修正は再現テスト先行」)。
