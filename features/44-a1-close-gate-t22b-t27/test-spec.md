# test-spec — Issue #44 A1 close gate (T22b + T27)

## 不足テスト（plan 計画分）

| ID | 状態 | 備考 |
|----|------|------|
| T22b | ✅ 実装済み (`a1_solid_invariant_across_angular_segments`) | 計画通り |
| T27 | ⚠️ 実装済みだが期待値が計画と乖離 | 下記参照 |

## 実装差分から追加すべきテスト

### T27 期待値の乖離

**計画値**: `1000 - 20π ≈ 937.168` (box 体積 - 円柱穴の物理体積)  
**実装値**: `1000 + 20π/3 ≈ 1020.94`

GLM はコメントで「A1 Cut 結果の円柱側面 (`same_sense=true`) が外向き法線のままになっており、mesh 積分で誤った符号貢献をしている。修正時は `1000 - 20π` に戻す」と注記した。

#### 影響の評価

`a1_mesh_volume_abs()` は `vol.abs()` を返すため、符号が逆でも正値にされる。そのため:
- 現 T27 は「mesh 積分の abs 値が `~1020.94` に近い」ことを確認するテストになっている
- Issue #44 が要求する「物理体積 `~937.17` に近い」は**未確認**

#### 追加すべき検証

1. `a1_cyl_face_same_sense_check` — A1 Cut 結果の cylinder lateral face が `same_sense=false` (法線が穴の内側向き) であることを確認
   - 根拠: `assemble.rs:290` の `flip_normals` ロジックは `(Cut, true, _)` で true になるはず → cylinder 側面は same_sense 反転されているはず。GLM の観察と矛盾している
   - これが false であれば assemble.rs のバグ

2. T27 の期待値修正 — `same_sense` バグが修正されれば expected を `1000 - 20π` に戻す

## エッジケース・退化入力

- 本 Issue はテスト追加のみ。B-rep 構築コードへの変更なし → 退化ケースの新規発生なし

## 数値境界

- T22b: entity name の完全一致 → 境界値なし（exact match）
- T27: 相対誤差 < 1% → `expected = 1000 + 20π/3` で CI は pass。物理値との整合は別途確認が必要

## 決定性

- T22b で angular=8 / 64 の両極端で確認済み ✅
