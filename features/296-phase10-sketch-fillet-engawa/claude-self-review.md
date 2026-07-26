# Claude self-review (STEP 6.7) — #296 Sketch Fillet (round 2)

対象差分: `claude/add-claude-guidelines-BKKtD..HEAD` (`crates/`, `examples/`)
判定サマリ: **debug-spec A-G 全項目 fixed。新規 critical 0 / high 0 / medium 2 / low 4**。
round 1 で指摘した「テストが主張どおりの不変条件を検証していない」問題は A-G の範囲では解消済み。
残る新規指摘はいずれも「修正が届いていない周辺」で、実装ロジックのバグではない。

検証実行 (すべて green):

| コマンド | 結果 |
|---|---|
| `cargo test -p engawa-build --test sketch_fillet_acceptance -- --include-ignored` | 23 passed / 0 failed / 0 ignored |
| `cargo test -p engawa-kernel --lib sketch_fillet` | 20 passed |
| `cargo test -p engawa-format --test golden_examples` | 21 passed (`golden_sketch_fillet` 含む) |
| `cargo test -p engawa-build --test examples_smoke` | 28 passed (`sketch_fillet` 含む) |
| `cargo test --workspace` | 全 green (failure 0) |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace -- -D warnings` (= `xtask ci` と同一) | green |

---

## debug-spec 対応状況 (A-G 各項目 fixed/not-fixed)

| 項目 | 判定 | 根拠 |
|---|---|---|
| **A** ゼロ長判定の element_id 誤報告 | **fixed** | `sketch_fillet.rs:57-68` で `len_a` / `len_b` が独立した `if` になり、それぞれ `a_id` / `b_id` を返す。回帰テスト `t_deg_zero_length_input_line_b` (`:466-485`) が「elem_b のみゼロ長 → `element_id == "l2"`」を assert。既存 `t_deg_zero_length_input_line` (`:714`) の a 側も維持。両方 green |
| **B** `normalize()` の tolerance 結合コメント | **fixed** | `sketch_fillet.rs:252-256` に4行コメント。記述内容も事実と一致することを確認 (`geometry/math.rs:10,13` で `LENGTH_TOLERANCE == ANGLE_TOLERANCE == 1e-9`、ADR-004 の tolerance 表と整合)。アルゴリズム変更はしておらず debug-spec の「最小対応」指定どおり |
| **C** T08 の build が fillet を適用していない | **fixed** | `sketch_fillet_acceptance.rs:187-217` で feature 列が `[CreateSketch(cw), SketchFillet{l1,l2,r=1}, Extrude]` になり `euler_poincare() == 0` も追加。手計算 (center=(1,4), start=π, end=π/2) と実行結果が一致。**ただし assert の判別力に残存弱点あり → contrarian 参照** |
| **D** kernel `t03_normal_60deg` の tangent length assert | **fixed** | `sketch_fillet.rs:411-435` で `expected_t = 1.0/(PI/6).tan()` (= √3) を実際に使い `trimmed_a.to == (√3, 0)` / `trimmed_b.from == (√3/2, 3/2)` を assert。`let _ = expected_t;` は消滅。さらに `:448-456` で `\|center - tangent_a\| == radius` / `\|center - tangent_b\| == radius` の接線性 assert を追加。`t = radius` 誤実装なら (√3,0) と (1,0) で 0.73 ズレるので確実に落ちる。expected 値は実装から独立に構成されており循環していない |
| **E** 空テスト3件 | **fixed (削除)** | `t02/t03/t05_..._covered_at_..._level` の3関数は消滅 (acceptance 26 → 23 件)。plan の T02/T03 は kernel inline (`t02_normal_90deg` / `t03_normal_60deg`)、T05 は `engawa-format/src/feature.rs:1745-1830` の inline 3件 (roundtrip / suppressed 省略 / suppressed 直列化) でカバー済みを実測確認 |
| **F** T04 の頂点座標 assert | **fixed** | `sketch_fillet_acceptance.rs:90-122`。`z ∈ {0.0, 3.0}` の両層で「(10,0,z) 不在」「(9,0,z) 存在」「(10,1,z) 存在」を assert。fillet が build 経路で適用されなければ必ず落ちる = 判別力あり。tolerance は 1e-9 厳密だが `t = 1/tan(π/4)` の f64 誤差は 1.8e-15 なので余裕あり |
| **G** `golden_sketch_fillet()` | **fixed** | `golden_examples.rs:310-366`。`assert_golden` は `Document::from_path` → `to_yaml` の byte 比較。`examples/sketch_fillet.engawa` 側にある `offset: 0.0` が golden 側で省略されているところまで一致しており、`skip_serializing_if` の drift ガードとして機能している |
| **H** (任意) CRUD gate 経路追加 | **not-fixed (blocking ではない)** | `edit(SketchFillet 自身)` / `reorder` / `suppress` / `delete(CreateSketch)` の4経路は依然テストなし。debug-spec が任意扱いなので判定は据え置き。round 1 の medium がそのまま残存 |

**A-G は全て fixed。新規に導入されたリグレッションは検出されなかった** (下記「修正由来の回帰チェック」参照)。

---

## architect

- **[medium] finding**: `built_sketch_profiles` の**累積経路 (`engawa-build/src/lib.rs:532-536` の `Some` 分岐) が全テストで一度も通っていない**。fillet 関連のテストはすべて SketchFillet を 1 個しか持たず、かつ SketchOffset と併用しないため、`built_sketch_profiles.get(sketch)` は常に `None` を返し `entry.profile` フォールバックだけが実行される。ところが「同一 sketch の別コーナー2箇所を fillet する」(例: `l1/l2` と `l3/l4`) は Non-Goals の「fillet 済み profile への再 fillet」ではなく素直な In-Scope 用法であり、実装上も成立するはず。手計算では 1回目後の profile が `[l1', arc12, l2', l3, l4]` となり、`l3`/`l4` は idx 3,4 で隣接のまま corner `(0,5)` の共有も保存されるので 2回目も通る。しかし (a) `insert_at` が 2 回目でずれない保証、(b) CRUD gate が**元の** `CreateSketch.profile` 基準で判定する (`feature_crud.rs:256-282`) のに build は累積 profile 基準という非対称、のどちらにもリグレッションネットが無い。
  **suggestion**: acceptance に 1 件追加 — 「同一 sketch に SketchFillet 2 件 (`l1/l2` と `l3/l4`) → build 成功 + `euler_poincare() == 0` + 頂点 `(1,5,z)` / `(0,4,z)` 存在 + `(0,5,z)` 不在」。既存 `fillet_rect_features` ヘルパーの feature 列に 1 要素足すだけで書ける。

- **[low] finding**: `compute_fillet` の**エラー優先順位が rustdoc に書かれていない**。実装の判定順は `radius` → `no_shared_corner` → `len_a` → `len_b` → `corner_angle` → `radius_too_large`。したがって「`elem_a` がゼロ長で、かつその点が `b.from` から離れている」入力では `fillet_zero_length_input_line` ではなく `sketch_fillet_no_shared_corner` が返る。決定的なので実害はないが、A の修正で「どちらの要素が退化したかを正しく返す」契約を強めた以上、`# Errors` 節 (`sketch_fillet.rs:29-35`) は列挙順であって優先順位ではない旨を明示しておかないと同種の誤報告が再発しうる。
  **suggestion**: `# Errors` に「判定順は上から」の 1 行を足すだけでよい。

