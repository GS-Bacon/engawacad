# Batch B-6 Codex 横断レビュー findings (non-blocking)

**Cycle**: post-#239 / #237 / #238 commit
**batch_start_sha**: d09312175afc87abb966abd9cc11fb6dcb9d98fb
**verdict**: pass (blocking=0)

## Medium findings (フォローアップ候補)

### F01 — `schema_version` peek の u64→u32 wrap

- file: `crates/engawa-format/src/document.rs` line ~69
- 現状: `peeked_version = value.get("schema_version").and_then(|v| v.as_u64()).map(|v| v as u32)`。`u32::MAX + 1` (= 4294967296) のような値が入ると wrap して `UnknownSchemaVersion` 判定を素通りする
- 修正案: `peeked_version` を `u64` のまま `CURRENT_SCHEMA_VERSION as u64` と比較し、上限確認後に `u32` へ落とす。回帰テストに `T_DEG_overflow_u32_plus_one` を追加
- 対応: 次サイクルで follow-up Issue 起票 → 修正

### F02 — Custom `Deserialize` が YAML 前提に狭まっている

- file: `crates/engawa-format/src/document.rs` line ~106
- 現状: `impl<'de> Deserialize<'de> for Document` が `serde_yaml::Value::deserialize` + `serde_yaml::from_value` を直接使う。非 YAML Serde フォーマット (bincode/msgpack) で読むと型不一致 / 意味変化
- 修正案: generic `Deserialize` は元の `RawDocument::deserialize` 経路に戻し、2 段階 schema_version 判定は `from_yaml` 専用入口に閉じ込める
- 対応: 次サイクルで follow-up Issue 起票 → 修正

## 自律モード判断

両者 medium / non-blocking / かつ「現状 YAML 経路のみが実利用されている」ため本サイクルは続行する。
次サイクル冒頭 (L-3 batch-select 後) に follow-up Issue を 1〜2 件起票して fix する。

---

# 過去 cycle の findings (archive)

(古いログは別 cycle で `verdict: fail | critical=0, high=1, medium=1, blocking=1` 等が残っていたが、本 cycle で上書きした)
