# debug-spec for #241 R6 — evaluate_scope() public API での name validation 適用

## R5 で残った Codex 指摘 (棄却継続を除く)

| ID | severity | persona | 対応 |
|----|----------|---------|------|
| A-F02 / C-F01 | medium | architect / contrarian (2 ペルソナ独立) | **採用 (本 debug-spec で対応)** |
| A-F01 | high | architect | **棄却継続** (validate() の expr 評価系は後続 Issue) |
| M-F02 | medium | migration | **棄却継続** (A-F01 と同主旨) |
| M-F01 | high | migration | **棄却継続** (5 round 連続、ADR-015 で許容済の Phase 9 内破壊的変更) |

## R6 修正方針 (mini fix)

### 現状の不備

`crates/engawa-format/src/variable.rs::evaluate_scope` 公開 API は重複名 (`EvalError::DuplicateVariable`) のみチェックしている。空名や不正文字 (例: `name: ""`、`name: "a b"`) の Variable を直接渡すと:

1. `extract_var_refs` は `${}` (空名) 参照を依存グラフから drop する
2. `lookup_var` は空名キーで解決を試みる
3. 結果として scope rule を bypass される (depend graph に載らないが評価されうる)

`Document::validate()` 経由なら `FormatError::InvalidVariableName` で reject されるが、`evaluate_scope` 直呼びはこのガードを通らない (二重防御の欠落)。

### 修正方針

`evaluate_scope` 冒頭で `check_unique_names` の後に `check_valid_names` を呼ぶ:

```rust
pub fn evaluate_scope(
    doc_vars: &[Variable],
    sketch_vars: &[Variable],
) -> Result<IndexMap<String, f64>, EvalError> {
    check_valid_names(doc_vars, "document")?;
    check_valid_names(sketch_vars, "sketch")?;
    check_unique_names(doc_vars, "document")?;
    check_unique_names(sketch_vars, "sketch")?;
    // ... 既存処理 ...
}

fn check_valid_names(vars: &[Variable], scope: &str) -> Result<(), EvalError> {
    for v in vars {
        if v.name.is_empty() {
            return Err(EvalError::InvalidVariableName {
                scope: scope.to_string(),
                name: v.name.clone(),
                reason: "variable name must not be empty",
            });
        }
        if !v.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
            return Err(EvalError::InvalidVariableName {
                scope: scope.to_string(),
                name: v.name.clone(),
                reason: "variable name contains invalid characters",
            });
        }
    }
    Ok(())
}
```

### EvalError 拡張

```rust
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EvalError {
    // ... 既存 variants ...

    /// 公開 API `evaluate_scope` 経由で不正な variable name が渡された
    #[error("invalid variable name in {scope} scope: {name:?} ({reason})")]
    InvalidVariableName {
        scope: String,
        name: String,
        reason: &'static str,
    },
}
```

### 既存テストへの影響

すべて valid な名前を使っているため影響なし。

## 追加で書いてほしいテスト

```rust
/// T_DEG_doc_var_empty_name (R6 regression):
/// evaluate_scope 直呼びでも doc.variables の空名 → InvalidVariableName(document)
#[test]
fn t_deg_doc_var_empty_name() {
    let doc_vars = vec![Variable { name: "".into(), expr: "1".into() }];
    let result = evaluate_scope(&doc_vars, &[]);
    match result {
        Err(EvalError::InvalidVariableName { scope, name, .. }) => {
            assert_eq!(scope, "document");
            assert_eq!(name, "");
        }
        other => panic!("expected InvalidVariableName, got {other:?}"),
    }
}

/// T_DEG_sketch_var_invalid_char (R6 regression):
/// evaluate_scope 直呼びでも sketch.variables の不正文字 → InvalidVariableName(sketch)
#[test]
fn t_deg_sketch_var_invalid_char() {
    let sketch_vars = vec![Variable { name: "a b".into(), expr: "1".into() }];
    let result = evaluate_scope(&[], &sketch_vars);
    match result {
        Err(EvalError::InvalidVariableName { scope, name, .. }) => {
            assert_eq!(scope, "sketch");
            assert_eq!(name, "a b");
        }
        other => panic!("expected InvalidVariableName, got {other:?}"),
    }
}
```

## 試した修正と結果

- [x] R1-R5 完了 (基本実装 → 2-stage scope → 重複名 reject → validate 強化)
- [x] R5 STEP 7.5: Codex blocking=2 (mini fix 候補 medium x2 + 棄却継続)
- [ ] R6 予定: evaluate_scope 公開 API での name validation + 2 件 regression test

## 次にやること

GLM core dispatch で:
1. `EvalError::InvalidVariableName` 追加
2. `check_valid_names` helper 追加
3. `evaluate_scope` 冒頭で `check_valid_names` を呼ぶ

続けて GLM test dispatch で 2 件 regression test 追加 → CI green → STEP 7.5 R6 再 Codex review (これが codex_loops 6 で `> 5` のため後続は assert-critical-zero フォールバック適用となる)。
