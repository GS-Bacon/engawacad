# Claude Self-Review for #274

GLM が見落としている弱点を 3 観点 (architect / contrarian / migration) で探した結果。
Codex 7.5 finding 率を下げる shift-left 目的。

## architect 観点 (既存 invariant / API 契約 / B-rep トポロジー保証)

### CSR-A01 (medium): Ellipse の `major >= minor` 不変条件が enforcement されていない

- ADR-017 §1 表は `Ellipse { ... }` の Requires に "`major >= minor > 0`" と明記しているが、`tessellate_sketch_element` の Ellipse case では退化判定 (`major < ε`, `minor < ε`, `minor/major < ε_axis_ratio`) しか行わず、`minor > major` のケース (例: `major=1, minor=2`) が `axis_ratio = 2.0 > EPS_AXIS_RATIO` で通過してしまう。
- 結果: parametric form `(major·cos t, minor·sin t)` は数学的には正しく ellipse を描くが、major/minor の "semantic 役割" は逆転 (実質の長軸は y 方向)。ADR-017 の invariant 違反 = 後段で `major = 長軸長` を仮定する code (たとえば bounding box 推定 / arc-length 概算) で誤った結果になる潜在リスク。
- **採用判定**: medium で記録。Phase 10 baseline では downstream consumer (extrude 等) がこの semantic を直接消費しないため即時 panic 経路にはならず、Phase 11+ で `major =長軸長` を仮定する consumer (例: scale-relative tolerance / adaptive sampling) が追加されたタイミングで invariant check を追加すれば足りる。本 Issue では post-merge follow-up Issue 候補として記録。

### CSR-A02 (low): 非有限 ellipse 入力の reason string が rotation 専用になっている

- sketch.rs:78-84 で `!major.is_finite() || !minor.is_finite() || !rotation.is_finite()` の場合 `reason: "non-finite rotation"` を返すが、実際は major/minor の non-finite ケースもこの分岐に入る。
- メッセージ精度の問題のみ (機能に影響なし)。**棄却**: post-merge cleanup。

## contrarian 観点 (採用方針の反論可能性)

### CSR-C01 (medium): Conic 固有値分解で `B² + (A-C)²` が極小のときの cancellation 未対策

- sketch.rs:130-140 で `λ = (A+C)/2 ± sqrt(((A-C)/2)² + (B/2)²)` を計算。`A ≈ C` かつ `B ≈ 0` のとき (= ほぼ円形 conic) sqrt 内が極小で `λ1 ≈ λ2` となり、固有ベクトル方向が不定 (= 主軸決定が numerically unstable)。
- plan.md §数値モデル "数値安定性" 段落では「`|λ1| >> |λ2|` で `sqrt(-F'/λ_i)` cancellation は base_segments × 2 で mask」と書いたが、`λ1 ≈ λ2` の degenerate 主軸方向は別問題。
- 影響範囲: Conic 入力で `A ≈ C, B ≈ 0` のとき、主軸方向が NaN や任意方向に倒れる可能性。実際の出力では `cos_r, sin_r` が `f64::NAN` になり、最終的に NaN 座標を返しうる。
- **採用判定**: medium。Phase 10 baseline では Conic 主軸計算の robustness は scope-defer 扱いで、test 3a/3b の代表ケース (`A != C` or `B != 0`) のみカバー。`A ≈ C, B ≈ 0` の "near-circular conic" は plan の Non-Goals 領域 (固有値計算の cancellation は Phase 11+ で再評価) に該当するため記録のみ。post-merge follow-up Issue 候補。

### CSR-C02 (low): hyperbola の `t ∈ [-2, 2]` clip 範囲がマジックナンバー

- sketch.rs:200 付近で hyperbola 分枝の AABB clip 範囲を t ∈ [-2, 2] と固定。これは `cosh 2 ≈ 3.76, sinh 2 ≈ 3.63` でユニット双曲線の主分枝を ±4 程度の範囲でサンプル。
- plan.md は "AABB clip 範囲のみ返す" としていたが、実装は固定 `[-2, 2]`。これは仕様準拠だが値の根拠が plan に明記されていない。
- **棄却**: 動作正常、範囲設定は実装判断 (Phase 11+ で adaptive 化時に再評価)。post-merge cleanup。

## migration 観点 (既存テスト互換 / 後方互換性 / public API 破壊)

### CSR-M01 (info): 既存 SketchElement consumer の exhaustiveness 確認

- 確認済 path:
  - `crates/engawa-build/src/lib.rs:520-524` validate_sketch_element_ids: Ellipse/Conic match arm 追加済
  - `crates/engawa-build/src/lib.rs:561-580` validate_profile_closed: Line 専用フィルタなので非 Line variant は無視 (Ellipse/Conic auto-closed 前提) — **OK**
  - `crates/engawa-format/src/feature.rs` Deserialize Tagged enum: Ellipse/Conic 追加済 (生成 TS と整合済)
  - `web/src/generated/SketchElement.ts`: gen-ts で同期済 (intermediate commit 済)
- 未確認 path: `engawa-cli` で SketchElement を pattern match する箇所があれば exhaustiveness エラーになっていない (= 既存パターン全て catch-all 経由)。grep 結果は `engawa-build` 以外で SketchElement::Line/Circle/Arc を直接 match する箇所は kernel/tessellation/sketch.rs のみ。**OK**
- **結論**: 弱点なし。

## 結論

検出弱点 (severity 別):
- critical: 0 件
- high: 0 件
- medium: 2 件 (CSR-A01 ellipse invariant 未強制 / CSR-C01 near-circular conic 数値不安定)
- low: 2 件 (CSR-A02 reason string 不正確 / CSR-C02 マジックナンバー)
- info: 1 件 (CSR-M01 exhaustiveness 確認結果)

すべて medium 以下のため STEP 7 へ進む。CSR-A01 / CSR-C01 は post-merge follow-up Issue 候補として記録 (Phase 11+ adaptive sampling 評価時に再考)。
