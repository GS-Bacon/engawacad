# debug-spec for #265 STEP 7.5 Codex Round 1 修正

## Codex 指摘 (3 persona 一致)

- **architect F02 (high)** / **contrarian F02 (high)** / **migration F01 (high)**: 3 ペルソナが同一方向で指摘。

atomic skip semantics は `simulate_history` の prefix walk にのみ適用されているが、`check_refs_resolve_before` の `producer_after` 走査と `check_no_downstream_break` の downstream consumer 走査は依然として broken な future feature を「実在する producer/consumer」とみなしている。結果として:

例 1: `[CreateBox(b1), Extrude(e1, sketch=missing_sk, fuse_target=Some(b1))]` に `Cut(target=b1, tool=b2)` (b2 を事前 push) を **idx 1 で insert** (Extrude e1 の直前):
- 現状: `check_no_downstream_break` が e1 を direct consumer (fuse_target=b1) とみなし → `InsertBeforeConsumer` で falsely reject。
- 期待: e1 自身が broken (sketch=missing_sk 解決不可) なので simulate_history で skip される → e1 は real consumer ではない → Cut(target=b1) は `Ok` であるべき。

例 2: `[CreateBox(b1), CreateBox(b2), Cut(c1, target=missing, tool=missing), Cut(real, target=c1, tool=b2)]` の状態で `Cut(new, target=c1, tool=b1)` を **idx 2 で insert** (broken Cut c1 の直前):
- 現状: `check_refs_resolve_before` の `producer_after` 走査が c1 (broken Cut) を producer とみなし → `InsertBeforeProducer { ref_id: "c1", producer_at: 2 }` で falsely reject。
- 期待: c1 は broken (refs 解決不可) なので skip され、c1 という body は実在しない → `BodyNotFound { feature_id: "new", body_ref: "c1" }` であるべき。

## 修正方針 (atomic skip semantics を forward scan にも対称適用)

1. **`simulate_history` の戻り値を拡張**: 現在は `(HashMap<String, usize>, HashMap<String, usize>)` (sketches_at / live_bodies_at)。これに加え **`executed_at: HashSet<usize>`** を 3rd 要素として返す。`executed_at` は「prefix walk で `refs_resolve_in_state` が true → consume + register が走った index」の集合。CreateBox/CreateCylinder/CreateSphere/CreateSketch (ref を持たない feature) は無条件で executed_at に追加。Extrude/ExtrudeCut/Cut/Fuse/Intersect は `refs_resolve_in_state` が true な場合のみ追加。

2. **`simulate_history` のシグネチャ変更**:
   ```rust
   fn simulate_history(
       features: &[Feature],
       up_to: usize,
   ) -> (HashMap<String, usize>, HashMap<String, usize>, HashSet<usize>)
   ```
   呼び出し側は `let (sketches_at, live_bodies_at, _executed_at) = simulate_history(...);` で受ける。ただし、forward scan で使うために `executed_at` を別途算出する必要がある (forward scan は features[at..] の各 index の executed 状態を知る必要があるため、`up_to = features.len()` の full simulate が必要)。

3. **forward scan 用に full executed_at を別途算出**:
   `check_refs_resolve_before` と `check_no_downstream_break` の冒頭で:
   ```rust
   let (_, _, executed_at_full) = simulate_history(features, features.len());
   ```
   これを以下 2 箇所の判定に使う:
   
   **`check_refs_resolve_before`** の body refs `producer_after` 走査 (line 342-374) を:
   ```rust
   let producer_after = features.iter().enumerate().skip(at).find_map(|(i, feat)| {
       if feat.id() == body_ref && executed_at_full.contains(&i) {
           match feat {
               Feature::CreateBox { .. }
               | Feature::CreateCylinder { .. }
               | Feature::CreateSphere { .. }
               | Feature::Extrude { .. }
               | Feature::ExtrudeCut { .. }
               | Feature::Cut { .. }
               | Feature::Fuse { .. }
               | Feature::Intersect { .. } => Some(i),
               _ => None,
           }
       } else {
           None
       }
   });
   ```
   implicit body refs の `producer_after` 走査 (line 391-407) にも同じ `executed_at_full.contains(&i)` ガードを入れる。
   
   **`check_no_downstream_break`** の consumer 走査 (line 438-468) を:
   ```rust
   for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
       // broken な future consumer は real consumer ではないので skip
       if !executed_at_full.contains(&consumer_idx) {
           continue;
       }
       let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
       // ... 既存ロジック
   }
   ```
   さらに re_registered ループ (line 447-468) の producer 判定にも `executed_at_full.contains(&reg_idx)` ガードを追加。broken producer による re-register は無効化。

