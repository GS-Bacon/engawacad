## Round 1

- R01 (high): 採用 → §設計判断 と Non-Goals (Self-intersection) と STEP 2 を改訂し、`validate_boolean_input` で「outer_loop が 1HE `Curve::Circle` の analytic loop は頂点数チェック除外」分岐を導入。T13 (planar Circle outer_loop accept 回帰テスト) を追加。これで cylinder cap 入力が通る
- R02 (medium): 採用 → §設計方針/決定性要件 と §シグネチャ と STEP 4 を改訂し、`reconstruct_intersection_curve` を BTreeMap 集約ではなく **入力 segments 順を保った 1-pass merge** に変更。`t_range` は endpoint 座標から `atan2` で逆算 + unwrap
- R03 (medium): 採用 → §退化幾何の扱い と STEP 3 を改訂し、`dedup_and_filter_arc_samples` helper を導入 (連続重複点 dedup + chord 長 `< LENGTH_TOLERANCE` 間引き、残存 2 点未満 skip)。T11 (極小弧 skip) と T12 (人工 dedup) を追加
- R04 (low): 採用 → §B-rep トポロジー妥当性 とテスト ID 表に T31 (A1 Euler) / T32 (A3 Euler + nesting) を独立テストとして追加。STEP 7 にも反映

## Round 2

- R01 (high): 採用 → partition.rs の `PlaneData::from_surface(..).unwrap()` plane-only 前提を解消する STEP 3 を追加 (`Vec<Option<PlaneData>>` 化、coplanar 判定の `Some` ガード)。A3 の sphere face 入力で panic しないことを T14 で検証
- R02 (high): **棄却** (rejection.md Round 2 参照) → A1 (intersection を伴う Cut) を別 Issue #39 へ分離。`reconstruct_intersection_curve` / `attach_pcurves_for_trimmed_faces` / `ArcProvenance` 等の provenance チェーンは #38 plan から全削除し Non-Goals (#39 担当) へ移動
- R03 (high): **棄却** → 同上、A1 (box 上面の inner_loop = multi-loop face = annulus) は #39。A3 は intersection なしで multi-loop face を生成しない
- R04 (medium): **棄却** → 同上、A1 (cylinder lateral × box top 全周交線) は #39。A3 は intersection なしで arc 再構成自体不要

スコープ縮小判断: ユーザー承認済 (案 B 採用)。#38 = sphere naming + convex 撤廃/analytic Circle accept + partition Option<PlaneData> 化 + cyl×sph gate + A3 Acceptance。A1 は #39、A2 は別 Issue。

## Round 3 (wrapper exit 3 → Claude 裁量 fallback、critical=0 確認済)

- R01 (high): 採用 → STEP 3 を改訂し「`.unwrap()` および `.as_ref().unwrap()` の残置は禁止」と明記。`PlaneData` 参照は `let Some(plane) = ... else { continue; }` または `if let Some(plane) = ...` の Some-guard で囲み、plane 投影が必要な処理は Some 前提 helper (`coplanar_overlap_2d` 等) に閉じ込める。`partition.rs` 内の plane 関連経路で unwrap がゼロであることを完了条件に追加
- R02 (medium): 採用 → A3 / T22(A3) / T32 / STEP 5 の sphere `radius` を `2` から **`3` (box 完全内包、max coord 3 ≤ box half-extent 5)** に戻し、Issue 本文の Acceptance 条件に合わせる
- R03 (medium): 採用 → STEP 2 と §シグネチャ と T16 を改訂し、`validate_planar_face_outer_loop_basic` の検証内容を強化:
  - polygon: (a) 頂点数 ≥ 3、(b) 隣接頂点距離 > `LENGTH_TOLERANCE` (ゼロ長辺禁止)、(c) signed area の絶対値 > `LENGTH_TOLERANCE.powi(2)` (面積ゼロ禁止)
  - analytic Circle: `radius > LENGTH_TOLERANCE` を必須
  - 退化時は `Err(InvalidBooleanInput { reason: ".." })`
  - T16 を新設し (a)-(d) 4 ケース (3 点 collinear、面積ゼロ、ゼロ長辺、`radius = LENGTH_TOLERANCE / 2` Circle) を assert
- R04 (medium): 採用 → STEP 4 と §シグネチャ と T15 を改訂し、cyl×sph gate を **`validate_boolean_input` 入口での入力全体 reject から `partition_faces` 内部の face pair ループでの skip + warn ログに変更**。混在入力 (cylinder と sphere が両方ある solid) 全体は通し、cyl×sph の face pair (target=Cylinder×tool=Sphere またはその逆) のみを `intersect_surfaces` 呼び出しから除外。これにより valid な boolean (例: 将来の cylinder lateral × box top など) は通せる

