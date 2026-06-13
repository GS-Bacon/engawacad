# Codex findings (medium 受理 - non blocking)

## Round 3 verdict: pass (blocking=0, critical=0, high=0, medium=2)

### F01 (medium): extract_boundary_edges の同一 triangle 内重複 edge 除外
- **対応済み**: `seen_in_tri` を追加し、同一 triangle 内で重複量子化キーを 1 回だけカウント

### F02 (medium): 未使用ヘルパ
- **対応済み**: `extract_face_vertex_indices` / `extract_positions_for_keys` を削除
