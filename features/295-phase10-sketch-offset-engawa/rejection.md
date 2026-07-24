<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round 3
- SC01 (medium, scope): 棄却 — 誤検出。`## In-Scope / Out-of-Scope` は plan.md line 10 に存在する (grep で確認)。GLM SCOPE ペルソナが自律判断ログ節の存在で file offset をズラした可能性。plan 修正なし
- IN01 (medium, invariant): 棄却 — 指摘自体が "本設計は runtime check で妥当と判断する" と結んでおり future work 提案 (Non-Goals 追記のみ)。runtime `!distance.is_finite()` で fail-fast は既に plan.md 明示、serde 側 validation は本 Issue scope 外 (Non-Goals で暗黙的にカバー)。plan 修正なし

## Round 5 (STEP 7.5 Codex round 3 pause 再開後、人間判断で決着 — #303/#295 コメント参照)

- **C-F02 / M-F01 (high): schema_version bump 要否 → 棄却**
  - 指摘: `Feature::SketchOffset` variant 追加で `CURRENT_SCHEMA_VERSION` (現在 2) を bump すべき。旧 v2 reader が unknown `type: sketch_offset` で crash する。
  - 棄却理由: `CURRENT_SCHEMA_VERSION` の唯一の変更 (1→2) は #274 (Ellipse/Conic) の `SketchElement` **untagged → tagged (`kind` 必須)** という既存表現そのものを壊す breaking change に対するもの。直後の #275 (Slot/Rectangle/Polygon) では GLM が `SketchElement` に `#[non_exhaustive]` を追加しようとしたが Codex round 4 で「scope 外まで変えている」と reject され、rollback commit `10a635c` (`chore(3ai): #275 codex-7.5 r4 rollback`) で撤回済み。撤回理由は明文化されている: 「`#275` では variant 追加のみが目的で、API 破壊変更は別 Issue で扱う」。`Feature::SketchOffset` の追加も同種の「既存 variant には無変更、新 variant のみ追加」する加算的変更であり、この確立済み前例に従えば version bump は不要。
  - 系統的な「新 variant 追加でも旧 reader が unknown discriminant で crash する」問題自体は実在する懸念だが、これは `SketchOffset` 固有ではなく Ellipse/Conic/Slot/Rectangle/Polygon 等これまでの全 variant 追加に共通する既存ギャップ。#274 の rejection.md にも "wire-format strictness は別 ADR + Issue 起票" と deferred 済み。本 Issue 単体で対応するのは scope 外と判断。
  - plan.md 修正なし。schema_version は 2 のまま。

- **C-F01 (high): public 契約 (`selection: Vec<String>`) vs build 実装 (Circle-only) の乖離 → narrow を明文化のみで解消 (adopt: doc-only)**
  - 指摘: build 経路は `apply_sketch_offset_build` で Circle 単一要素の sketch のみ受理するが、公開 `Feature::SketchOffset.selection: Vec<String>` は任意要素を選べるように見え、契約が広すぎる。
  - 調査結果: build 側は既に `t_deg_line_arc_rejected_at_build` / `t_deg_connected_lines_rejected` / `t_deg_single_arc_rejected_at_build` の 3 acceptance test で Line/Arc/多要素 profile が `UnsupportedFeature { kind: "sketch_offset_only_circle" }` で確実に reject されることを end-to-end で証明済み。selection の未知 ID 無視 (silent no-op) も plan.md 数値モデル節で既に Non-Goal として明記済み (Refactor Pass 送り)。つまり実行時の narrow 化は既に十分。
  - 採用範囲: **doc のみ**。`selection: Vec<String>` の wire 型変更 (例: 単一 `target: String` への縮小) は行わない — 4 round の設計レビューを経た意図的な将来拡張 (Phase 11+ Line/Arc build 接続) の配線であり、過剰な再スコープ (ADR-006 の粒度原則にも反する scope creep) になるため見送る。
  - 対応: `Feature::SketchOffset` の rustdoc と plan.md In-Scope 表に "build 契約: Circle 単一要素の sketch のみ" を明記 (Task #3/#4 で実施)。

- **C-F03 / M-F02 (medium): golden YAML 追加 → 採用**
  - `examples/sketch_offset.engawa` の byte-identical golden test (`golden_sketch_offset`) を追加。`selection: []` は canonical 出力 (`skip_serializing_if = "Vec::is_empty"`) で省略されるため example からも削除して正準形に統一する。

## Round 6 (rebase 後 fresh STEP 7.5 で再検出、round 4/5 の A01)

- **round 4 A01 (high): CRUD (`refs_resolve_in_state`) が `SketchOffset` の参照 sketch が単一 Circle かを検証していない → 採用**
  - `crates/engawa-build/src/feature_crud.rs` の `refs_resolve_in_state` / `check_refs_resolve_before` に「参照先 `CreateSketch.profile` が単一 `Circle`」チェックを追加、回帰テスト `t19_sketch_offset_line_profile_rejected` を追加。GLM 実装、`cargo xtask ci` green。

- **round 5 A01 (high): `simulate_history()` が `built_sketch_profiles` 相当の累積 offset 状態を持たず、複数回 inward offset の collapse (radius <= 0) を CRUD 側で検出できない → 棄却**
  - 指摘: 同一 Circle sketch に inward offset を 2 回重ねて半径が 0 以下になるケースなど、`apply_sketch_offset_build()` の数値計算結果に依存する build-time failure を CRUD が事前検出できない。
  - 調査: `feature_crud.rs` は他のどの Feature 型についても **数値/幾何的な buildability を一切事前検証していない** (grep で確認: `depth`/`radius`/`distance`/`degenerate`/`collapse` に関する検証コードは存在せず、テストフィクスチャの値のみ)。例えば `Cut`/`Fuse`/`Intersect` の `refs_resolve_in_state` は `live_bodies_at.contains_key(target) && live_bodies_at.contains_key(tool)` のみで、boolean 演算が幾何的に失敗する (非交差、退化 solid 等) ケースは一切 CRUD で検出せず `build_bodies_from_features()` の `Err` に委ねている。`Extrude` の depth=0 相当のケースも同様。
  - 判断: CRUD 層の設計上の境界は「参照の構造的解決可能性 (id 存在・liveness・型クラス)」のみを検証し、「数値演算の結果が実際に成功するか」は一貫して `build_bodies_from_features()` に委譲するという、このコードベース全体で例外なく守られている architecture。round 4 で追加した「単一 Circle かどうか」の検査は「参照の型クラス」チェックの範囲内 (rustc の型チェックに近い判断) だが、round 5 が要求する「offset 演算を CRUD 内でシミュレートして collapse を予知する」のは、`apply_sketch_offset` の数値ロジックを CRUD 層に複製することを意味し、他のどの Feature 型にもない前例のない拡張。二重実装は今後の math 変更で drift するリスクも生む。
  - 本 Issue の scope 外と判断し棄却。CRUD は `SketchOffset` insert/edit を許可するが、collapse する入力は `build_bodies_from_features()` が既存の `DegenerateSketchElement` で正しく `Err` を返す (round 3 以前から acceptance test で検証済み) — CRUD レベルでの早期検出が無いだけで、データ破損やクラッシュは発生しない。この非対称性を体系的に解消したい場合 (CRUD に数値シミュレーションレイヤーを持たせる) は別 foundation Issue の scope。
