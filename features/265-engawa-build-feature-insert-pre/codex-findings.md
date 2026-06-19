# STEP 7.5 Codex Findings (#265)

## Round 1 (2026-06-19)

### Verdict
- aggregate: **fail** (blocking=5, critical=2, high=3)
- per persona: architect 1 critical / 1 high / contrarian 1 critical / 1 high / migration 0 critical / 1 high
- Note: 3 persona すべてで `[CODEX_USAGE_LIMIT]` notice (rate limit 到達) ですが yaml は完全な findings を含む。

### Findings 処理

#### Critical: untracked test file (architect F01 + contrarian F01)

- **判定**: 採用 (procedural fix)。`git add crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs crates/engawa-build/src/feature_crud.rs` を実行して staging に追加。pre-step8-check が後段で同じ guard を持つため、STEP 8 直前で必ず気づく経路だが、Codex の指摘を受けて即時対応。

#### High: forward scan に atomic skip 未適用 (architect F02 + contrarian F02 + migration F01)

- **判定**: 採用 + 修正 (debug-spec 経由で GLM 再 dispatch)。
- 3 persona すべてで同じ方向の指摘 (executed_at_full を forward scan で使う必要あり)。
- 修正内容: simulate_history の戻り値を `(sketches_at, live_bodies_at, executed_at)` 3-tuple に拡張、CreateBox/Cylinder/Sphere/CreateSketch は無条件 executed、body producer は `refs_resolve_in_state` true 時のみ executed。`check_refs_resolve_before` の body refs / implicit body refs `producer_after` 走査と `check_no_downstream_break` の consumer + re_registered ループに `executed_at_full.contains(&i)` ガードを追加。
- FORWARD01-04 acceptance test を追加。GLM 再 dispatch (mode=core + debug-spec) で実装、CI green。

## Round 2 (2026-06-19)

### Verdict
- aggregate: **fail** (blocking=2, critical=0, high=2, low=1)
- per persona: architect 1 high (verdict: fail) / contrarian 0 (verdict: pass with low) / migration 1 high (verdict: fail) ※migration は usage_limit notice あり
- 注: critical=0 (Round 1 から大幅改善)

### Findings 処理

#### Rejected → rejection.md に記録

- **r2-A-F01 (architect, high)**: 「CreateSketch も atomic skip 対象に (plane_ref=Entity 解決失敗 → inert)」
  - **判定**: 棄却 (scope-defend)。本 Issue plan.md Non-Goals 明示 (`CreateSketch の sketches_at register 抑制 — #264 で別レイヤ`)。#264 (plane_ref lifetime tracking) と #266 (transitive sketch user) で別 Issue 追跡。

#### Scope-deferred → 新規 Issue #267 起票

- **r2-M-F01 (migration, high)**: 「post-insert re-simulation が必要 (insert で missing ref が供給され previously skipped feature が live 化する稀ケース)」
  - **判定**: scope-defer to #267。executed_at_full は insert 前の状態で算出されており、insert された feature が missing ref を供給するケースは別アルゴリズム (post-insert re-simulate) が必要。本 Issue の Issue body literal scope (= prefix simulation) から外れる adjacent case。
  - **follow-up Issue**: #267 (feat(engawa-build): Feature insert で post-insert re-simulation - missing ref が新規 insert で供給される稀ケースの defensive validate)

#### Accepted → Claude 直接 edit (tests/ guard 緩和)

- **r2-C-F01 (contrarian, low)**: 「`t_boundary_existing_acceptance_passes` が `assert!(true)` だけ → 実質回帰カバレッジなし」
  - **判定**: 採用。具体的な 3 ケース (CreateBox tail / DuplicateFeatureId / clean Cut at tail) を assert する形に書き換え。tests/ 配下なので Claude 直接 Edit 可能。
  - 修正後: t_boundary が 3 つの実 assertion を持ち、`cargo test t_boundary_existing_acceptance_passes` で 1/1 pass を確認。

### Claude 裁量 codex_review passed

- critical=0、残 high は (a) 棄却 (scope-defend) / (b) scope-defer (#267) のいずれかで処置済。code 系の未処置 critical/high なし。
- contrarian r2 は verdict: pass。architect/migration r2 の usage_limit notice は migration のみだが、yaml 内容は完全な findings を含む。
- 同系統指摘の無限ループ兆候なし (r1 → r2 で blocking 5 → 2、critical 2 → 0)。
- cycle #40 precedent (scope-defend high + 受容 low + Claude 裁量 codex_review passed) に従い `codex_review=passed` に倒し STEP 8 へ進む。
