# feat: ExtrudeCut Feature 追加と UI — 押出でモデルから体積を除去する

## 概要

Phase 6 の完了条件「Extrude(add) と ExtrudeCut(remove) が別 Feature として `.mycad` に積まれる」のうち、
除算側を本 Issue で実装する。ADR-008 Decision 1 に従い `Feature::ExtrudeCut` を新規 variant として
`mycad-format` に追加し（`make_extrusion` + Boolean `Cut` の合成、Phase 4 実装済み）、
ブラウザの押出パネルに「押出カット」ボタンを追加して UI からも実行できるようにする。

前提: extrude-op (#N) が closed であること。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `Feature::ExtrudeCut` variant を `mycad-format/src/feature.rs` に追加（フィールド構造は `Extrude` と同一）| `ThroughAll`（"最後まで"フラグ）等の拡張オプション |
| `mycad-build/src/lib.rs` に `ExtrudeCut` ディスパッチ追加（`make_extrusion` + `boolean_cut` 合成） | フィレット・面取り |
| TS 型の自動再生成（ts-rs 経由） | テーパー角・両側カット |
| extrude-op パネルに `data-testid="btn-extrude-cut"` ボタンを追加し `POST /api/v0/features` で実行 | undo/redo |
| 退化ケース・境界ケースのテスト（plan.md `### 数値モデル` セクション必須） | |

## Non-Goals

- ThroughAll フラグ
- テーパー角・両側カット
- undo/redo

## 完了条件

- `cargo test --workspace` で `ExtrudeCut` のユニットテストが通る（box に対して cube をカット → 頂点/面数の期待値が変化する）
- 決定性テスト: 同一入力で 2 回ビルドした結果が byte-for-byte 等しい
- 退化ケーステスト: カット形状がソリッドに全く含まれない場合に適切にエラーを返す
- Playwright: `simple_box.mycad` で面を選択 → 「押出カット」ボタンが visible になる
- Playwright: 深さを入力して「押出カット」クリック → `POST /api/v0/features` が 200 → `GET /api/v0/mesh` でメッシュ頂点数が減少または変化している
- `mycad export` で出力した `.mycad` YAML に `ExtrudeCut` エントリが存在する
- `cargo xtask ci` および Playwright E2E が通る

## 関連 ADR

- ADR-008: Decision 1（ExtrudeCut = 別 variant）
- ADR-004: 数値モデル（Boolean Cut の tolerance）→ plan.md に `### 数値モデル` セクション必須

## 実装ヒント

`feature.rs` の `Feature::Extrude` 直後に `ExtrudeCut` variant を追加。
`mycad-build/src/lib.rs` の match arm に `Feature::ExtrudeCut => { let tool = make_extrusion(...); boolean_cut(base, tool) }` を追加。
幾何リスクが高いため `batch:kernel`（STEP 7.5 Codex ゲート保持）。
plan.md には `### 数値モデル` セクション（`ε = 1e-10`、退化判定基準、ADR-004 準拠方針）を必ず含める。
