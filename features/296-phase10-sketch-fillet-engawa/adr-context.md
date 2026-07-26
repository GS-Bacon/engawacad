# ADR-017 抜粋 (Issue #296 Sketch Fillet 関連部分)

出典: `docs/decisions/017-phase10-sketch-curves-and-edits.md` (Status: Accepted)

## §3 スケッチ編集7種の API 抽象 (該当行のみ抜粋)

採用: **各編集オペレーションを独立した `Feature` enum variant として履歴に残す (純関数モデル)**。

| 編集op | feature variant (`Feature::*`) | 主要パラメータ |
|--------|-----------------------------|--------------|
| Sketch Fillet | `SketchFillet` | `sketch_ref`, `vertex_ref: (e1_id, e2_id)`, `radius` |

ID 安定性:
- 編集後も既存 element ID は **保持** する
- 分割される場合 (例: Trim で element a が2つに分かれる) は `{a}_split_{n}` 派生 ID を決定的に割り当てる
- これは ADR-005 Topological Naming の「編集後も意味的に同一であれば同 ID を保つ」原則と整合

参照解決失敗時のエラー:
- `BuildError::SketchRefNotFound { sketch_id, element_id }` を導入 (engawa-kernel では既存 `KernelError::SketchNotFound` がこれに相当)
- `engawa-build` の `Feature::apply()` 内で fail-fast

dispatch 順序:
- 編集op は `engawa-build` で先頭から線形に適用する
- in-place mutation ではなく純関数 (各op が新しい `Vec<SketchElement>` を返す)

## Migration Plan (該当箇所抜粋)

5. **#277 (Offset / Sketch Fillet / Sketch Chamfer)** — Offset/Fillet/Chamfer。`Vec<element_id>` selection の表現を確立

(#277 は #295 Offset / #296 Fillet / #297 Chamfer に分割済み)

## 本 Issue での override (plan.md 自律判断ログ参照)

- `sketch_ref: EntityRef` → `sketch: String` に変更。理由: #295 で Codex architect が「EntityRef は B-rep Face/Edge/Vertex 専用 (ADR-005 契約)」として refute 済み。既存 `Feature::Extrude.sketch: String` の慣習に合わせる。
- `vertex_ref: (e1_id, e2_id)` タプル → `elem1_id: String, elem2_id: String` フラット2フィールドに変更。理由: 既存 `Feature::Fuse{target,tool}` / `Feature::Cut{target,tool}` と同型のフラット命名の方が serde/YAML 可読性が高く、既存コード規約と整合する。Issue #296 本文の命名ともそのまま一致する。
