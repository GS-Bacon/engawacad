# ADR-009: Boolean 交線円を周期エッジで保持し離散化をテッセレーション層へ遅延する

## Status

Accepted

## Context

現在の Boolean 演算 (`crates/mycad-kernel/src/booleans/partition.rs`) は、曲面同士の交線円を実装上の都合で**先に弦分割してから** B-rep に焼き込んでいる:

- `ANGULAR_SEGMENTS_DEFAULT = 64` (`partition.rs:11`) を用いた 64 弦分割が partition.rs 内 8 箇所で参照され、`circle_curve_for_edge` が各弦に正確な円弧 `t_range` を再構成する形で B-rep に格納される。
- 結果として **1 円 = 64 円弧エッジ + 64 頂点** がトポロジーに恒久的に書き込まれる。幾何としての円情報 (`Curve::Circle { center, normal, radius }`) は各弦エッジ内に保持されるが、円であるという事実はトポロジー上の「64 個の弧」という外形にしか現れない。
- 一方テッセレーション側 (`tessellation/mod.rs:610-680`) は `n_u = arcs_per_rev` 等のヒューリスティクスで辻褄を合わせ、境界一致は **「両側の面が独立に同じサンプル列 (k*2π/64) を再導出する」規約**に依存している。両側が偶然同じインデックス・同じ `t` ステップを使う限り頂点座標が一致し、`assemble` 時の vertex マージで watertight になる。

この構造には以下の問題がある:

1. **#129 / #130 / #131 の連鎖バグ**は症状であり、真因は本構造である。各 fix は「両側のサンプリングを揃える」辻褄合わせとして実装され、`tessellation/mod.rs:649-680` の `adj_is_sphere` 分岐は両腕とも `arcs_per_rev` を返す**死に分岐**として痕跡が残っている (#131 試行錯誤跡)。
2. **ユーザー解像度指定が効かない**: `TessellationOptions.angular_segments` は Boolean 結果の側面・キャップで事実上無視される (64 固定がトポロジーに焼かれているため)。
3. **出力戦略との衝突**: メモリ memo `[[project_output_strategy]]`「ビューアは解像度非依存・厳密 B-rep が真実」に反する。解像度がトポロジーに固定化され、ビューアから解像度を変えても B-rep 由来の側面構造が動かせない。
4. **「両側が同じサンプルを再導出する」規約の脆弱性**: テッセレーション側で参照する `Curve::evaluate` の引数 `t` を片方の面がわずかにずらしただけで境界が破綻するが、規約はコードのコメントにしか書かれず構造的強制がない。

## Decision

**案 A を採用する**: 曲面同士の Boolean 交線円を **1 円 = 1 周期エッジ**（または閉曲線が他のエッジで分断される場合は分断点で分けた**少数の弧**）として B-rep に保持し、弦への離散化はテッセレーション層に遅延する。

具体的に:

- 交線が完全な円である場合、`Edge` 1 本に `Curve::Circle` を載せ、`t_range = [0, 2π]` の周期エッジとして保持する。両端頂点は同一点を指す seam vertex として扱う。
- 交線円が他の Boolean 交線 (例: 別曲面との交わり) によって複数弧に分断される場合は、分断点を頂点として弧の数だけ Edge を生成する。各 Edge は `Curve::Circle` + 部分 `t_range` を保持する (現状の `circle_curve_for_edge` と同等のデータ表現)。
- B-rep 上は **「円弧の数 = 幾何の分断点の数」**に一致させる。離散化都合の人工分割 (64 弦) は B-rep に現れない。
- テッセレーションは各 Circle エッジを `TessellationOptions.angular_segments` に基づいて弦分割し、両側の面は **共通のエッジサンプル列**を参照する (「両側が独立に同じ式を計算する」規約から「両側がエッジを参照する」構造へ移行)。

## Rationale

### 案 A の利点

- **解像度がトポロジーに固定化されない**: `angular_segments` がユーザー指定どおり効く。出力戦略と整合。
- **Single source of truth**: 境界サンプル列が Edge から導出され、両側で一致する構造的保証が得られる (規約依存の解消)。
- **連鎖バグの根治**: 「両側の式の差」を検出できないテッセレーション側の辻褄合わせコード (`adj_is_sphere` 死に分岐等) を撤去できる。
- **Curve 型の拡張は不要**: 既存の `Curve::Circle { center, normal, radius }` で十分。新 variant を追加せずに済む (ADR-004 の `Surface` / `Curve` enum 拡張原則に従う)。
- **決定性 (ADR-005)**: B-rep が小さくなるため決定性検証コストが下がる (頂点数・エッジ数が幾何の分断点数と一致するため `assemble` 時の vertex マージが減る)。

