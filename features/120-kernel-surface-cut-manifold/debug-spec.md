# Debug Spec — Issue #120: kernel-surface-cut-manifold

## 仮説（確定）

`pslg_subdivide` の外側フェース（DCEL outer face）フィルタに**絶対値誤差**を使っているため、
`x_offset ≈ 4.978`（= u=-0.02190701... が2のべき乗分数でない値）で浮動小数点誤差によって
6頂点の外側フェースが誤って残存し、余分な fragment を生成する。

### 詳細な因果チェーン

1. **DCEL が3サイクルを生成する**  
   `tool face[1]`（z=+1 面、u_axis=(-1,0,0)）に対し、ターゲット右面（x=5）との交差線分が
   UV 座標 `u = x_offset - 5 = -0.02190701...` に入る。  
   DCEL は `he_next` 表を使ってサイクルを列挙し、3サイクルを得る:
   - **Cycle 1（6頂点）**: 外側 DCEL フェース ← フィルタで除去すべき
   - **Cycle 2（4頂点）**: outside-target サブフェース（x>5 の領域）
   - **Cycle 3（4頂点）**: inside-target サブフェース（x<5 の領域）

2. **フィルタバグ**  
   該当コード（`partition.rs` の `pslg_subdivide` 内）:
   ```rust
   let outer_area: f64 = outer_loop  // 4点正方形: (-1,-1),(1,-1),(1,1),(-1,1)
       .iter()
       .zip(outer_loop.iter().cycle().skip(1))
       .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
       .sum::<f64>()
       .abs()
       / 2.0;
   // → 全座標が ±1 (IEEE754 で exact) → outer_area = 4.0 EXACTLY

   sub_faces_raw.retain(|(f, _)| {
       let area = signed_area_2d(f).abs();
       area < outer_area - area_eps   // area_eps = 1e-18
   });
   ```
   - `outer_area = 4.0` (正確)
   - 6頂点フェース `signed_area_2d` は `u=-0.02190...` を含むショーレース計算で累積誤差が発生:
     最終ステップ `8.02190701330951 - 0.02190701330951` の IEEE 754 演算が `8.0 ± ε`（|ε|≈4e-16）になりうる
   - `area = (8.0 - 4e-16) / 2 = 4.0 - 2e-16`
   - フィルタ条件: `(4.0 - 2e-16) < (4.0 - 1e-18)` → **True**（意図と逆）→ 6頂点フェースが残存

3. **なぜ x_offset=4.5 は通るか**  
   `u = 4.5 - 5 = -0.5 = -1/2`（IEEE 754 で exact な2のべき乗分数）。  
   ショーレース計算の全中間値も exact → `signed_area_2d` = 4.0 EXACTLY → フィルタ正常。

4. **余分フラグメントがマニフォールド違反を引き起こす**  
   6頂点フェース（UV: [(-0.022,-1),(1,-1),(1,1),(-0.022,1),(-1,1),(-1,-1)]）の重心は
   3D で `(x_offset, 0, 1)` = `(4.978, 0, 1)` ≈ target 内部 → `InsideOther` → Cut に選択される。  
   このフラグメントのエッジ k=0: `(5,-1,1)→(3.978,-1,1)` は inside-target フラグメントの
   エッジ `(5,-1,1)→(3.978,-1,1)` と **同方向**。  
   両方が forward HE を要求 → edge に HE が1本しか作られない → `validate_manifold` 失敗:
   `"edge must have exactly 2 half-edges"`

## 関連ファイル

- **修正対象**: `crates/mycad-kernel/src/booleans/partition.rs`
  - 関数: `pslg_subdivide`
  - 行: `sub_faces_raw.retain(...)` のフィルタ条件（おおよそ行 2425-2428）

## 修正方針

フィルタを**絶対値誤差**（`outer_area - 1e-18`）から**相対値誤差**に変更する:

```rust
// Before:
sub_faces_raw.retain(|(f, _)| {
    let area = signed_area_2d(f).abs();
    area < outer_area - area_eps
});

// After:
sub_faces_raw.retain(|(f, _)| {
    let area = signed_area_2d(f).abs();
    area < outer_area * (1.0 - 1e-10)
});
```

### 根拠

- outer face は常に `area ≈ outer_area` (相対誤差 < 1e-13)
- 有効サブフェースは `area << outer_area`（最薄スライスでも `area / outer_area > 1e-6`; proptest 範囲 x_offset∈[4.1,5.4] で確認）
- 閾値 `1e-10` はこの両者を明確に分離できる

### plan.md の "Out-of-Scope" との関係

