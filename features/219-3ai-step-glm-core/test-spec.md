# test-spec — Issue #219

## 不足テスト（plan 計画分）

以下は STEP 5.5 で `#[ignore]` 付き skeleton として配置済み。STEP 6.6 (GLM テスト実装) で `#[ignore]` を外して本体を実装する。

### 1. `crates/engawa-build/tests/cuboid_feature_id_acceptance.rs`

- `t02_create_box_propagates_feature_id_to_face_names` — 既にバグ再現可能な本体を持つ。コア実装で fix されたはずなので **`#[ignore]` 行を削除する** のみ。本体修正は不要。実行で pass することを確認する。

### 2. `crates/engawa-kernel/tests/cuboid_feature_id_acceptance.rs`

GLM が `todo!()` を実装し、それぞれの `#[ignore]` を外す:

| 関数 | 期待挙動 |
|------|---------|
| `t01_determinism` | `make_cuboid(10,20,30, "cuboid", &mut IdGenerator::new(0))` を 2 回呼び、全 vertex/edge/face id・position・name が完全一致。`assert_eq!(s1.vertices.len(), 8)` 等の構造アサート + 2 つの Solid 比較を全フィールドで実施。 |
| `t03_fid_propagation_to_named_entities` | `make_cuboid(1,1,1, "my_box", &mut gen)` → 全 vertex/edge/face の `name.as_ref().unwrap()` が `EntityRef::Named { feature_id: "my_box", .. }` であることを assert。 |
| `t04_degen_empty_fid` | `make_cuboid(1,1,1, "", &mut gen)` を呼ぶ。`EntityRef::try_named` が空文字列を `Err` (= FormatError) で reject する仕様であれば、`name` フィールドが `None` になる (cuboid.rs では `.ok()` で吸収)。**観察した実挙動を assert に固定** する: 空 fid の場合に `make_cuboid` 自体は Ok を返し、全 entity の name が `None` になることを確認する (現在の `.ok()` ガード semantics を保護)。 |
| `t05_boundary_long_fid` | `let long_fid = "a".repeat(256);` を渡して `make_cuboid` を呼び、Face/Edge/Vertex の `name.as_ref().unwrap()` の `feature_id` が同じ 256 文字 (truncate 等の意図しない処理がないこと) であることを assert。 |

## 実装差分から追加すべきテスト

GLM の core 実装で予期せず追加された分岐や挙動を git diff で確認した結果:
- `make_cuboid` の `feature_id: &str` 追加以外に **public API の挙動変更なし**。`fid` 引数は内部 `EntityRef::try_named` 呼び出しの第 1 引数にのみ転写されている。
- `crates/engawa-build/src/lib.rs:348` の `make_cuboid(*width, *height, *depth, id, gen)` で `Feature::CreateBox.id` (`&str`) が直接渡されている。
- 他の primitives (cylinder/sphere/extrusion) は **本 Issue scope 外** のため変更されていない。

→ 上記 T01〜T05 + T02 で必要十分。追加すべきテストは **なし**。

## エッジケース・退化入力

T04 (空文字列 fid) と T05 (長文字列 fid) で `feature_id` 文字列軸の退化/境界をカバー済み。

数値軸 (dx/dy/dz の退化) は既存テスト (`test_make_cuboid_invalid_dimensions`, `t04_make_cuboid_*`) でカバー済みのため重複追加不要。

## 数値境界

数値ロジック変更なし → 追加不要。

## 決定性

`fid` 引数は `IdGenerator.next()` 呼び出し順序に影響しない (`EntityRef::try_named(fid, ...)` の戻り値文字列に転写されるのみ)。T01 で同じ fid + 同じ IdGenerator seed → 同一 Solid を保証。

異なる fid を渡すと name 文字列のみ異なり、ID・position は同じになるべき (これも T03 で `fid="my_box"` 結果が `fid="cuboid"` の T01 結果と ID 一致することを副次的に確認すると望ましい — GLM の判断に委ねる)。
