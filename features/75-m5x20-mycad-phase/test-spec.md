# Test Spec — Issue #75: stdlib M5x20 ボルト .mycad ファイル

## 不足テスト（plan 計画分）
T01/T02/T03 は `crates/mycad-build/tests/m5x20_acceptance.rs` に `#[ignore]` スケルトンとして存在。
GLM テスト実装でこれらの `#[ignore]` を外し、内容を実装する。

### T01_parse_ok
- M5x20.mycad を `Document::from_yaml` または `serde_yaml::from_str` でパース
- `features.len() == 2` を確認

### T02_shaft_cylinder_params
- features[0] の type が `CreateCylinder`
- `radius == 2.5`, `height == 20.0` を確認

### T03_boundary_head_origin
- features[1] の type が `CreateCylinder`
- `origin == [0.0, 20.0, 0.0]` を確認

## 実装差分から追加すべきテスト
- なし。実装は YAML ファイル追加のみで Rust コード変更なし。
- `m5x20_bolt()` smoke テスト（examples_smoke.rs）は既に green。

## エッジケース・退化入力
- N/A — YAML ファイルは静的テキスト。退化入力（空 features 等）は別 Issue の領域。

## 数値境界
- T02: `radius` / `height` の値検証は `== 2.5` / `== 20.0` で exact（YAMLデシリアライズは lossless）
- T03_boundary: `origin[1] == 20.0` で exact

## 決定性
- YAML デシリアライズは決定的（同一バイト列 → 同一 struct）。個別テスト不要。