### 案 B (棄却) — 64 分割焼き込みを正式仕様化

案 B はテッセレーション層に「必ずエッジトポロジーからサンプルを導出する」規約を追加することで境界一致の構造保証を得る。しかし:

- **解像度がトポロジーに固定**される問題が残る (出力戦略違反)。
- B-rep 上に幾何的に意味のない 64 頂点・64 エッジが永久に残り、`.mycad` 永続化サイズ・後段アルゴリズム (Phase 7 以降のスケッチ・フィレット) の入力サイズが膨張する。
- 64 という数の妥当性に物理的根拠がない。将来「精度を上げるには 128 にすればよい」という方向の議論が頻発する温床になる。

### 案 A の難所と対応

- **周期エッジ (両端頂点が同一)** を許容する Solid の不変条件確認が必要。`crates/mycad-kernel/CLAUDE.md` の "Key Invariants" に既に「自己隣接周期面を許容: 1 本の seam edge に正逆 2 HalfEdge を載せる面 (e.g. full sphere) は正当な B-rep 表現」とあり、周期 edge の概念は既存。Boolean 交線円はこの拡張で扱える。
- **既存 `assemble` の vertex マージ**は座標一致比較に依存しているが、Edge 駆動のサンプリングに切り替えると同一エッジから派生したサンプルは構造的に同じ Point を返すため、マージは不要 (または短絡可能) になる。
- **ADR-004 の数値モデル (per-entity tolerance)** との整合: Edge の `tolerance` field 導入 (Phase 5 候補) は本決定と独立に進行できる。本決定は Tolerance 表現を要求しない。

## Implementation Outline

実装本体は別 Issue に切り出す (ADR-006「決定と実装を混在させない」)。骨子は以下:

1. **Phase 1 — 交線エッジ生成の周期化**: `partition.rs` の `IntersectionLoop` から `Edge` を生成する箇所で、64 弦分割 (`ANGULAR_SEGMENTS_DEFAULT` 参照箇所 8 件) を廃し、`Curve::Circle` を 1 本のエッジに載せる。弧分断は他のループとの交差点のみで行う。
2. **Phase 2 — テッセレーション側のエッジ駆動化**: `tessellation/mod.rs` の cap / lateral 境界サンプル列を、隣接 Edge を辿って `Curve::sample_segment` で得たサンプルを共有する構造に変更する。`arcs_per_rev` / `adj_is_sphere` 等の規約依存ヒューリスティックは撤去する。
3. **Phase 3 — テスト**: 共有境界の直接比較テスト (codex-review #129-F02 提案) を追加。`angular_segments` をユーザー指定どおりに変化させた回帰テスト (4 / 16 / 64 / 256) を追加し、トポロジーが不変・メッシュのみが変わることを検証する。
4. **後方互換**: `.mycad` 永続化は Feature history (ADR-001) のため B-rep 変更の影響を受けない。既存 Boolean 関連の golden YAML は再生成不要。

各 Phase は別 Issue として進める。**起票ポリシー** (ADR-002「着手する Phase の Issue のみ作成」と整合):

- **即時起票**: Implementation Outline の Phase 3 のうち、ADR 待ち不要な「死に分岐削除 + 共有境界の直接比較テスト追加」のみ。Phase 7 foundation 作業として消化可能。
- **延期起票 (TBD)**: Implementation Outline の Phase 1 (交線エッジ周期化) と Phase 2 (テッセレーション側のエッジ駆動化) を含む実装本体 Issue は、Phase 4 再訪タイミング (Boolean サブシステム改修を予定する時点) で起票する。ADR-009 はその時点で参照され、Implementation Outline がそのまま Issue 起票時の骨子となる。

## Affects

- **ADR-001** (B-rep 採用): Edge トポロジーの粒度規約を「幾何的分断点と一致」に絞る位置づけ。
- **ADR-004** (自由曲面・数値モデル): 本 ADR は `Curve` enum 拡張を要求しない。`Tolerance` field 導入とは独立。
- **ADR-005** (決定性): B-rep の頂点・エッジ数が幾何由来となり、決定性検証の負担が軽くなる。
- **`[[project_output_strategy]]`** (memo): 「ビューアは解像度非依存・厳密 B-rep が真実」を構造的に保証する。

## 関連

- #138 (本 ADR の起票元)
- #129 / #130 / #131 (連鎖バグの症状)
- Codex review #129-F02 (共有境界の直接比較テスト提案)
