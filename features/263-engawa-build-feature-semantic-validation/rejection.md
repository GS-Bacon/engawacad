<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## STEP 7 Final Review (GLM) Round 1 — 全 3 件棄却

- FN01 (critical): 「Issue #263 は『設計のみ、実装は後続』と明記、実装包含はスコープ違反」を棄却 — Issue body の "本 Issue では設計のみ、実装は後続" は **直前の 3 つの設計 question (Q1/Q2/Q3) への注釈** であって Issue 全体への指示ではない。Issue label は `type: foundation`、batch-select は `tier: foundation-batch` で deliverable=code として pick している。plan.md でも In-Scope 表で実装 deliverable を明記し、type:foundation/batch:kernel ラベル意図と整合。Issue タイトルも `feat(engawa-build): ...` で feat。GLM の文脈誤読 (hallucination)。Cycle #40 (#255) M-F01 の hallucination 棄却 precedent と同じ判断。
- FN02 (high): 「circular ref テスト未実装」を棄却 — plan.md Out-of-Scope に「環参照のグラフ検出 (history が線形なため発生不可)」と明記済み。Issue body の "circular ref" の意味論は linear history では「後方参照 (= 自身より後ろを参照)」となり、これは T05_insert_before_producer (= InsertBeforeProducer) で既にカバー済み。A→B→A のような multi-feature cycle は production order 線形性により発生不能 (前 feature しか参照不可)。
- FN03 (medium): 「producer_at の計算で `at +` していない」を棄却 — false positive。`feature_crud.rs:217-223` を読めば `producer_at: at + features.iter().enumerate().skip(at).position(|(_i, feat)| feat.id() == sketch_ref).unwrap()` で **`at +`** を含んでいる。body refs 側 (250-272 行付近) は `find_map(|(i, feat)| ... Some(i))` で `enumerate()` の絶対 index `i` を返しているため `at +` 加算不要。両経路とも実装は正しい。GLM の読み取りミス。

判定根拠: blocking=2 だが critical=1 は scope/解釈問題で実コード修正なし、high=1 は Non-Goals 該当、medium=1 は false positive。GLM 修正 dispatch しても "修正すべきコード" が存在せず無限ループになる。Cycle #40 #255 precedent (M-F01 hallucination 棄却) に従い、Claude 裁量で受け切り → STEP 7.5 Codex 独立 gate で再検証する。

## STEP 7.5 Codex 3-persona Review Round 1 — C-F02 を scope-defer

- C-F02 (high, contrarian 単独): `check_no_downstream_break` が `CreateSketch.plane_ref = PlaneRef::Entity(...)` の implicit body lifetime 依存を見ない → 一部採用 (有効指摘) だが本 Issue scope を越える複雑度: EntityRef::Named.feature_id / EntityRef::Derived chain の再帰 traversal が必要。Issue #263 body の 4 variants と direct ref に限定する scope を守り、別 Issue #264 を起票 (type: foundation, batch:kernel, milestone Phase 9) して defer。plan.md Non-Goals にも追記。
- A-F01 / C-F01 / M-F01 (critical, 3 persona 一致): 新規 acceptance test file が untracked → **採用**。STEP 8 直前で `git add` する手順を `pre-step8-check.ts` のガード経路でカバー (既存のガード)。procedural fix のみ。
- A-F02 / M-F02 (high, 2 persona 一致): sketch 後方探索が CreateSketch 限定でない → **採用**。GLM 修正 dispatch で `feature_crud.rs` の sketch 経路を `Feature::CreateSketch` のみマッチに修正し、regression test (T17_mixed_type_id_collision) を追加する。

## STEP 7.5 Codex 3-persona Review Round 2 — A-F01 を scope-defer、C-F01 を non-blocking 記録

- A-F01 (high, architect 単独): `simulate_history` が pre-existing history の broken ref を defensive に検証しない → **部分採用** (有効指摘) だが、本 Issue #263 scope は「新 feature の正当性検証」に限定。pre-existing history の整合検証は別 axis (build_assembly が既に最終 gate)。別 Issue #265 を起票 (type: foundation, batch:kernel, milestone Phase 9) して defer。plan.md Non-Goals にも追記。
- C-F01 (medium, contrarian 単独, 非 block): `next.validate()` より semantic check が先に走るための診断順序問題 → **記録のみ採用**、`codex-findings.md` に詳細を記載。medium severity で blocking=0 なので merge を止めない。改善余地ありだが Issue #263 の射程外 (variant 4 個の semantic と validate 順序は別 axis)。
- migration persona: round 2 で verdict=pass (issues 0 件)。

判定: blocking=1 (high) で critical=0、A-F01 は code 系だが scope-defer 妥当 → Claude 裁量で受け切り (cycle #40 #255 precedent と一致)、STEP 8 へ進む。codex_loops=1 で fix-dispatch 上限 (5) には到達していないが、A-F01 への "修正" は #265 起票そのもので、コード変更なしで Codex を再度同じ findings に向かわせるのは無意味なため、ここで受け切るのが効率的。


