# Codex Findings — #92 face_ids (non-blocking)

## F01 (medium) — web/src/mesh.ts
`validateMesh` が `face_ids` の長さ不変量（`face_ids.length == indices.length / 3`）を未検証。
TS 側テストでも 3 三角形 + face_ids 1 件のメッシュを通してしまう。
→ 後続 Issue（#93 API 実装など）で `validateMesh` に face_ids 検証を追加するか、
  または別 Issue として起票する。

## F02 (low) — crates/mycad-kernel/src/tessellation/mod.rs:t09_merge_determinism
`t09_merge_determinism` が `face_ids` を比較していない。
→ `assert_eq!(merged1.face_ids, merged2.face_ids)` を追加推奨。

## F03 (low) — crates/mycad-kernel/tests/face_ids_acceptance.rs:T03
T03 が `starts_with("N(")` のみで `;F:` を未確認。Edge/Vertex id が混入しても通過する。
→ アサーション強化推奨: `id.contains(";F:")` を追加。