plan.md は「pslg_subdivide 内部の DCEL アルゴリズム変更」を Out-of-Scope としているが、
今回の修正は DCEL トポロジー計算ではなく**フィルタの数値閾値の修正**であり、
変更は `retain(...)` 条件の1行のみ。スコープ内と判断してよい。

## Codex R2 指摘への対応

### R2-F01 採用: pslg_subdivide フィルタを max-area 除去に変更

`area < outer_area * (1.0 - 1e-10)` は極小サブフェース（面積 < outer_area * 1e-10）を
outer-face として誤って捨てるリスクがある。

**修正**: tolerance フィルタを廃止し、**最大面積の face を1件だけ除去**する:

```rust
// Before:
let outer_area: f64 = outer_loop
    .iter()
    .zip(outer_loop.iter().cycle().skip(1))
    .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
    .sum::<f64>()
    .abs()
    / 2.0;

sub_faces_raw.retain(|(f, _)| {
    let area = signed_area_2d(f).abs();
    area < outer_area * (1.0 - 1e-10)
});

// After:
// DCEL outer face always has the maximum area; remove exactly that one.
let max_idx = sub_faces_raw
    .iter()
    .enumerate()
    .max_by(|(_, (fa, _)), (_, (fb, _))| {
        signed_area_2d(fa)
            .abs()
            .partial_cmp(&signed_area_2d(fb).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
    .map(|(i, _)| i)
    .unwrap(); // safe: len > 1
sub_faces_raw.swap_remove(max_idx);
```

`outer_area` の変数もこの変更に伴い削除する（未使用変数警告を避けるため）。

### R2-F02 採用: chain_segments_into_polygon の partners 整合修正

CCW 正規化で `pts.reverse(); partners.reverse();` の後に `partners.rotate_left(1)` が必要。

edge i は `pts[i] → pts[(i+1) % n]` に対応するため、逆転後は1シフト必要:

```rust
// Before:
if signed_area_2d(&pts) < 0.0 {
    pts.reverse();
    partners.reverse();
}

// After:
if signed_area_2d(&pts) < 0.0 {
    pts.reverse();
    partners.reverse();
    if !partners.is_empty() {
        partners.rotate_left(1);
    }
}
```

## Codex 指摘への対応（F01 採用）

Codex F01（high）: `chain_segments_into_polygon` に zero-area ループのチェックがない。

**採用修正**: `chain_segments_into_polygon` の最後（`Some((pts, partners))` を返す直前）に zero-area ガードを追加する:

```rust
// After normalizing to CCW:
let area = signed_area_2d(&pts).abs();
if area <= area_eps {
    return None;  // zero-area or degenerate loop → fall back to pslg_subdivide
}
```

ただし `area_eps` は `LENGTH_TOLERANCE * LENGTH_TOLERANCE = 1e-18` の代わりに、
`pslg_subdivide` で使われている同じ定数を参照すること。

**非採用**: 非凸フェースでセグメントが境界外に出るケース → plan.md Non-Goals に「tool/target が非凸な場合」が含まれる。Out-of-Scope のため棄却。  
**非採用**: `segments_are_interior` の段階でセグメント本体包含チェック → 同上 Out-of-Scope。

## 試した修正と結果

- [x] GLM 第1回: ring/disc branch の追加（chain_segments_into_polygon）→ proptest t02 で x_offset=4.978 がまだ失敗
- [x] cargo fmt 修正（第1回 GLM が eprintln! のインデントを崩した）→ fmt 通過、proptest は引き続き失敗
- [ ] **pslg_subdivide フィルタの相対誤差化**（今回 debug-spec として GLM に指示）

## 次にやること

以下の1行変更を `crates/mycad-kernel/src/booleans/partition.rs` に施す:

```diff
-            area < outer_area - area_eps
+            area < outer_area * (1.0 - 1e-10)
```

その後 `cargo xtask ci` で全テスト（acceptance + proptest）が green になることを確認する。

## 追加で書いてほしいテスト

GLM は既存の `dbg_surface_cut_partition_trace` デバッグテストに、
x_offset=4.97809298669049 でツール face[1] のサブフェース数が **2** になることを
アサートするコードを追加すること（マニフォールド違反の直接原因の回帰テスト）。

```rust
// 追加アサートの例（既存テスト内に埋め込む）
let tool_face1_frags: Vec<_> = tool_frags.iter()
    .filter(|f| f.source_face == 1)
    .collect();
assert_eq!(
    tool_face1_frags.len(), 2,
    "tool face[1] must produce exactly 2 sub-faces, got {}: {:?}",
    tool_face1_frags.len(), tool_frags
);
```
