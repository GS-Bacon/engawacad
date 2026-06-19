# Codex 独立 review findings (#269)

Codex 3 persona aggregate: **verdict=pass, blocking=0 (critical=0, high=0)**, medium=2, low=2.
本ファイルは medium/low (非 block) を記録するメモ。

## Contrarian persona

### C-F01 (low) — skeleton ファイルの未削除

- file: `crates/engawa-build/tests/refs_resolve_transitive_plane_ref_acceptance.rs`
- finding: STEP 5.5 で生成した skeleton ファイルが 10 件の `#[ignore] todo!()` のまま残っており、CI ignored count を汚す。
- 採用判断: **partial 採用**。test-spec.md の Follow-up メモにも同件を既記録 (Claude が STEP 6.5 で先に検出していた)。削除を試みたが `crates/**` 破壊操作 guard でブロック。次サイクル B-7 sweep もしくは別 Issue で対応する。本 Issue では merge を優先し、本 finding は **既知** として記録する。

### C-F02 (medium) — 決定性検証の偏り

- file: `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs:1507`
- finding: 決定性テストが失敗パス比較に偏っており、成功パスの byte-equal が無い。
- 採用判断: **scope-defer**。本 Issue の T01 (決定性) は plan で「同一入力 → 同一 ID 順序の一致」を求めており、`t_269_determinism` がそれを満たしている。成功パスの YAML byte-equal は engawa-format 側の serialization 決定性問題で、本 Issue (engawa-build feature_crud の skip 判定強化) のスコープ外。将来別 Issue で format crate に追加するべきテスト案として記録するが、本 Issue では追加しない。

## Migration persona

### M-F01 (medium) — skeleton ファイル未削除 (再掲)

- file: 同上 (skeleton ファイル)
- finding: 計画 ID と実装テスト関数名がファイル分離で乖離、coverage_hints=0 のまま。
- 採用判断: C-F01 と同件。**partial 採用** (test-spec.md follow-up と同記録)。次サイクル sweep か別 Issue で対応。

### M-F02 (low) — `EDGE_269_*` ブロックの scope 越境

- file: `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs:1650`
- finding: GLM が追加した YAML roundtrip / NaN / empty id 検証は engawa-format の serialize 挙動を engawa-build 側で固定しており、scope 越境。
- 採用判断: **scope-defer**。本 Issue の merge は blocking 0 で進める。後続 format/migration 作業前にこれらテストを engawa-format 側に移すか削除するか、follow-up Issue で判断する。