4. **CreateSketch の取り扱い**: simulate_history で CreateSketch は無条件 register (sketches_at 経由)。これは現行と同じ。executed_at には CreateSketch も含める (sketch ref を持たないため refs_resolve_in_state で `_ => true` となり pass)。

5. **CreateSketch.plane_ref=Entity が broken な場合**: 本 Issue scope 外 (#264 で plane_ref lifetime tracking を実装済、#266 で transitive scope-defer 済)。CreateSketch を executed_at に常時追加することで、broken plane_ref を持つ CreateSketch も "executed" 扱いになる。これは #264/#266 のスコープであり本 Issue では介入しない。

## 追加テスト (`tests/feature_crud_prefix_validate_acceptance.rs` への append)

- **FORWARD01 (broken future consumer の前に insert)**: 例 1 のシナリオ。`[CreateBox(b1), CreateBox(b2), Extrude(e1, sketch=missing_sk, fuse_target=Some(b1))]` に `Cut(target=b1, tool=b2)` を idx 2 で insert → `Ok(_)` を assert。
- **FORWARD02 (broken future producer の前に insert)**: 例 2 のシナリオ。`[CreateBox(b1), CreateBox(b2), Cut(c1, target=missing, tool=missing)]` に `Cut(new, target=c1, tool=b1)` を idx 2 で insert → `Err(BodyNotFound { feature_id: "new", body_ref: "c1" })` を assert (InsertBeforeProducer ではない)。
- **FORWARD03 (broken future re-register が無効化される)**: `[CreateBox(b1), CreateBox(b2)]` に対し `Cut(consume_b1, target=b1, tool=b2)` を idx 0 で挿入する試み。downstream に broken な re-register 候補 `Cut(b1, target=missing, tool=missing)` を仮置きしてみる → re_registered が false に保たれて `InsertBeforeConsumer` か正しい挙動を assert。
- **FORWARD04 (executed_at の決定性)**: 同一 history を 2 回 simulate_history → executed_at が同一 (HashSet なので順序問わず) を間接 assert (insert の挙動が同一であることで間接的に確認)。

## 試した修正と結果

(初回 — まだ修正未実施)

## 次にやること

1. simulate_history の戻り値型を `(HashMap, HashMap, HashSet<usize>)` に拡張
2. simulate_history 内の各 arm で executed_at に index を追加 (CreateSketch/CreateBox/Cylinder/Sphere は無条件、body-producer は `refs_resolve_in_state` true 時のみ)
3. simulate_history の既存呼び出し箇所 (`check_refs_resolve_before` 内 1 箇所) を分解代入に対応
4. check_refs_resolve_before / check_no_downstream_break 冒頭で full simulate_history を呼んで executed_at_full を取得
5. body refs `producer_after` 走査 + implicit body refs `producer_after` 走査 + downstream consumer ループ + re_registered ループ に executed_at_full ガードを追加
6. 上記 FORWARD01-04 テストを `tests/feature_crud_prefix_validate_acceptance.rs` に append
7. `cargo xtask ci` で既存 17 件 + 新 4 件 + 全体 1216 件が green を確認
