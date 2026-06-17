# Codex final review findings (#216)

非 blocking medium/low の指摘記録。次サイクル以降で別 Issue として拾うか、後続 Phase で吸収する。

## Round 3 (blocking=0, verdict=pass)

- **F01 (medium)**: `t10_negative_radius_graceful` の命名・契約曖昧化
  - 指摘: 入力は「負半径パラメータ」ではなく 8 角形の **巻き方向違い** (reversed winding)。さらに `Ok` と複数 `Err` variant をすべて許容しているため、巻き方向処理の回帰を検出できない。
  - 推奨対応: 命名を `reversed_winding` 等に改め、`Ok` か特定 `Err` variant のいずれかに 1 つ pin する。
  - 本 Issue での扱い: T10 の目的は plan 上「panic でない (graceful error)」であり、ground-truth pin が plan 時点で未定義だった (negative radius の挙動が validate_profile_closed と extrude のどちらで弾かれるか実装依存)。本 medium は #216 の核 (Face EntityRef → SketchPlane 解決) と独立であり、巻き方向ハンドリング契約は **本 Issue のスコープ外**として扱う。後続 Issue (Phase 10 スケッチ拡張、または巻き方向統一の専用 ADR) で正式 pin する。
  - 影響: medium のみ・blocking=0・STEP 7.5 verdict=pass。

## Round 2 で消化済

- F01 (critical): 新規ファイル untracked → cad ブランチで commit 済 (`2aa8fcd`)。git diff から可視化。
- F02 (medium): T05 の `Debug` 部分一致 → `matches!(err, KernelError::InvalidParameter { kind } if *kind == "profile")` で strict pin。
- F03 (medium): T11 の `unused 'solid'` → 削除。

## Round 1 から残存し r2 で吸収

(なし — r1 は r2 でリセット)
