<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## STEP 3.5 Codex

- R01 (完全採用部分の棄却): 「CRUD gate (`refs_resolve_in_state`/`check_refs_resolve_before`) を current profile
  ベースに再構築する」という Codex の完全修正案は棄却 — `simulate_history` が kernel の幾何 op を実行する必要があり
  既存設計判断 (「history gate is a ref-resolution check, not a geometry validator」) の反転になる。Fillet/Offset
  の gate にも波及し `build_bodies_from_features` の profile replay ロジックを `feature_crud` へ共有抽出する
  refactor を伴うため ADR-006 の 1 Issue = 1 GLM サイクル粒度を超える。plan.md Non-Goals に既知の制約 (false-reject/
  false-accept 双方向) として明記し、T10/T11/T12 で public contract として固定した上で、別 follow-up Issue
  (`type: refactor, batch:kernel`) へ先送りする (Opus 4.7 委譲判定: partial)

## STEP 7.5 Codex

- M01 (critical): 「tracked diff に `sketch_chamfer.rs` / `sketch_chamfer_acceptance.rs` /
  `examples/sketch_chamfer.engawa` が含まれず、clean checkout ではコンパイル・テストが成立しない」を棄却 —
  `dispatch-codex.ts:105` の `git diff ${baseBranch}...HEAD` が **untracked ファイルと unstaged 変更を
  除外する**ツール側の制約による偽陽性。`/3ai` 規約 (STEP 8 の `finalize-feature.ts` まで手動 `git add` 禁止) の
  結果 3 ファイルは正常状態として `??` であり、STEP 8 の squash コミットで差分に含まれる。
  実装欠陥ではないため修正不要。
  - 独立検証: `git status --short` で 3 ファイルすべて `??` を確認。
    `git diff origin/HEAD...HEAD --stat` は `web/src/generated/Feature.ts` 18 行のみ (実装差分が Codex に
    渡っていない) を確認。実 working tree で `sketch_chamfer_acceptance` 32 passed /
    `examples_smoke` 29 passed / `golden_examples` 22 passed (`golden_sketch_chamfer` 含む) を再実行で確認。
    `ci.log` も `=== All CI checks passed ===`。
  - Codex は `git status --short` / `git diff --no-index -- /dev/null <file>` / `sed` で 3 ファイルの中身を
    実際に読んでレビューしており (`codex-final.yaml.log`)、M01 は diff の梱包方法への指摘であって
    実装の欠陥報告ではない。入力 `claude-self-review.md:4` にも「未コミット (working tree + untracked) の状態で
    `cargo xtask ci` green」と明示していた。
  - ツール側の構造的バグとして #330 (`bug` / `batch:skill`) を起票。
    `extract-test-summary.ts` / `detect-deliverable.ts` / `check-diff-coverage.ts` /
    `dispatch-glm-review.ts` / `check-spec-divergence.ts` も同じ `git diff` 前提を共有する横断的影響を記載。
