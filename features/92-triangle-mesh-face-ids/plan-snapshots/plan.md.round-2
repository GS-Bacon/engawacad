## In-Scope / Out-of-Scope
<!-- ADR-006 §plan.md 必須セクション。GLM SCOPE ペルソナが存在を検証する。 -->
| In-Scope | Out-of-Scope |
|----------|--------------|
| `TriangleMesh` に `face_ids: Vec<String>` フィールド追加 | ブラウザ側のピッキング UI (#94) |
| 各 strategy 関数が push する三角形ごとに face の文字列 id を充填 | 書き込み API (#93) |
| `Face.name: Option<EntityRef>` → 文字列化（`canonical_name()`、None は `""`） | NURBS / 自由曲面の face 参照 |
| `face_ids.len() == triangle_count()` の不変量テスト | face_ids を使った集約・フィルタリング |
| `web/src/generated/TriangleMesh.ts` の再生成（`Array<string>` 追加）+ xtask ゴールデン更新 | カメラ・選択状態の永続化 |
| 決定性テスト（同一面 → 同一 id）と既存テストのリグレッション確認 | |

## Non-Goals
<!-- Out-of-Scope と同内容でも重複 OK。dispatch-codex-auto.ts の guard が参照する。 -->
- viewer-pick の実装（#94）
- 書き込み API（#93）
- face_ids を使った任意の集約・フィルタリング
- EntityRef を新たな wire フォーマットで露出すること（文字列化のみ。`EntityRef` 構造体そのものを TS に出すのは本 Issue 対象外）

## 実装対象
<!-- Issue: #92 -->
<!-- 影響クレート/ファイル: crates/mycad-kernel/src/tessellation/mod.rs, crates/xtask/src/main.rs, web/src/generated/TriangleMesh.ts, web/src/generated/BodyMesh.ts -->

**影響クレート/ファイル:**
- `crates/mycad-kernel/src/tessellation/mod.rs` — `TriangleMesh` 構造体 + 全 strategy 関数 + `push_triangle` + `merge_meshes`
- `crates/xtask/src/main.rs` — `TRIANGLE_MESH_GOLDEN` / `BODY_MESH_GOLDEN` ゴールデン文字列更新
- `web/src/generated/TriangleMesh.ts` / `web/src/generated/BodyMesh.ts` — `cargo xtask gen-ts` で再生成（コミット必須）

**変更する型・関数のシグネチャ:**

1. **`TriangleMesh` 構造体（mod.rs L22 付近）** — フィールド追加

   before:
   ```rust
   pub struct TriangleMesh {
       pub positions: Vec<[f64; 3]>,
       pub normals: Vec<[f64; 3]>,
       pub indices: Vec<u32>,
   }
   ```
   after:
   ```rust
   pub struct TriangleMesh {
       pub positions: Vec<[f64; 3]>,
       pub normals: Vec<[f64; 3]>,
       pub indices: Vec<u32>,
       /// 三角形ごとの所属 face の文字列 id。長さは triangle_count() に一致。
       /// 名前なし face（Face.name == None）は空文字列。
       pub face_ids: Vec<String>,
   }
   ```
   `new()` / `Default` 実装にも `face_ids: Vec::new()` を追加。

2. **`push_triangle`（L864 付近）** — face id 引数を追加し、退化スキップしなかった三角形にのみ face_id を push する

   before:
   ```rust
   fn push_triangle(mesh: &mut TriangleMesh, a: u32, b: u32, c: u32) {
       // 退化チェック後 indices.push(a/b/c)
   }
   ```
   after:
   ```rust
   fn push_triangle(mesh: &mut TriangleMesh, a: u32, b: u32, c: u32, face_id: &str) {
       // 退化でスキップする場合は face_id も push しない（不変量維持）
       // push する場合のみ mesh.face_ids.push(face_id.to_string())
   }
   ```

3. **各 strategy 関数** — `tessellate_face_fan_from_points` / `tessellate_face_earcut` / `tessellate_face_uv_grid` / `tessellate_face_sphere` / `tessellate_sphere_face_trimmed` および直接 `indices.push` している箇所すべてに、対象 face の文字列 id を伝播。`tessellate_solid_with` のループ（L107）で `let face_id = face.name.as_ref().map(EntityRef::canonical_name).unwrap_or_default();` を一度算出して各 strategy 関数に渡す。

4. **`merge_meshes`（L883 付近）** — `face_ids` も連結する。

## 設計方針
<!-- 決定性要件 / derive 規約 / エラーハンドリング -->

- **決定性**: `face_ids` の各要素は `face.name`（`Option<EntityRef>`）の `canonical_name()` を文字列化したもの。`EntityRef` 自体は `feature_id`/`kind`/`role` で決まる決定的な値であり、同一 Solid を再テッセレーションすれば同一 face は同一文字列 id を返す。生成順は `tessellate_solid_with` の face イテレーション順に従い既存の三角形生成順と完全一致するため、`face_ids` の順序も決定的。
- **文字列表現の選択（`canonical_name()`）と ADR-005 F02 との関係**: face_id 文字列には `EntityRef::canonical_name()`（例 `N(box_1;F:top)`、`feature.rs:78-96`）を採用する。ADR-005 Decision 6（F02）は「canonical name grammar は内部表現で `.mycad` の wire/on-disk format ではない（永続化は構造化形式 `{feature_id, kind, role}`）」と定めるが、本 Issue が対象とする `face_ids` は **`.mycad` に永続化されない `TriangleMesh` の派生出力メタデータ**（Feature history が真実の源、メッシュは再生成物）であり、F02 が制約する EntityRef の永続化経路には当たらない。フロントは face_id を**不透明な面識別トークン**として扱い（文字列の中身を parse しない）、書き込み API(#93) へ渡すキーとして使う。`EntityRef` の唯一の文字列表現メソッドが `canonical_name()` であり、ADR-008 Decision 2 が要求する「`EntityRef` 文字列表現」に最も忠実。face_id → face の逆引きは #93 の責務でありここでは扱わない。
- **None face の扱い**: `Face.name == None` の場合は `unwrap_or_default()` で空文字列 `""`。これは Issue 本文の `Vec<String>` 契約と TS `Array<string>` 契約に忠実。生 index は露出しない（ADR-005 準拠）。
- **不変量**: `face_ids.len() == indices.len() / 3 == triangle_count()`。`push_triangle` が退化三角形をスキップする際は face_id も push しないことでこの不変量を維持する（これが本実装の最重要ポイント）。
- **B-rep トポロジー妥当性**: 本 Issue はメッシュのメタデータ付与のみで、トポロジー（V/E/F）には一切変更を加えない。Euler-Poincaré は不変。N/A。
- **退化幾何の扱い**: 既存の `push_triangle` の退化スキップ挙動をそのまま踏襲。新たな退化判定は追加しない。
- **derive 規約**: `TriangleMesh` は既に `Debug, Clone, Serialize, Deserialize, TS` を持つ。`Vec<String>` は全て自動導出に対応するため derive 変更不要。
- **エラーハンドリング**: 新規エラー経路なし。`canonical_name()` は `String` を返す純関数で失敗しない。
- **workspace.dependencies 規約**: 新規依存なし。`EntityRef` は `mycad-format` にあり kernel は既に `use mycad_format::EntityRef` 済み。

### 数値モデル
<!-- 本 Issue は文字列メタデータ付与のみ。tolerance/ε 判定は新規導入しない。既存の退化三角形スキップ閾値（push_triangle 内）に変更を加えない。 -->
N/A（新規の数値判定なし。既存の `push_triangle` 退化スキップ閾値を踏襲するのみ）。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | cuboid を 2 回テッセレーションし `face_ids` が完全一致 | `assert_eq!(m1.face_ids, m2.face_ids)` |
| T02 | 不変量 | cuboid: `face_ids.len() == triangle_count()` | `assert_eq!` |
| T03 | 正常系 | cuboid: 各 face id が `N(...;F:...)` 形式の非空文字列で、6 種の id が出現 | 期待 id 集合と一致 |
| T04 | 不変量 | cylinder / sphere でも `face_ids.len() == triangle_count()` | `assert_eq!` |
| T05_boundary_unnamed | 境界 | `Face.name == None` の face を含む Solid で当該三角形の id が `""` になる | 空文字列を含む |
| T06_degen_skip | 退化 | 退化三角形がスキップされる入力で `face_ids.len()` が push 後の triangle_count と一致（ズレない） | `assert_eq!` |
| T07 | TS生成 | xtask ゴールデン更新後 `TRIANGLE_MESH_GOLDEN` に `face_ids: Array<string>` を含む | golden 一致 |

## 幾何的不変条件チェックリスト
<!-- 本 Issue は Boolean/Partition/Assemble 系ではない（メッシュへのメタデータ付与）。 -->
- [ ] N/A — partition 出力の polygon 頂点順（本 Issue は partition を扱わない）
- [ ] N/A — 各プリミティブの outer_loop 向き（本 Issue で変更しない）
- [ ] N/A — flip_normals / same_sense 意味論（本 Issue で変更しない）
- [ ] N/A — pslg_subdivide 出力向き（本 Issue で扱わない）
