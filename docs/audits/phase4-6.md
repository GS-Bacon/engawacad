# Phase 4-6 全体監査レポート — Fable 5 積年ドリフト検出

- **監査 Issue**: [#134](https://github.com/GS-Bacon/engawacad/issues/134)
- **実施日**: 2026-06-11
- **監査者**: Claude Fable 5(単一セッション、レビュー方針 5 ルール遵守)
- **スコープ**: Phase 4 着手(2026-05-27)〜 Phase 6 完了(2026-06-07)の全コミット(約 105 件)、`crates/engawa-kernel/src/`、ADR-002/004/005/008、`features/<n>-*/` 痕跡

---

## 全体所感

Phase 4-6 のコードベースは、決定性については模範的に守られている(`assign_intersection_edge_selectors` の BTreeMap + ソート設計、各所の 100-run 決定性テストなど、HashMap を使いつつ順序依存を慎重に排除している)。一方で、ドリフトは「機能の中心」ではなく「機能と機能の継ぎ目」に集中している: rotation は kernel 実装(#77)と build 配線の間に落ち、テッセレーションと boolean は解像度整合を暗黙の規約(arcs_per_rev 一致)で支え合い、UV 周期処理は unwrap + ad-hoc シフトの寄せ集めで周期等価性の保証がない。#129→#130→#131 の 3 連続バグはこの「整合 by 規約」構造の症状であり、個別修正では再発する。また /3ai ワークフローの STEP 8 クラッシュ経路(#121/#126/#127/#128)が `todo!()` スタブテストや probe ファイルを系統的に残しており、CI green が実態より広いカバレッジを示唆する状態になっている。

なお、事前調査の既知懸念 4 件のうち「surface_intersect.rs:402-404 の r=2 ハードコード assert」は `#[cfg(test)]` 内の正当なテストアサーションであり、誤検知として取り下げた(プロダクションコードに該当 assert は存在しない)。

---

## 監査チェックリスト結果(9 観点)

| # | 観点 | 結果 |
|---|---|---|
| 1 | ADR と実装の乖離 | **乖離 2 件**: ADR-004「カーネルは rad のみ」に対し `geometry/transform.rs:26` `euler_to_matrix` が度数を受領(→ Finding 1 に集約)。ADR-002 のマイルストーン運用は Phase 4-6 とも ROADMAP ✅・milestone close 済みで整合。ADR-005 の決定性制約・ADR-008 の `face_ids` 長さ不変量は遵守 |
| 2 | 数値ロバストネス | 既知懸念の r=2 assert は誤検知(テストコード)。`clamp` 系は Finding 3 に集約。**Top 5 外の所見**: 公差の意味論違反が散在 — `partition.rs:1874-1886` ローカル `angle_near` が sinθ(無次元)を `LENGTH_TOLERANCE` と比較、`classify.rs:71` が cosθ を `len_eps` と比較、`partition.rs:1780,1831` の `LENGTH_TOLERANCE * 10.0` ad-hoc 倍率、`partition.rs:1952,2020,2158` の `LT²` を長さ次元の分母判定に使用。ADR-005 §10「maker ごとの別閾値を禁止」の精神に反するが、数値は現状すべて 1e-9 系で実害未確認のため次点(Tolerance newtype への段階移行 #34 系の作業で吸収を推奨) |
| 3 | 公開 API 一貫性 | `KernelError` は thiserror で良好。**Top 5 外の所見**: `partition_faces`(partition.rs:131)と `classify_fragments`(classify.rs:13)が `pub` のまま `Result<_, String>` を返し、`booleans/mod.rs:26,35` で全部 `KernelError::BooleanInternal` に丸めている — 「未対応ケース(ユーザー入力起因、想定内)」と「内部不変量違反(バグ)」が型レベルで区別不能。`Curve2D`/`Pcurve` の `Deserialize` 欠落、`Surface`/`Curve`/`TriangleMesh`/`tessellate_solid` の crate root 再エクスポート欠落も確認(低) |
| 4 | module 構造 | 概ね良好。`tessellation/mod.rs` 2,869 行(うちテスト ~1,700 行)は許容範囲だが、トリム系 3 関数の分離余地あり(Finding 4 の対応時に同時整理を推奨)。`brep/topology.rs` への `IdGenerator` 同居は軽微 |
| 5 | 重複コード | tessellation に同一目的の境界整合ロジックが 3 系統(UV grid の n_u ヒューリスティクス / trimmed UV face の collect_loop_points 共有 / trimmed sphere の「同 n_u で再サンプリング」)— Finding 4 に集約。`tessellate_face_earcut`(mod.rs:313-321, 330-337)が inner loop を 2 回サンプリングし 2 回目を `unwrap_or_default()` で握り潰すのも同根 |
| 6 | テスト隙間 | 過去バグ #50/#56/#120/#129/#130/#131 の regression テストは全て存在(箇所はサブ調査で確認済み)。決定性テストも transform/boolean/tessellation で網羅的。**残骸が問題**(→ Finding 5)。未網羅: rotation×boolean 複合(rotation 配線後に必須、Finding 1/3 参照)、シーム横断穴・背面穴の trimmed tessellation(Finding 2 参照) |
| 7 | パフォーマンス hot path | `tessellation/mod.rs:718-730` `adjacent_face_idx` が half_edge 全走査×face 全走査(O(H·F·L))だが、呼び出しは arcs_per_rev>1 の面に限られ現状実害なし。`partition.rs` の face×face 総当たりは MVP として妥当。指摘なし(Finding 4 の設計見直しで自然解消する位置づけ) |
| 8 | 依存の漂流 | **漂流なし**。全クレートが `[workspace.dependencies]` + `{ workspace = true }` を遵守 |
| 9 | /3ai workflow 由来のドリフト | codex-review.yaml 指摘 4 件中 3 件対応済み、#129-F02(境界直接比較テスト)のみ未対応(→ Finding 4 に含める)。`TODO/FIXME/HACK` コメントはゼロ。**STEP 8 クラッシュ経路の残骸が系統的**(→ Finding 5): `todo!()` スタブの ignore テスト 4 本、`test_bool_probe.rs`、untracked スタブ 12 本 |

---

## Findings(優先度順 Top 5)

### Finding 1(高): `Component.transform.rotation` が build 層で無言で無視される

- **箇所**: `crates/engawa-build/src/lib.rs:374-382`(`build_component_tree`)、関連 `crates/engawa-kernel/src/geometry/transform.rs:26`
- **症状**: `.engawa` の `Component.transform.rotation` は format 層で受理され(`component.rs:15`、docs/file-format.md にも記載)、kernel には `Solid::rotate`(#77、topology.rs:476)が実装済みなのに、build 層は position のみ合成し rotation を黙って捨てる。コメントは「rotation ignored until #77」のままだが #77 は 2026-06-06 に closed。非ゼロ rotation を拒否するバリデーションも存在しない(`is_default_transform` は serde skip 用のみ)。
- **経緯**: #77 本文が「build 層への組み込みは Phase 6 以降に委ねる」と明記して kernel 実装のみで close。しかし後続の配線 Issue は一度も起票されず、Phase 6 も完了。Phase 完了優先で継ぎ目が落ちた典型例。
- **影響範囲**: rotation を書いたユーザーは**エラーなしで間違ったジオメトリ**を得る(assembly の部品配置、Phase 7 以降のスケッチ平面にも波及)。付随して、ADR-004「deg↔rad 変換は format/build 層の責務、カーネルは rad のみ」に対し `euler_to_matrix(rx_deg, ...)` が**カーネル内で度数を受領**しており、#77 の Issue 文面自体が ADR と矛盾したまま実装された。
- **推奨アクション**: 第一手(small)として build 層で非ゼロ rotation を `KernelError::UnsupportedFeature` 等で明示拒否。本修正(medium)で `euler_to_matrix` + `Solid::rotate` を `build_component_tree` に配線し、deg→rad 境界を build 層に移動(ADR-004 整合)。rotation×boolean 複合の決定性テストを追加。
- **自己反論**: 「#77 が明示的に Phase 6 以降へ委ねたのだから意図的では?」— 委ねた先の Issue が存在せず、ROADMAP にも現れないため「意図的な延期」ではなく「追跡漏れ」。silent failure である点も正当化不能。**維持**。
- **fix 規模**: small(拒否)→ medium(配線) / **別 Issue 化必須**

### Finding 2(高): trimmed UV face の内側ループ u シフトが 2π の整数倍でなく、穴の周方向位置によって三角形分割が破綻し得る

- **箇所**: `crates/engawa-kernel/src/tessellation/mod.rs:474-489`(`tessellate_trimmed_uv_face`)
- **症状**: 内側ループ(穴)の UV 化後、`shift = outer_u(外周ループ先頭点) - avg_inner_u` を計算し `|shift| > π` のとき**そのままの量**を全 u に加算する。シフト量が 2π の整数倍に丸められないため、(a) シフト適用時は穴が外周先頭点の u 位置へ「平行移動」して earcut の接続トポロジーが歪む、(b) 非適用時(|shift| ≤ π)でも穴の unwrap 後 u が外周ループの u スパン外に落ちると earcut が穴を無視し、切断穴が塞がったメッシュになる。基準が「外周ループの先頭点」という任意の点である点も不安定(Issue #134 既知懸念 2 と同一)。
- **影響範囲**: Boolean Cut で円筒側面に開けた穴の周方向位置はユーザー入力(tool 位置)で決まるため、**現行機能で到達可能**。#130 のテスト群(t01-t04)は特定配置のみ検証しており、シーム近傍・背面側(u≈±π)の穴は未網羅。症状は naked edge / 自己交差メッシュとして無言で現れる。
- **推奨アクション**: シフトを `(target - avg_inner_u を 2π で割った最近接整数) * 2π` の周期保存シフトに変更し、基準を外周ループ u スパンの中央値にする。穴位置を周方向に掃引するパラメタライズドテスト(特に u≈π、シーム横断)を追加。
- **自己反論**: 「テストが通っているので実用上は十分では?」— 通っているのは穴位置固定のケースのみで、入力空間の大半が未検証。ヒューリスティクスが幾何学的に正しくない(非周期シフト)ことはコードから自明であり、偶然動く範囲に依存している。**維持**。
- **fix 規模**: small〜medium / **別 Issue 化必須**

### Finding 3(中): trimmed sphere tessellation がグローバル Z 軸を決め打ちし、回転した形状で無言に破綻する地雷

- **箇所**: `crates/engawa-kernel/src/tessellation/mod.rs:988-999`(`tessellate_sphere_face_trimmed`)
- **症状**: トリム円の緯度を `circ_center.coords.z`(グローバル Z 成分)から計算し、トリム方向判定も `center_z < center.coords.z`(Z 比較)。エッジが持つ `circ_normal` は読み捨てている(`let _ = (circ_radius, circ_normal)` mod.rs:1091)。さらに `rel_z.clamp(-1.0, 1.0)`(mod.rs:992)が球外円を黙って吸収し、接円(tangent)の退化は `push_triangle` の AREA_EPS 除去に押し付けられる多段の握り潰し構造。境界リングは「隣接面と同じ n_u で u=0 から一様再サンプリングすれば一致するはず」という規約依存(mod.rs:1001-1028)。
- **影響範囲**: 現状は surface_intersect の MVP 制約(plane×sphere は法線 ±Z のみ、cyl×sphere は軸 ±Z のみ)が非 Z 切断を上流で拒否しているため**未到達**。しかし Finding 1 の rotation 配線、または交線サポート拡張のどちらかが入った瞬間に、回転済み boolean 結果の球面が**エラーなしで誤ったメッシュ**になる。Finding 1 の修正と連動して必ず踏む地雷。
- **推奨アクション**: `circ_normal` を信頼して球ローカル軸を導出し、緯度・トリム方向・極をその軸基準で計算する。クランプ到達(=円が球面から外れた)を debug_assert または KernelError 化し、接円ケースの明示テストを追加。
- **自己反論**: 「上流 MVP が非 Z を拒否しているので現時点では正しい」— その通りで、だからこそ優先度は高でなく中。ただし制約の存在がこのコードにコメントされておらず、上流制約の緩和時にここが壊れることを知る手掛かりがない。**地雷として維持**。
- **fix 規模**: medium / **別 Issue 化必須**

### Finding 4(中): boolean 交線の 64 分割ハードコードとテッセレーション解像度の「規約による整合」— #129/#130/#131 連鎖の真因

- **箇所**: `crates/engawa-kernel/src/booleans/partition.rs:11`(`ANGULAR_SEGMENTS_DEFAULT = 64`)、同 :458,:593,:952,:1920-1923、`tessellation/mod.rs:602-663`(n_u ヒューリスティクス)、:713-730(`adjacent_face_idx`)
- **症状**: boolean は交線円を一律 64 弦に離散化し、結果の B-rep に**1 円 = 64 円弧エッジ + 64 頂点**のトポロジーを焼き込む(`circle_curve_for_edge` が各弦に正確な円弧 t_range を再構成するため幾何は厳密だが、トポロジーが恒久的に膨張する)。テッセレーション側はこれと辻褄を合わせるため `n_u = arcs_per_rev` 等のヒューリスティクスを持ち、`adj_is_sphere` 分岐(mod.rs:654-660)は **if/else 両腕が同値の死んだ分岐**(#131 の試行錯誤痕跡)。`TessellationOptions.angular_segments` は boolean 結果の側面では事実上無視され(64 固定)、ユーザー解像度指定が効かない。境界の一致は「両側が独立に同じサンプル列を再導出する」ことに依存し、#129(cap 境界不整合)→ #130(トリム面未対応)→ #131(隣接面 n_u 不一致)は全てこの構造の症状。codex-review #129-F02 が提案した「共有境界の直接比較テスト」も未対応のまま。
- **影響範囲**: 新しい曲面型・トリムケース・boolean ケースを足すたびに「もう一方の側のサンプリング規約」を暗記して再実装する必要があり、漏れると naked edge。プロジェクトの「ビューアは解像度非依存・厳密 B-rep が真実」という出力戦略とも将来衝突する(解像度がトポロジーに固定化されるため)。
- **推奨アクション**: ADR を 1 本起こし、(a) 交線エッジを「1 円 = 1 周期エッジ(または少数の弧)」として保持し離散化をテッセレーション層に遅延する方向か、(b) 64 分割焼き込みを正式仕様としてテッセレーションが必ずエッジトポロジーからサンプルを導出する(境界サンプリングの single source of truth)方向かを決定する。死んだ if/else の削除と #129-F02 のテスト追加は即時可能。
- **自己反論**: 「MVP として PSLG ベースの弦分割は合理的で、テストも全て green では?」— 合理的なのは事実だが、この決定はどの ADR にも記録されておらず、2 日間で 3 連続バグ修正(ea34ddb→2b43c1b→d0c0598)を要した時点で「個別 PR では見えない構造問題」の基準を満たす。リファクタ単独でなく「拡張時の地雷」起点。**維持**。
- **fix 規模**: large(ADR + 段階実装。死に分岐削除と直接比較テストのみなら small)/ **別 Issue 化必須**(ADR 決定と実装は分離すること)

### Finding 5(中): /3ai STEP 8 クラッシュ経路が残した「嘘をつくテスト資産」群

- **箇所**: `crates/engawa-kernel/tests/extrude_negative_direction_acceptance.rs`(全 4 テストが `#[ignore = "STEP 6 で実装後に解除"]` + `todo!()`)、`tests/test_bool_probe.rs`(空 probe)、untracked の `tests/debug_trim*.rs` / `diag_*.rs` 12 本(全て「// temporary diagnostic file removed」スタブ)
- **症状**: #110(負方向押し出し)は kernel 実装済み・closed で、実際の regression テストは `src/primitives/extrusion.rs:783-825` にインラインで存在する。一方 tests/ 配下の受け入れテストファイルは**同名テスト**(`t01_kernel_neg_depth_determinism` 等)が `todo!()` のまま ignore されており、実行すると 4 本全て "not yet implemented" で panic する(本監査で実行確認済み)。ファイル名・テスト名だけ見ると受け入れテストが存在するように見え、ignore 理由文「STEP 6 で実装後に解除」は永遠に来ない約束になっている。#121/#126/#127/#128 が示すとおり、/3ai STEP 8 の pre-check クラッシュ時に作業物が中途半端に残る経路が系統化している。
- **影響範囲**: 将来の読者(人間・AI とも)がテスト名で網羅性を誤認する。とくに /3ai の GLM ワーカーは「既存テストがある」と判断して新規テストを省略し得る。CI は green のため腐敗が検出されない。
- **推奨アクション**: (1) `extrude_negative_direction_acceptance.rs` はインライン版と重複のため削除(または統合テストとして実装して ignore 解除)、(2) `test_bool_probe.rs` と untracked スタブ 12 本を削除、(3) /3ai STEP 8 の異常終了時に untracked テストファイルを警告列挙するチェックを skill 側に追加。
- **自己反論**: 「実カバレッジはインラインに存在するので実害ゼロでは?」— 検出力の実害はないが、テスト資産の信頼性(名前と実体の一致)は /3ai 自動化の前提インフラであり、誤認による将来のテスト省略は実害になる。掃除コストは small。**維持**。
- **fix 規模**: small / **別 Issue 化必須**

---

## 起票ドラフト

> 起票セッション(Opus 4.7 または人手)が `gh issue create` で起票すること。Fable 5 は起票しない。
> 起票後、本ファイル末尾の「起票結果」に Issue 番号を追記してコミットし、#134 を close する。

### ドラフト 1

- **title**: `fix(build): Component.transform.rotation を build 層に配線する(現状エラーなしで無視される)`
- **labels**: `bug`, `kernel`, `batch:kernel`
- **想定 milestone**: Phase 7(差し込み bug として)
- **関連 ADR**: ADR-004(deg↔rad 境界)、ADR-007(回転数値モデル)
- **body**:
  ```
  ## 症状
  `.engawa` の `Component.transform.rotation` は format で受理されるが、
  `engawa-build/src/lib.rs:374-382` build_component_tree が position のみ合成し
  rotation を黙って捨てる。バリデーションも無いため、非ゼロ rotation を書いた
  ユーザーはエラーなしで間違ったジオメトリを得る。

  ## 経緯
  #77 が kernel 側 Solid::rotate を実装し「build 配線は Phase 6 以降」と明記して
  close したが、後続 Issue が起票されず Phase 6 も完了してしまった。
  コメント「rotation ignored until #77」は #77 closed 後も残置。

  ## 作業内容
  1. (即時) build 層で非ゼロ rotation を明示拒否するガード + テスト
  2. (本修正) euler_to_matrix + Solid::rotate を build_component_tree に配線。
     accumulated transform を offset+matrix の合成に拡張
  3. ADR-004 整合: deg→rad 変換を build 層に移し、kernel euler_to_matrix の
     度数入力を rad 入力へ変更(geometry/transform.rs:26)
  4. rotation×boolean 複合の決定性・manifold テスト追加
     (注: 現状 trimmed sphere tessellation が Z 決め打ちのため、
      rotation×boolean cut 球は別 Issue <ドラフト3> の解決が前提)

  ## 監査出典
  docs/audits/phase4-6.md Finding 1(高)
  ```

### ドラフト 2

- **title**: `fix(tessellation): trimmed UV face の内側ループ u シフトを 2π 周期保存に正規化する`
- **labels**: `bug`, `kernel`, `batch:kernel`
- **想定 milestone**: Phase 7(差し込み bug として)
- **関連 ADR**: ADR-004(数値モデル)
- **body**:
  ```
  ## 症状
  tessellation/mod.rs:474-489 で内側ループの u を
  shift = outer_u(外周先頭点) - avg_inner_u だけ平行移動するが、
  2π の整数倍に丸めていないため:
  - |shift| > π: 穴が UV 上で別の周方向位置へ移動し earcut の接続が歪む
  - |shift| ≤ π でも穴が外周 u スパン外なら earcut が穴を無視(穴が塞がる)

  ## 作業内容
  1. シフトを round(Δ/2π)*2π の周期保存シフトへ変更
  2. 基準を外周ループ先頭点でなく外周 u スパン中央値へ
  3. 穴位置を周方向に掃引するテスト(u≈0/π/2/π/-π/2、シーム横断)+
     naked_edge=0 / 三角形数の検証

  ## 監査出典
  docs/audits/phase4-6.md Finding 2(高)。Issue #134 既知懸念 2 の確定版
  ```

### ドラフト 3

- **title**: `fix(tessellation): trimmed sphere tessellation のグローバル Z 決め打ちを circ_normal 基準に一般化する`
- **labels**: `kernel`, `type: foundation`, `batch:kernel`
- **想定 milestone**: Phase 7(rotation 配線の前提整備)
- **関連 ADR**: ADR-004
- **body**:
  ```
  ## 症状(現状未到達の地雷)
  tessellation/mod.rs:988-999 がトリム円の緯度を circ_center.z から計算し、
  トリム方向も Z 比較で決定。エッジの circ_normal は読み捨て(mod.rs:1091)。
  現状は surface_intersect の MVP 制約(±Z のみ)が上流で守っているが、
  rotation の build 配線(ドラフト1)または交線拡張で即座に無言破綻する。

  ## 作業内容
  1. circ_normal から球ローカル軸を導出し、緯度・トリム方向・極を軸基準化
  2. rel_z.clamp(-1,1) 到達(円が球面外)を debug_assert または KernelError 化
  3. 接円(tangent circle)の明示テスト、回転済み球ディンプルのテスト
  4. 境界リング再サンプリング(mod.rs:1001-1028)の隣接面整合を直接比較で検証

  ## 監査出典
  docs/audits/phase4-6.md Finding 3(中)。Issue #134 既知懸念 3 の確定版
  ```

### ドラフト 4

- **title**: `docs(adr): boolean 交線離散化とテッセレーション解像度の整合戦略を ADR 化する(#129/#130/#131 連鎖の真因)`
- **labels**: `kernel`, `type: foundation`, `docs`
- **想定 milestone**: Phase 7 以降(着手前に ADR 決定)
- **関連 ADR**: ADR-004(新 ADR の親)、ADR-005
- **body**:
  ```
  ## 問題
  boolean が交線円を ANGULAR_SEGMENTS_DEFAULT=64(partition.rs:11)で弦分割し、
  1 円 = 64 円弧エッジのトポロジーを B-rep に焼き込む。テッセレーションは
  n_u=arcs_per_rev 等のヒューリスティクス(mod.rs:602-663)で辻褄を合わせ、
  境界一致は「両側が独立に同じサンプル列を再導出する」規約に依存。
  #129→#130→#131 はこの構造の症状で、個別修正では再発する。
  TessellationOptions.angular_segments も boolean 結果側面では事実上無効。

  ## 決定すべきこと(ADR、実装と分離)
  - 案A: 交線を 1 円=1 周期エッジで保持し、離散化をテッセレーション層へ遅延
  - 案B: 64 分割焼き込みを正式仕様化し、テッセレーションは必ずエッジ
    トポロジーからサンプルを導出(境界サンプリング SSOT)

  ## 即時実施可能(ADR 待ち不要、小)
  - mod.rs:654-660 の if/else 同値の死に分岐を削除
  - codex-review #129-F02 の「共有境界の直接比較テスト」を追加

  ## 監査出典
  docs/audits/phase4-6.md Finding 4(中)
  ```

### ドラフト 5

- **title**: `chore(test): /3ai 残骸テスト資産の整理 — todo!() スタブ受け入れテスト・probe・診断スタブ`
- **labels**: `kernel`, `type: foundation`, `batch:kernel`
- **想定 milestone**: Phase 7(差し込み)
- **関連 ADR**: ADR-002(/3ai 運用)
- **body**:
  ```
  ## 症状
  - tests/extrude_negative_direction_acceptance.rs: 全 4 テストが
    #[ignore = "STEP 6 で実装後に解除"] + todo!() のまま。#110 は closed で
    実テストは src/primitives/extrusion.rs:783-825 にインライン実在 →
    同名スタブが網羅性を誤認させる
  - tests/test_bool_probe.rs: 空 probe(コメントに「削除可」と自記)
  - untracked の tests/debug_trim*.rs / diag_*.rs 12 本(全てスタブ化済み)

  ## 作業内容
  1. extrude_negative_direction_acceptance.rs を削除(または統合テストとして
     実装し ignore 解除。インライン版との重複名は解消)
  2. test_bool_probe.rs と untracked スタブ 12 本を削除
  3. /3ai skill: STEP 8 異常終了時に untracked テストファイルを警告列挙する
     ガードを追加(#121/#126/#127/#128 の再発防止)

  ## 監査出典
  docs/audits/phase4-6.md Finding 5(中)
  ```

---

## 取り下げた指摘(自己反論で消えたもの・代表例)

- **surface_intersect.rs:402-404 の r=2 ハードコード assert(既知懸念 1)**: `#[cfg(test)]` 内で半径 2 の円筒を作って交線半径 2 を検証する正当なテスト。プロダクションコードに該当 assert なし。誤検知。
- **assemble.rs:157 の HashMap イテレーション(ADR-005 §9 違反疑い)**: 直後の `assign_intersection_edge_selectors` が BTreeMap + `edges.sort()` で順序を正規化しており、出力は順序不変。決定性テスト(100-run 多数)も green。意図的な設計として正当化可能。
- **push_triangle の AREA_EPS 除去(既知懸念 4)**: #29 で指摘された `.sqrt()` 適用位置の誤りは修正済み(`cross_norm_sq < AREA_EPS²` の二乗比較で一貫)。face_ids と indices の同期も保たれている。ADR-004 に private heuristic として記録済みであり、単独では finding にしない(Finding 3 の多段握り潰し構造の一部としてのみ言及)。

## 起票結果(2026-06-11、Opus 4.7 セッションで起票)

- ドラフト 1: #135 — `fix(build): Component.transform.rotation を build 層に配線する`
- ドラフト 2: #136 — `fix(tessellation): trimmed UV face の内側ループ u シフトを 2π 周期保存に正規化する`
- ドラフト 3: #137 — `fix(tessellation): trimmed sphere tessellation のグローバル Z 決め打ちを circ_normal 基準に一般化する`
- ドラフト 4: #138 — `docs(adr): boolean 交線離散化とテッセレーション解像度の整合戦略を ADR 化する`
- ドラフト 5: #139 — `chore(test): /3ai 残骸テスト資産の整理`

すべて milestone Phase 7 に紐づけ。依存関係: #135 は #137 解決を前提に
「rotation × boolean Cut 球」テストを実施する旨を本文で言及済み。
