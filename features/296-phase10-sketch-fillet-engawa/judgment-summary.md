<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R01: 採用 → plan の「...」節を修正 -->
<!-- - R02: 棄却 → Non-Goals に記載済みの蒸し返し -->
<!-- - R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue に委譲 -->

## Round 1

- IN01 (medium, invariant): 部分採用 → 決定性懸念自体は棄却 (element_id は非決定要素を含まない total function)。ただし「実装場所を GLM 実装時に先送りしている」点は妥当と判定し、plan 側で確定: helper 一式は sketch_fillet.rs 内に複製 (geometry/math.rs への切り出しは engawa_format 依存混入のため却下)。Opus 4.7 subagent 判定。
- AM01 (low, ambig): 採用 → normalize_angle は repo に未実装と判明 (grep 0 hits)。契約 ((-π, π] 正規化) とコード例を plan に追記。調査中に GLM 4ペルソナ全員が見落としていた派生問題 (fillet の負 sweep Arc と sketch_offset::offset_arc の arc_negative_sweep 拒否の潜在衝突) を発見し、数値モデル節・In-Scope 表・テスト計画 (T08)・Non-Goals に追記。Opus 4.7 subagent 判定。

## Round 2

- IN01 (critical, invariant): 棄却 → 誤検知。SketchElement::id (String, format layer) と EntityId=u64 (IdGenerator, B-rep layer) の混同。ADR-017:93-96 が派生 element ID を文字列フォーマット (`{a}_split_{n}`) で決定的に割り当てると明示規定し、同 ADR Decision Matrix (:153) はカウンタ再採番を ADR-005 と衝突するとして棄却済み。format!("{a_id}_{b_id}_fillet_arc") は入力のみに依存する純関数で決定性要件を満たす。IdGenerator 導入は History 編集での ID シフトを招き ID 安定性を後退させる。Opus 4.7 subagent 判定 (実地調査: topology.rs:11, feature.rs, sketch_offset.rs, ADR-017 引用済み)。
- IN02 (high, invariant): 部分採用 → 前提 (IdGenerator 導入) は IN01 と同根で棄却。ただし T01 の検証不足という観察は正当と判定し、(a) T01 に派生 Arc ID の文字列完全一致 assert を追加、(b) T01d (IdGenerator 込みの build 全体決定性) を新設、(c) T01c (Arc ID衝突時の fail-fast) を新設 + `sketch_fillet_arc_id_collision` ガードを apply_sketch_fillet_build に追記。Opus 4.7 subagent 判定。

## Round 3

- AM03 (low, ambig): 棄却 → ペルソナ自身が「値は固定済みのため、指摘の必要はない」と自己完結的に結論しており、数値モデル節で ANGLE_TOLERANCE=1e-9 は既に固定済み (実装者への判断丸投げではない)。Claude 判定 (軽微・自己解決のため Opus 委譲省略)。
- AM04 (low, ambig): 棄却 → ペルソナ自身が「実装レベルで担保済み」と結論。`as_line` ヘルパーが Line 以外を渡された場合に `UnsupportedFeature{kind:"sketch_fillet_only_line_line"}` を返す設計により、fillet 済み Arc を挟んだ再 fillet は自動的に拒否される。Claude 判定。
- AM05 (medium, ambig): 棄却 → ペルソナ自身が「必須ではない」と結論。plan.md には既に「却下: geometry/math.rs への切り出し」「実装時に判断を委ねない、確定事項」と明記済みで、追加の念押し文言は冗長 (メタワーク回避)。Claude 判定。

## Round 4

- NU01 (medium, numeric): 棄却 → 事実誤認。plan.md には `### 数値モデル` セクションが290行目に既に存在する (`grep -n "^### 数値モデル" plan.md` で確認)。Claude 判定。
- NU02 (medium, numeric): 棄却 → 本 Issue と無関係な hallucination。「共面する2面 (dihedral angle)」「KernelError::DegenerateInput」は Boolean/Partition 系 (3D面演算) の概念で、2D Sketch Fillet (Line-Line corner) には適用されない。`KernelError::DegenerateInput` は `crates/engawa-kernel/src/error.rs` に実在しない (既存 variant は `DegenerateSketchElement`/`DegenerateBooleanContact`/`DegeneratePcurve` のみ、STEP 2 調査時に全件確認済み)。ADR-004 §3.2 という引用も本 Issue の数値モデル節が引用する ADR-004 の既定 ε 値の話とは無関係。Claude 判定。

## STEP 3.5 Codex

- R01 (high, blocking): 部分採用 → 中核 (feature_crud.rs の refs_resolve_in_state が sketch 存在しか見ておらず、CreateSketch profile の rename/reorder edit で history gate をすり抜け build で初めて壊れる) は実地確認で正しいと判定、かつ SketchOffset には存在しない新規リスクと確認 (Offset の selection は未知IDを黙って無視するため gate/build が乖離しない)。対応として (a) kernel 側に `find_adjacent_pair` を pub fn として切り出し feature_crud.rs から再利用 (ロジック重複ゼロ)、(b) refs_resolve_in_state / check_refs_resolve_before に element-level 参照ゲートを追加、(c) FeatureCrudError::SketchElementNotResolved を新設、(d) set_feature_suppressed の抜け漏れを追加 (exhaustive match で必須と判明)、(e) T10/T11/T12 追加。Codex suggestion の「effective profile を再生」(= simulate_history 内で幾何計算を再実行) は棄却 (CRUD層とbuild層の二重実装・発散を招くため、元profileでの近似で十分かつ健全)。Opus 4.7 subagent 判定。
- R02 (medium, non-blocking): 部分採用 → validate_sketch_profile_contours の保証範囲を過大表現していた記述を修正。Codex 提案の Euler-Poincaré assert は検出力がほぼゼロ (make_extrusion の prism構成上恒真) と判明したため、真の処方である「fillet後profileのendpoint連結性を直接検証」を T09 として新設。Euler assert 自体は1行・低コストなので回帰ネットとしてT04/T06に追加するが検出力の限界を明記。tessellate_sketch_element が Line.to を無視する事実 (T09が必要な根拠) も追記。Opus 4.7 subagent 判定。

## STEP 7.5 round 2 (Codex)

- A01 (high, blocking): 部分採用 → gate 実装変更は棄却 (plan.md 既存 scope 決定の再指摘、かつ Codex の帰結「build で sketch_fillet_no_shared_corner に落ちる」は事実誤認 — 全 Line profile は validate_profile_closed が CreateSketch 時点で先に捕捉する)。到達可能な残余 (mixed Line+Arc profile) は Non-Goals 領域、かつ同種の穴は Extrude 分岐にも #296 以前から存在するため fillet だけを部分硬化するのは一貫性を欠く。到達不能性を実行可能な assertion で固定する回帰テスト T13 を追加し、plan.md の不正確な記述 (「取りこぼしは1種のみ」) を訂正。Opus 4.7 subagent 判定。
