## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| 既存 Boolean example 9 本の headless ビルド/テッセレーション・プリチェック (mycad export) | example のリネーム/新規追加 |
| 目視確認チェックリスト整備とユーザーへの起動手順提示 | viewer (mycad-api/web) や kernel のコード変更 |
| 視覚問題発見時の別 Issue 起票 (close 前に解消) | CLAUDE.md の「mycad-viewer 未実装」記述修正 |
| 後処理: ROADMAP Phase 4 ✅ + milestone #5 close + Issue #35 close | example YAML の golden byte 固定 (#24 管轄) |

## Non-Goals

- GLM による実装・テスト追加 (本 Issue にコード成果物なし)
- example YAML の golden byte 固定 (#24 の管轄)
- 退化三角形の自動検出実装 (#23 の管轄)

## 実装対象

これはコード実装 Issue ではなく人間検証 gate。書くべき crates/** の Rust コードは原則無い。

変更対象:
- `ROADMAP.md` — Phase 4 ヘッダを ✅ に更新・完了日追記 (後処理)
- `features/35-boolean-view-gate/visual-checklist.md` — 目視確認チェックリスト (新規)

## 設計方針

**フロー適応** (ユーザー合意済み):
- /3ai の GLM 実装ループ (STEP 5.5〜6.6) は skip — crates/** への変更が無いため。
- GLM 設計レビュー (STEP 3) も skip — 描画結果という実行時/視覚事実は markdown レビューでは判定不可。
- 目視確認は **ユーザーがローカル (ディスプレイ有) で実施**。

**example 命名**: 既存名 `boolean_box_<op>` を維持。Issue 本文の `boolean_<op>_box` は表記揺れ。

**headless プリチェック手段**: `mycad export <file> -o <out.stl>` — parse→build→tessellate を同期実行し、失敗時に非ゼロ終了 (`crates/mycad-cli/src/main.rs:51-91`)。

### 数値モデル

N/A (テッセレーション結果の人間確認 gate。数値判断はカーネル実装済み)

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | headless smoke | 全 9 example が `mycad export` exit 0 & STL 非空 | 各 exit code = 0, file size > 0 |
| T02 | 人間目視 | 各 example を `mycad view` で起動しチェックリスト①〜④を確認 | visual-checklist.md の全項目 ✅ |

対象 example:
- `examples/boolean_box_fuse.mycad` (box ∪ box)
- `examples/boolean_box_cut.mycad` (box − box)
- `examples/boolean_box_intersect.mycad` (box ∩ box)
- `examples/boolean_box_void.mycad` (内部 void shell)
- `examples/boolean_cut_cylinder_hole.mycad` (box − cyl = 丸穴)
- `examples/boolean_cut_sphere_dimple.mycad` (box − sphere = 窪み)
- `examples/boolean_fuse_box_cyl.mycad` (box ∪ cyl)
- `examples/boolean_intersect_box_cyl.mycad` (box ∩ cyl)
- `examples/boolean_intersect_cyl_sphere.mycad` (cyl ∩ sphere)

## 幾何的不変条件チェックリスト

- N/A (人間目視確認の記録。カーネルの幾何的整合性は #33/#34 実装時に検証済み)
