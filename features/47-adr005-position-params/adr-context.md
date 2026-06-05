# ADR-005 抜粋（#47 レビュー用コンテキスト）

本 Issue は §7 の「canonical local frame の必須化」段落直後に position パラメータの扱いを 1 段落追記する docs-only Issue。
関連する §6 (canonical name grammar) と §7 全文を以下に抜粋する。

## §6. 基底名の canonical grammar と charset

- grammar 文字列 `<feature_id>;<kind>:<role>` は内部 canonical stable name（ハッシュ seed・同一性判定用）。
  `.mycad` 上の wire format ではない。on-disk は構造化形式 `{ feature_id, kind, role }`。
- `feature_id` 例: `box_1` / `kind`: `F`/`E`/`V` / `role`: `top`/`seam` 等（各 maker が割当）。
- 文字集合は `[A-Za-z0-9_-]`、`;` と `:` は予約。
- 例: `cyl_1;F:lateral`, `cyl_1;E:seam`, `box_1;F:top`。

## §7. role 付与ルール（全文）

**基本規約**: 生成順カウンタは禁止(index と同じ脆さ)。各 maker が face/edge/vertex すべてに
一意・決定的な role を明示列挙する。隣接面 role からの導出式は採らない。

**canonical local frame の必須化**: 各 maker は role を導く基準となる canonical local frame
(座標系・面法線の向き・loop 巻き方向・周期面の seam 原点)を、feature **パラメータから一意・決定的**
に導く規則を定義しなければならない。内部リファクタで別名化させないため、role 表はこの frame に従って
固定する。具体 frame と role 表は各 primitive の role 付与実装 issue で確定する。

→ **本 Issue はここに position 段落を挿入する。**

**face role の例**:

| Feature | face role |
|---------|-----------|
| CreateBox | `top` / `bottom` / `front` / `back` / `left` / `right` |
| CreateCylinder | `lateral` / `cap_top` / `cap_bottom` |
| CreateSphere | `surface` |
| Extrude | `cap_start` / `cap_end` / `side_<sketch要素安定名>` |

**edge/vertex role**: maker が明示命名する。同一 Solid 内で一意であること。

**自己隣接・周期トポロジー**: seam や極は専用 role を必ず割り当てる。
（sphere 極 edge: `north_pole`/`south_pole`、seam edge: `seam`、cylinder rim: `cap_top_rim`/`cap_bottom_rim`）

## 現状コード（feature.rs:276-284）

```rust
CreateCylinder { id: String, radius: f64, height: f64 },
CreateSphere { id: String, radius: f64 },
```

→ position フィールドはまだ存在しない。origin/center の追加は impl Issue #48。
