## 自律判断ログ (Codex intent-check aligned:no 後の自律修正)

Codex intent-check は次の 3 点を指摘した:

1. T02/T03/T_BOUNDARY_empty_expr の期待結果が Issue 本文に未記載 → 本 plan.md のテスト計画表に明示期待値を全 ID で記述する。
2. `${var_name}` 記法を ADR で一本化 → Issue 本文が `ADR-014` と引用しているのは誤りで、実際は **ADR-015 §2** に `${name}` 構文 + Sketch → Document の lexical scope lookup として確定済み。本 plan は ADR-015 を参照する。
3. 独立 Out-of-Scope 節を明記、`### 数値モデル` 節は不要 → 削除し In-Scope/Out-of-Scope 表 + Non-Goals 節を独立して記述。

ADR-015 Open Questions のうち本 Issue で詰めるもの:

- **Q1** (循環参照エラー context 粒度): cycle の閉じた path を `CircularDependencyError { cycle: Vec<String> }` に格納する。`["a", "b", "c", "a"]` 形式 (戻り点を含む) で末端からの順序を持つ。
- **Q2** (string literal ネスト): 本 Issue では `"prefix-${var}-suffix"` interpolation を **Out-of-Scope** とする。理由: ADR-015 Q2 が「ADR-016 で確定」と書いているが ADR-016 は engawa CLI 命名規約で interpolation を扱っていない。expr 全体が数式 (純粋 var + 四則 + 括弧) という最小スコープで先に通し、interpolation は後続 ADR で扱う。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `Variable { name: String, expr: String }` 型を `engawa-format/src/variable.rs` に新設 | string literal interpolation (`"prefix-${var}-suffix"`) — ADR-015 Q2、後続 ADR で扱う |
| `Document.variables: Vec<Variable>` 追加 (default empty) | 関数呼び出し (`sin`, `cos`, `sqrt`, …) |
| `CreateSketch.variables: Vec<Variable>` 追加 (default empty) | boolean 演算 / 比較演算子 |
| Equation 評価エンジン: 四則 (`+ - * /`) + 単項マイナス + 括弧 + 数値リテラル + `${name}` 参照 | Feature の数値 field 内での Variable 参照 (`width: ${w}` 形式) — 別 Issue |
| topological sort 依存解決 + 循環検出 | Document/Component を越える global shared variables — Phase 10+ |
| Sketch (近) → Document (遠) の lexical scope lookup | 数値の整数/小数型分離 (常に `f64`) |
| エラー型: `Circular { cycle }` / `Undefined { name }` / `EmptyExpression` / `Parse { reason }` | parse cache / メモ化 / 増分評価 |

## Non-Goals