---

## contrarian

- **[medium] finding**: **T08 の assert は依然として「fillet を適用してもしなくても通る」**。C の修正は debug-spec の指示どおり (feature 列に `SketchFillet` を入れ、`euler_poincare()` を足した) だが、追加された assert は `built.all().len() == 1` と `euler_poincare() == 0` の 2 つだけで、**どちらも feature 列から `SketchFillet` を抜いた素の CW 矩形押し出しでも通る**。`V - E + F - 2S` は純粋な位相量なので、負 sweep Arc がタッセレーションで逆回り (π → π/2 ではなく π → π/2 + 2π) に展開されて profile が自己交差しても検出できない。さらに悪いことに、**CW / 負 sweep プロファイルに対する chain continuity テストは kernel 側にも存在しない** — kernel `t09_profile_chain_continuity` も acceptance `t09` も CCW の `rect_profile()` しか使っておらず、`cw_rect_profile()` を使うのは T08 だけ。結果として「負 sweep 経路の**幾何**が正しい」ことを保証するテストはリポジトリ全体で 0 件になっている。
  実装自体は正しいことを確認済み: `engawa-kernel/src/tessellation/sketch.rs:82-97` が `sweep = end_angle - start_angle` を符号付きのまま線形補間するので、負 sweep でも短い側を回る。手計算で arc 中点 (角度 3π/4) = `(0.293, 4.707)` を確認した。
  なお chain continuity を足すだけでは不足する点に注意 — 端点は `end_angle` から計算されるので逆回り展開でも一致してしまう。**逆回りを落とせる唯一の条件は円弧上の中間頂点の位置**。
  **suggestion**: T08 に 2 種類足す。(1) T06 と同形の `filleted.vertices.len() > baseline.vertices.len()` (fillet が適用されたこと自体の判別、1 行)。(2) 「center `(1,4)` から距離 ≈ radius の頂点はすべて `x <= 1 + tol && y >= 4 - tol` を満たす」— 正しい弧は角度 `[π/2, π]` にしか点を持たないのでこれを満たし、逆回り展開は `(2,4)` や `(1,3)` を生むので落ちる。

- **[low] finding**: `t12_crud_gate_rejects_reorder_breaking_adjacency` は**名前に反して `FeatureCrud::reorder` を呼んでいない** (`FeatureCrud::edit(&doc, "sk", new_sketch)` 経路)。中身は妥当で、profile 内の要素順を `[l1, l3, l2, l4]` に替えて `find_adjacent_pair` の adjacency 判定を実際に踏み `EditBreaksConsumer` を得ている。だが feature 列の reorder API は一度も実行されない。round 1 の表では「edit 経路」に正しく分類したが、テスト名が経路を誤って示しているため、将来の読み手が「reorder はカバー済み」と誤解する。
  **suggestion**: `t12_crud_gate_rejects_profile_element_reorder_breaking_adjacency` へのリネーム、または H の reorder テストを 1 件足す。

