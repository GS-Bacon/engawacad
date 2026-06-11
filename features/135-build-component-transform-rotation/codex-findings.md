# Codex Findings — #135 build-component-transform-rotation

## STEP 7.5 Round 1 — non-blocking 記録

### F02 (medium) — IDENTITY3 / Vec3::zeros() の exact 比較

**file**: `crates/mycad-build/src/lib.rs:438`

**指摘**:
> `total_rotation != IDENTITY3` と `total_offset != Vec3::zeros()` を exact `f64` 比較で判定しているため、親 `+45°` / 子 `-45°` のような相殺ケースでは合成後も near-identity として `rotate` が走り、不要な座標ノイズを焼き込む。

**提案**:
> 行列積/回転済みオフセットの後に snap か epsilon 正規化を入れるか、identity/zero 判定を epsilon ベースにしてから `rotate`/`translate` を呼ぶ。

**判定**: 別 Issue 候補として記録のみ (本 Issue では非対応)。

**理由**:
- medium severity で blocking 対象外
- 「near-identity でのノイズ焼き込み」は数値モデル設計の論点 (ADR-007 の snap 規則と関連)
- 解決策の選定 (snap vs epsilon 閾値) が本 Issue 範囲を超え、`Tolerance` newtype 導入の議論にも繋がりうる
- 現状の T01-T09 テストは全て pass しており、本 Issue の完了条件 (rotation 配線) は満たしている
- 将来 phase で `Vertex.tolerance` 等の per-entity tolerance 導入 (ADR-004 §段階移行プラン Issue #34) と合わせて議論するのが筋

**追跡**: 必要に応じて別途 `bug(kernel)` ラベルで Issue 起票する候補。
