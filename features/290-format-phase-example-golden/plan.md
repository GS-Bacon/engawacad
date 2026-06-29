## 自律判断ログ
- Issue body は `rect_polygon_slot.engawa` の golden 追加を In-Scope に挙げているが、その元 Issue #275 は cycle 76 で `needs-human` 退避済で `rect_polygon_slot.engawa` ファイル自体が存在しない (`ls examples/` で未確認)。**したがって本 Issue ではその golden 化を Out-of-Scope に倒し、existing landed Phase 10 example のみを対象にする** (= 検証不能な対象を待たず、確定済みファイルを golden 化する)。
- Phase 10 で landed している example は `circle_arc.engawa` (#273) と `ellipse_conic.engawa` (#274) の 2 件のみ (`git log --all -- examples/*.engawa` で確認)。これらを Issue body の「他の example も同様に golden 化されているか確認」に従って golden 化する。
- 既存 `crates/engawa-format/tests/golden_examples.rs` は Phase 0–9 の例 (simple_box 除く) を byte-identical golden で扱っており、本 Issue も同ファイルに test 関数を 2 つ追加する形で揃える。
- golden の期待文字列 (= `Document::from_path(file).to_yaml()` の出力) は schema_version 正規化 (`schema_version: 1` → migrate 後の `schema_version: 2`) とフィールド省略動作の確認を兼ねるため、**実装フェーズで `cargo test` をいったん赤で走らせて出力を採取し、それを golden に貼り付ける**手順を取る (= TDD っぽい red→green の運用、既存 `extruded_rect` で他 Issue が取った経路と同じ)。

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `crates/engawa-format/tests/golden_examples.rs` に `circle_arc.engawa` (#273) の byte-identical golden 追加 | `rect_polygon_slot.engawa` (#275) の golden 追加 (元 example が landed していないため検証不可、#275 解消後の別 Issue に逃がす) |
| 同ファイルに `ellipse_conic.engawa` (#274) の byte-identical golden 追加 | `simple_box.engawa` 他 Phase 0–9 の未 golden example の retro-fit (本 Issue は Phase 10 scope) |
| from_path → to_yaml の正規化結果が Phase 10 で安定していることを 1 commit で確定する | Slot tessellation 復活 / `rect_polygon_slot.engawa` の追加 (Phase 11+ で扱う) |
| 既存テスト群が green であることの確認 (golden retro-fit による回帰がない) | `sketch_*.engawa` 系 (#218/#215 由来) の golden 化 (本 Issue は Phase 10 sketch curves scope に限定) |

## Non-Goals
- `rect_polygon_slot.engawa` の golden: #275 が landed していないため、ファイル自体が存在せず golden 化不可。#275 が再開・close された後に追加 Issue で扱う
- `simple_box.engawa` 等 Phase 0 example の golden 化: 本 Issue scope 外 (別 Issue で扱える)
- `sketch_extrude_pillar.engawa` / `sketch_extrudecut_hole.engawa` / `sketch_via_face_entity_ref.engawa` / `sketch_circle_on_face.engawa` の golden 化: Phase 8 由来 (#218 系) で Phase 10 scope に含まれない
- `examples_smoke.rs` 側の修正: 既存 `circle_arc` / `ellipse_conic` smoke は landing 済 (`crates/engawa-build/tests/examples_smoke.rs`)、本 Issue で触る必要なし
- to_yaml の挙動変更 / schema migration の仕様変更: 本 Issue は wire-format pin であり、現在の挙動を golden として固定するだけ

## 実装対象
- Issue: #290
- 影響クレート/ファイル: `crates/engawa-format/tests/golden_examples.rs` のみ
- 変更内容: 末尾に 2 つの `#[test]` 関数 (`golden_circle_arc`, `golden_ellipse_conic`) を追加。各関数は `assert_golden(filename, golden_str)` を呼ぶだけ (既存 helper を再利用)
- 既存関数の修正なし (新規 #[test] 関数の追加のみ)

### 追加する関数の素描 (golden 文字列は実装フェーズで確定)
```rust
#[test]
fn golden_circle_arc() {
    // #273 で landed した Phase 10 Circle example。
    // schema_version: 1 で書かれているため migration 後の to_yaml は schema_version: 2 になる。
    assert_golden(
        "circle_arc.engawa",
        concat!(
            // 実装フェーズで cargo test の left/right diff から確定
        ),
    );
}

#[test]
fn golden_ellipse_conic() {
    // #274 で landed した Phase 10 Ellipse / Conic example。
    assert_golden(
        "ellipse_conic.engawa",
        concat!(
            // 実装フェーズで cargo test の left/right diff から確定
        ),
    );
}
```

## 設計方針
- 決定性要件: `Document::to_yaml()` は既存テスト群 (`golden_extruded_rect` 等) で同一入力→同一出力が前提化されているため、本 Issue は同じ前提に乗る (T01 で再確認のみ)
- B-rep トポロジー: 本 Issue は format 層のテキスト round-trip 検証であり kernel 不変量に介入しない (N/A)
- 退化幾何: 同上 N/A (例ファイルが既に landing 済 = build 可能なことは `examples_smoke.rs` で担保)
- derive 規約: 変更なし (新規型なし)
- エラーハンドリング: `from_path` の panic ハンドリングは既存 helper `assert_golden` が `unwrap_or_else` で実装済、再利用
- workspace.dependencies: 変更なし

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_determinism | 決定性 | 既存 golden テスト全体が 2 回連続で同一結果を返す (Rust テストランナーは同一バイナリで複数 invocation 可能) | `cargo test --test golden_examples` を 2 回連続実行で両方 0 failures |
| T02_circle_arc | 正常系 | `circle_arc.engawa` を `Document::from_path` → `to_yaml` した結果が golden 文字列と byte-identical | `assert_eq!` 通過 |
| T03_ellipse_conic | 正常系 | `ellipse_conic.engawa` を同様に round-trip → byte-identical | `assert_eq!` 通過 |
| T04_boundary_schema_migration | 境界 | `circle_arc.engawa` は `schema_version: 1` で書かれている。golden 文字列が `schema_version: 2` から始まることを確認 (migration が走っている証拠) | golden 先頭が `schema_version: 2\n` |
| T05_boundary_existing_unchanged | 境界 | 既存の `golden_cylinder` / `golden_extruded_rect` 等が引き続き通る (本 Issue による回帰がない) | 既存テストが 0 failures |

## 幾何的不変条件チェックリスト
- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — **N/A** (本 Issue は format 層のテキスト golden 検証で kernel に触れない)
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか — **N/A** (同上)
- [ ] flip_normals / same_sense の意味論が明確か — **N/A** (同上)
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか — **N/A** (同上)