- **[low] finding**: SketchFillet の `suppressed: true` を build に通すテストが acceptance に **0 件** (`grep 'suppressed: true'` がヒットなし)。build 側は `lib.rs:207` の loop 先頭 guard に依存しており、dispatch arm 側は `suppressed: _` で無視する構造。SketchOffset (`sketch_offset_acceptance.rs`) も同様に未カバーなので #296 固有の退化ではないが、Feature を足すたびに同じ穴が 1 個ずつ増えている。
  **suggestion**: 本 Issue では対応不要 (前例踏襲)。記録のみ。

---

## migration

- **[medium] finding**: **未追跡ファイル `crates/engawa-format/tests/dump_fillet_temp.rs` が worktree に残っている** (`git status` で `??`、中身は空行 1 行のみ)。debug ラウンド中の一時ダンプ用と思われる。`tests/` 直下の `.rs` は中身が空でも cargo が**独立したテストターゲットとしてコンパイル**するため (`cargo test --workspace` の出力に `0 passed` のターゲットが増えている)、STEP 8 で `git add -A` 相当を行うと無意味なテストバイナリがリポジトリに入り、以後ビルド時間を食い続ける。
  **suggestion**: commit 前に削除する (`rm crates/engawa-format/tests/dump_fillet_temp.rs`)。

- **[low] finding**: `cargo clippy --workspace --all-targets -- -D warnings` は **base ブランチ時点から赤**。`crates/engawa-format/src/document.rs:500` と `:705` の `path.extension().map_or(true, |e| e != "engawa")` が rust 1.92 (clippy 0.1.92) で追加された `clippy::unnecessary_map_or` に抵触する。**#296 の差分とは完全に無関係** (`document.rs` は差分に含まれない) であり、`cargo xtask ci` が実行するのは `--all-targets` なしの `cargo clippy --workspace -- -D warnings` (`xtask/src/main.rs:809`) なので、**本 Issue の CI は green のまま**。ブロッカーではない。
  **suggestion**: `is_none_or` への置換を別 foundation Issue で起票。#296 に混ぜない。

- **[low] finding**: acceptance からスタブ 3 件を消したこと自体は正しいが、**plan のテスト ID → 実テストの対応が追えなくなった**。ファイル冒頭コメント (`sketch_fillet_acceptance.rs:2`) は plan.md を指すだけで、T02/T03/T05 が別クレートに移ったことを示す手掛かりが無い。debug-spec E は「削除して kernel 側テストへの doc コメント参照のみ残す」を選択肢として挙げていたが、参照コメントは残されていない。
  **suggestion**: 冒頭コメントに 1 行 — 「T02/T03 は `engawa-kernel/src/geometry/sketch_fillet.rs`、T05 は `engawa-format/src/feature.rs` の inline test」。

---

## 修正由来の回帰チェック (「修正それ自体が新しいバグを生んでいないか」)

いずれも問題なしを確認した:

- **A の分岐分割**: `t_deg_corner_angle_flat` は `l2` の長さが 1e-12 なので `len_b <= LENGTH_TOLERANCE` に先に当たり `fillet_zero_length_input_line` を返すが、テストが `corner_angle_degenerate` / `zero_length` の両方を許容しているため green。既存 `t_deg_zero_length_input_line` (a 側) も `element_id == "l1"` のまま変化なし。
- **C の feature 列変更で T08 の期待値が古くなっていないか**: `end < start` の assert は `apply_sketch_fillet_build(&cw, ...)` の戻り値 `out` に対するもので feature 列とは独立しており、修正前後で不変。追加した `euler_poincare() == 0` も実行 green。round 1 で予測した `center=(1,4)`, `start=π`, `end=π/2` と実挙動が一致。
- **D の expected 値の循環参照**: `expected_t` は `1.0 / (PI/6.0).tan()` と実装から独立に構成されており、`compute_fillet` の出力を再利用していない。接線性 assert も `expected_tangent_a` (独立に計算) を使用。
- **E の削除で失われたカバレッジ**: なし。T02/T03 は kernel inline、T05 は format inline に実体があり、いずれも実行 green を確認。
- **F の座標 assert の tolerance**: `t = 1/tan(π/4)` の f64 誤差は 1.8e-15、`LENGTH_TOLERANCE` 1e-9 に対して 6 桁の余裕。フレーク要因にならない。
- **G の golden**: `golden_examples.rs` 21 件すべて green。既存 golden への影響なし。
- **TS drift**: 修正コミット `67a5d40` は `xtask/src/main.rs` / `web/` を触っていないため round 1 の byte-identical 検証がそのまま有効。`cargo test -p xtask` 28 passed に golden TS テストが含まれており green。
- **累積合成順序 (SketchOffset → SketchFillet)**: round 1 で検証済み。round 2 の修正は `engawa-build/src/lib.rs` に触れていない。
- **workspace 全体**: `cargo test --workspace` に failure 0。#296 の変更による既存テストへの巻き添えなし。
