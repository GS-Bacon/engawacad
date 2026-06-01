# 修正指示 — F01: ComponentRef 検証漏れ

## 問題
`Document::validate()` (`crates/mycad-format/src/document.rs`) が feature_id の検証しか行っておらず、
`Component.reference`（`ComponentRef`）の中身を検証していない。
そのため `ComponentRef::StdLib("".into())` や `ComponentRef::File("".into())` を手で組んだ Document が
`to_yaml()` を通過して不正 `.mycad` が書ける。

## 修正内容

### 1. `document.rs` — validate() に ComponentRef 検証を追加

`Component` を再帰走査する箇所に、以下のチェックを追加:
- `reference` が `Some(ComponentRef::StdLib(path))` の場合: `path.is_empty()` なら
  `FormatError::InvalidReference { value: "stdlib://".into(), reason: "stdlib reference must have a non-empty path after 'stdlib://'" }`
- `reference` が `Some(ComponentRef::File(path))` の場合: `path.is_empty()` なら
  `FormatError::InvalidReference { value: "".into(), reason: "reference must not be empty" }`

既存の `ComponentRef::from_str` がすでに同じ条件でエラーを返しているので、
そのロジックを validate() 側にも反映するだけ。

### 2. `document.rs` のテスト — 負例テストを追加

```rust
#[test]
fn t15_stdlib_empty_path_rejected_by_validate() {
    let mut doc = Document::new("Test");
    doc.root_component.reference = Some(ComponentRef::StdLib("".into()));
    let err = doc.to_yaml().unwrap_err();
    assert!(matches!(err, FormatError::InvalidReference { .. }));
}

#[test]
fn t16_file_empty_path_rejected_by_validate() {
    let mut doc = Document::new("Test");
    doc.root_component.reference = Some(ComponentRef::File("".into()));
    let err = doc.to_yaml().unwrap_err();
    assert!(matches!(err, FormatError::InvalidReference { .. }));
}
```

## 完了条件
- `cargo test -p mycad-format` が全テスト通過（t15, t16 含む）
- `cargo xtask ci` green
