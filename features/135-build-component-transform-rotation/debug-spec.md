# Debug Spec (Round 1 → Round 2)

## 仮説

前回 (glm-runs=1) の実装は `euler_to_matrix` のシグネチャを deg→rad に変更したが、
`crates/mycad-kernel/src/brep/topology.rs` 内の inline `#[cfg(test)] mod tests` の
`euler_to_matrix(<deg>, ...)` 呼出し (7 箇所) を追従修正しなかったため、CI で 2 件 fail:

- `brep::topology::tests::t02_rotate_90deg_face_normals`
- `brep::topology::tests::t08_boundary_180`

これは plan.md 「変更箇所 3」の追従対象列挙に `brep/topology.rs` が抜けていたことが原因 (plan は今回更新済み)。

## 関連ファイル

`grep -rn 'euler_to_matrix' crates/` 結果:

- `crates/mycad-kernel/src/geometry/transform.rs` — 関数定義 + inline tests
- `crates/mycad-kernel/src/brep/topology.rs:1495,1513,1547,1638,1669,1700,1718` — inline tests **(これが追従漏れ)**
- `crates/mycad-kernel/tests/rotate_acceptance.rs` — 既に追従修正済み（と推定）

## 修正方針

1. **完全列挙**: シグネチャ変更前に `grep -rn 'euler_to_matrix' crates/` を実行し、出力 list を debug-spec.md に貼ること。
2. **`src/` 配下も対象**: `tests/` 配下のみ走査では不十分。`src/**/*.rs` 内の `#[cfg(test)] mod tests` も追従対象。
3. **置換規則**: 各 `euler_to_matrix(<expr1>, <expr2>, <expr3>)` を `euler_to_matrix((<expr1>).to_radians(), (<expr2>).to_radians(), (<expr3>).to_radians())` に置換。
4. **値が `0.0` の場合**: `0.0.to_radians()` でも問題なし (= 0.0)。可読性のためそのままで良い。
5. **置換後の確認**: `cargo test -p mycad-kernel --lib brep::topology::tests` と `cargo test -p mycad-kernel --test rotate_acceptance` の両方で green を確認すること。

## 試した修正と結果

- [x] (Round 2 完了)

**Round 2 修正内容**:
1. `crates/mycad-kernel/src/brep/topology.rs` の 7 箇所を修正:
   - L1495: `euler_to_matrix(30.0, 45.0, 60.0)` → `.to_radians()` 付与
   - L1513: `euler_to_matrix(90.0, 0.0, 0.0)` → `.to_radians()` 付与
   - L1547: `euler_to_matrix(30.0, 45.0, 60.0)` → `.to_radians()` 付与
   - L1638: `euler_to_matrix(90.0, 0.0, 0.0)` → `.to_radians()` 付与
   - L1669: `euler_to_matrix(0.0, 0.0, 0.0)` → コメント追加 (0° = 0 rad)
   - L1700: `euler_to_matrix(180.0, 0.0, 0.0)` → `.to_radians()` 付与
   - L1718: `euler_to_matrix(45.0, 30.0, 60.0)` → `.to_radians()` 付与
2. `cargo fmt --all` でフォーマット修正
3. CI 確認: Rust 側 (build/test/clippy/fmt) 全て green

**結果**: `cargo test --workspace` 28 tests passed, `cargo clippy` clean, `cargo fmt --check` clean

## 次にやること

1. `grep -rn 'euler_to_matrix' crates/` を実行して完全 list を作る
2. 既に rad 化済み (Round 1 の編集結果) の箇所と、未修正 (Round 1 で見落とした) 箇所を区別する
3. 未修正箇所のみ rad 化置換を適用
4. `cargo xtask ci` を実行して green 確認

## 追加で書いてほしいテスト

不要。既存テスト群 (t01-t08 inline + rotate_acceptance.rs + transform_rotation_acceptance.rs) で十分。