- string literal 内 `${var}` interpolation (ADR-015 Q2 を別 ADR に倒す)
- 関数呼び出し / 数学関数 (`sin` / `cos` / `sqrt` / `pow` 等)
- Feature の数値 field 内での `${var}` 参照解決 (parser は Variable.expr 内のみで動作)
- 数値以外の値型 (string / boolean / vector)
- Component 階層を越える variable 解決 (children component の variable は parent 不可視)
- Variable persistence schema migration (#239 で `schema_version` 入口は実装済 — 本 Issue は v1 内拡張で互換)

## 実装対象

<!-- Issue: #241 -->
<!-- 影響クレート/ファイル: crates/engawa-format/src/ (variable.rs 新規 / lib.rs / document.rs / feature.rs / error.rs) -->

### 新規ファイル

- `crates/engawa-format/src/variable.rs`
  ```rust
  pub struct Variable {
      pub name: String,
      pub expr: String,
  }
  pub enum EvalError {
      CircularDependency { cycle: Vec<String> },
      UndefinedVariable { name: String },
      EmptyExpression { name: String },
      Parse { name: String, reason: String },
  }
  pub fn evaluate_scope(
      doc_vars: &[Variable],
      sketch_vars: &[Variable],
  ) -> Result<indexmap::IndexMap<String, f64>, EvalError>;
  ```
  内部: recursive-descent parser (新規依存追加なし) + Kahn's topological sort (lexicographic tiebreak で決定性確保)。

### 既存ファイル変更

- `crates/engawa-format/src/lib.rs` — `pub mod variable;` 追加 + `pub use variable::{Variable, EvalError};`

- `crates/engawa-format/src/document.rs` — `Document` 構造体に `variables` 追加:

  **before**:
  ```rust
  pub struct Document {
      #[serde(default = "default_schema_version")]
      pub schema_version: u32,
      pub version: String,
      pub root_component: Component,
  }
  ```
  **after**:
  ```rust
  pub struct Document {
      #[serde(default = "default_schema_version")]
      pub schema_version: u32,
      pub version: String,
      #[serde(default, skip_serializing_if = "Vec::is_empty")]
      pub variables: Vec<Variable>,
      pub root_component: Component,
  }
  ```
  `RawDocument` も同様に変更。`Document::new` で `variables: Vec::new()` を初期化。serialized field order は `schema_version → version → variables → root_component` (variables を root_component の前に置く理由: scope 的に Document 全域は root_component より広い概念のため)。既存 golden YAML テスト (`test_extruded_rect_yaml_golden` 等) は `variables` 省略時に出力されないため影響なし。

- `crates/engawa-format/src/feature.rs` — `CreateSketch` variant に `variables` 追加:

  **before**:
  ```rust
  CreateSketch {
      id: String,
      plane: SketchPlane,
      #[serde(default, skip_serializing_if = "is_zero")]
      offset: f64,
      profile: Vec<SketchSegment>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      plane_ref: Option<PlaneRef>,
  },
  ```
  **after**:
  ```rust
  CreateSketch {
      id: String,
      plane: SketchPlane,
      #[serde(default, skip_serializing_if = "is_zero")]
      offset: f64,
      #[serde(default, skip_serializing_if = "Vec::is_empty")]
      variables: Vec<Variable>,
      profile: Vec<SketchSegment>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      plane_ref: Option<PlaneRef>,
  },
  ```
  既存 inline test の `CreateSketch { ... }` 構築箇所 4 箇所 (golden tests) に `variables: Vec::new()` を追記する。golden YAML 上は `variables` 省略時に出力されないため文字列 golden は影響なし。

- `crates/engawa-format/src/error.rs` — Variable 系エラーは `EvalError` (variable.rs) で持つため `FormatError` に追加バリアントは不要 (`EvalError` は `#[from]` でも `FormatError` 経由でもなく独立した値返却型として運用する; `from_yaml` でパースする `Variable.expr` は文字列のままで eager 評価しない)。

### スコープ規約 (ADR-015 §2 準拠の実装上の解釈)

- Sketch 内 `Variable.expr` の `${a}` 解決順: (1) Sketch.variables 内の `a` (自己除外) → (2) Document.variables 内の `a` → どちらにも無ければ `UndefinedVariable`
- Sketch 内で同名 `a` を Document でも宣言している場合、Sketch.a の expr 中の `${a}` は **Document.a を見ない** (Sketch scope に同名がある = self-cycle になり `CircularDependency { cycle: ["a", "a"] }` を返す)。これは一般的な lexical scope の挙動 (TypeScript の `let a = a + 1` がエラーになるのと同質)。
- Sketch 内 b が Document.a を参照する場合 (Sketch には a が無い) → Document.a を解決して使う = shadowing の正しい用法

## 設計方針

- **決定性**: 評価結果は `indexmap::IndexMap<String, f64>` で返す。Kahn's topological sort は名前の lexicographic 順で tiebreak することで同一入力 → 同一順序を保証。値計算は f64 演算で IEEE 754 準拠 (同一 expr 列 → 同一 f64 bit pattern)。
- **derive 規約**: `Variable` は `Debug, Clone, Serialize, Deserialize, JsonSchema, TS` (既存 `SketchSegment` と同じ derive 群)。`EvalError` は `Debug, Clone, PartialEq, thiserror::Error` (テスト容易性のため `PartialEq` 付加)。
- **エラーハンドリング**: `EvalError` は `thiserror::Error` で `#[error("...")]` メッセージを各 variant に付与する。`FormatError` には添えない (parse 時に評価しないため)。
- **workspace.dependencies**: parser は自前。`indexmap` は既に workspace dep 済み。新規依存は追加しない。
- **B-rep / Euler-Poincaré / 退化幾何 / 数値モデル**: 本 Issue は format 層の Variable 評価のみで B-rep / 幾何不変条件に触れない → 該当なし (チェックリストは N/A)。

## テスト計画 (ID 付き)

すべて `crates/engawa-format/tests/variable_acceptance.rs` (新規 integration test) に置く。inline unit tests は `variable.rs` 内 `#[cfg(test)] mod tests` に置く。

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 Variable 列を 2 回 `evaluate_scope` → 同一 IndexMap (キー順含む) | `assert_eq!(result1, result2)` (順序付き比較) |
| T02 | 正常系 | Document のみ: `a=10`, `b="${a}*2"`, `c="${b}+${a}"` | `a=10.0, b=20.0, c=30.0`、評価順 `[a, b, c]` |
| T03 | 正常系 (shadowing) | Document `a=10`, Sketch `b="${a}+5"` (Sketch に a なし) | Sketch scope 評価結果 = `{a:10.0, b:15.0}` (Sketch.b は Document.a を参照) |
| T03b | 正常系 (shadowing 上書き) | Document `a=10`, Sketch `a=20`, Sketch `b="${a}+5"` | Sketch scope 評価結果中 `a=20.0, b=25.0` (Sketch.a が優先) |
| T_DEG_circular | 退化 | Document `a="${b}"`, `b="${a}"` | `Err(EvalError::CircularDependency { cycle })`、cycle は `["a","b","a"]` または `["b","a","b"]` のいずれか (lexicographic tiebreak で `["a","b","a"]` に決定) |
| T_DEG_circular_self | 退化 | Document `a="${a}"` | `Err(EvalError::CircularDependency { cycle: vec!["a","a"] })` |
| T_DEG_undefined | 退化 | Document `a="${missing}"` | `Err(EvalError::UndefinedVariable { name: "missing".into() })` |
| T_BOUNDARY_empty_expr | 境界 | Document `a=""` | `Err(EvalError::EmptyExpression { name: "a".into() })` |
| T_BOUNDARY_whitespace_expr | 境界 | Document `a="   "` (whitespace only) | `Err(EvalError::EmptyExpression { name: "a".into() })` |
| T05_arithmetic | 正常系 | Document `a="2+3*4"` (operator precedence) | `a=14.0` (掛算が加算より先) |
| T06_parens | 正常系 | Document `a="(2+3)*4"` | `a=20.0` |
| T07_unary_minus | 正常系 | Document `a="-5"`, `b="${a}*2"` | `a=-5.0, b=-10.0` |
| T08_division | 正常系 | Document `a="10/4"` | `a=2.5` (f64 除算、整数除算ではない) |
| T09_yaml_roundtrip | YAML | Document.variables + Sketch.variables 含む YAML を `to_yaml` → `from_yaml` → 再 `to_yaml` で同一文字列 | golden 文字列で `assert_eq!` |
| T10_empty_vars_omitted | YAML | `variables: []` を含む Document を `to_yaml` | YAML に `variables` フィールドが出力されない (skip_serializing_if) |
| T_DEG_sketch_self_shadow_self_ref | 退化 (lexical scope) | Document `a=10`, Sketch `a="${a}+1"` | `Err(EvalError::CircularDependency { cycle: vec!["a","a"] })` (Sketch.a の `${a}` は Document.a を参照せず self を見て循環) |
| T_DEG_parse_invalid | 退化 (parser) | Document `a="2 + + 3"` (不正トークン列) | `Err(EvalError::Parse { name: "a".into(), reason: ... })` |
| T_DEG_div_by_zero | 退化 (f64 IEEE) | Document `a="1/0"` | f64 IEEE 754 動作: `a` は `f64::INFINITY`、エラーにはしない (decision: division-by-zero 検出は本 Issue 範囲外、IEEE 数値演算を尊重) |

### 退化/境界ケース ID チェック

- `T_DEG_circular` / `T_DEG_circular_self` / `T_DEG_undefined` / `T_DEG_sketch_self_shadow_self_ref` / `T_DEG_parse_invalid` / `T_DEG_div_by_zero` — 退化系 6 件
- `T_BOUNDARY_empty_expr` / `T_BOUNDARY_whitespace_expr` — 境界系 2 件

両系列とも 1 件以上を満たし、STEP 5.5 grep が pass する。

### golden YAML (T09)

```yaml
schema_version: 1
version: 0.1.0
variables:
- name: width
  expr: '10.0'
- name: depth
  expr: ${width} * 2
root_component:
  name: Test
  features:
  - type: create_sketch
    id: sketch_1
    plane: xy
    variables:
    - name: w_local
      expr: ${width} + 5
    profile: []
```

(`'10.0'` と `${width} * 2` の quote の差は serde_yaml のヒューリスティクスに従う; 実テストでは `to_yaml` の出力をそのまま採用する)

## 幾何的不変条件チェックリスト

本 Issue は engawa-format 層の数式評価のみで B-rep / Boolean / Partition / Assemble に触れないため、すべて **N/A**。

- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか → N/A
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか → N/A
- [ ] flip_normals / same_sense の意味論が明確か → N/A
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか → N/A
