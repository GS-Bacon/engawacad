# Debug Spec for #274 — Codex round 1 fixup

## 背景

STEP 7.5 Codex 3 persona 並列レビューで blocking=6 (high)。Codex usage limit に当たり stale cache 経由の指摘だが、内容は実体ある重要事項で ADR-017 §4 / 数値モデル §退化判定 違反を含む。本 fixup で全 high を解消する。

## 修正項目

### F01 [high]: CURRENT_SCHEMA_VERSION 1 → 2 + v1→v2 migration hook (C-F02 / M-F01)

ADR-017 §4 mandate。`SketchElement` に Ellipse/Conic variant 追加 = wire-format 変更 = `CURRENT_SCHEMA_VERSION` を 2 に bump 必須。

**修正方針**:

- `crates/engawa-format/src/document.rs` の `CURRENT_SCHEMA_VERSION: u32 = 1` → `= 2`
- `default_schema_version()` は `1` 据え置き (legacy file = v1 扱い)
- `Document::from_yaml` (または近傍の loader) で `schema_version == 1` の場合に migration hook `migrate_v1_to_v2()` を起動
- migration hook の役割: `profile` 内の各要素が `kind:` フィールド未持ち (= legacy untagged Line) なら `kind: line` を補完する pure transformation
  - 既に Deserialize 側 (`SketchElement::deserialize` の legacy LegacyLine fallback) で透過処理してあるため、追加処理は最小限で済む可能性
  - `kind:` 既存なら no-op
- 既存 example YAML (`examples/*.engawa` で `kind:` を使うすべての sketch profile) は **書き換えなしで読める** ことを smoke test で確認
- `examples/ellipse_conic.engawa` を `schema_version: 2` に bump (新規 v2 wire format を使うため)
- writer は常に v2 (`CURRENT_SCHEMA_VERSION` = 2) で出力

**追加テスト**:

- `crates/engawa-format/src/document.rs` 内 inline test:
  - `t_migrate_v1_legacy_line_passes`: legacy untagged Line のみの v1 YAML が parse 通過 + 内部表現は `SketchElement::Line` (migration hook ルート)
  - `t_v2_schema_version_round_trip`: `schema_version: 2` を serialize/deserialize で保存
  - `t_ambig_schema_v2_with_legacy_line_rejected`: `schema_version: 2` だが `kind:` 無し → `LoadError::AmbiguousSchema` で fail (ADR-017 §4 reject 戦略)

### F02 [high]: Conic 実点存在検証 (A-F02 / C-F01 / M-F02)

`coeffs = [-1, 0, -1, 0, 0]` のような **実点を持たない conic** (`-x² - y² = 1` = 空集合) で sqrt(-fp/lambda) が NaN を返す。

**修正方針** (`crates/engawa-kernel/src/tessellation/sketch.rs` の Conic case):

- canonical 化後 (λ1, λ2, F' = `fp` を得たあと) に以下を順序判定:
  1. λ1, λ2, fp の有限性検証 (`!is_finite()` なら `DegenerateSketchElement { reason: "non-finite canonical form" }`)
  2. ellipse case (discriminant < 0):
     - `-fp / λ1` および `-fp / λ2` が両方 **> 0** であることを確認 (= 半軸長が実数)
     - 1 つでも非正なら "non-real ellipse conic (no real points)" として reject
  3. hyperbola case (discriminant > 0):
     - λ1, λ2 が異符号で `|fp / λ1|` `|fp / λ2|` ともに > 0 (有限) なら OK
     - `fp == 0` (中心点 conic) は既存 "degenerate point conic" gate でカバー
  4. サンプリング後の polyline 全点で `is_finite()` を assert (panic ではなく `DegenerateSketchElement` で fail)

**追加テスト** (`#[cfg(test)] mod tests` in sketch.rs):

- `t_degenerate_conic_no_real_points_ellipse`: `coeffs=[-1, 0, -1, 0, 0]` → `Err(DegenerateSketchElement { reason: "non-real conic (no real points)" })`
- `t_degenerate_conic_imaginary_branch`: `coeffs=[1, 0, 1, 0, 0]` (= `x² + y² = -1` 不可能) → 同様 reject

### F03 [high]: Ellipse center NaN/Inf 検証 (C-F01 / M-F02)

`Ellipse { center: [f64::NAN, 0.0], ... }` でも現状 `Ok(Vec<[f64;2]>)` を返してしまう。

**修正方針** (sketch.rs の Ellipse case):

- 退化検査の最先頭に追加:
  ```rust
  if !center[0].is_finite() || !center[1].is_finite() {
      return Err(DegenerateSketchElement { reason: "non-finite center" });
  }
  ```
- 同じ修正を Circle / Arc 各 variant にも一貫適用 (既存 #273 では center NaN を見ていない可能性 → defensive guard)

**追加テスト**:

- `t_degenerate_ellipse_nan_center`: `center=[f64::NAN, 0.0]` → `Err(DegenerateSketchElement { reason: "non-finite center" })`
- `t_degenerate_ellipse_inf_center`: `center=[f64::INFINITY, 0.0]` → 同様 reject

### F04 [high]: Ellipse の major >= minor invariant enforcement (A-F03 / Claude self-review CSR-A01)

ADR-017 §1 表 "Requires major >= minor > 0" を実装で enforce する。

**修正方針** (sketch.rs の Ellipse case):

- 退化検査に追加 (axis_ratio 判定の直前):
  ```rust
  if *minor > *major {
      return Err(DegenerateSketchElement { reason: "minor > major (axis order invariant)" });
  }
  ```

**追加テスト**:

- `t_degenerate_ellipse_minor_gt_major`: `major=1.0, minor=2.0` → `Err(DegenerateSketchElement { reason: "minor > major (axis order invariant)" })`

### F05 [high]: validate_profile_closed が open Conic を素通り (A-F01 / C-F03)

`validate_profile_closed` (`crates/engawa-build/src/lib.rs:553`) は all-Line のときだけ閉路検証し、それ以外は素通し。新規 hyperbola conic は開曲線なので extrude profile としては不正だが現状受理。

**修正方針**:

- profile 内に **hyperbola conic (= discriminant > 0)** が含まれているかを判定し、含まれていれば `InvalidParameter { kind: "profile" }` を返す
  - 別解: 全 Conic variant を一律 "profile から弾く" — 楕円 conic も extrude 不可になるため過剰。hyperbola のみ block する方が ADR-017 §1 と整合
- 判定ロジック: `tessellate_sketch_element(elem, 1)` を試して error 経由でなく conic 種別判定するのは過剰 → discriminant 計算を `validate_profile_closed` 内で行う (もしくは `engawa-kernel` に `is_open_curve(elem) -> bool` を提供して呼ぶ)
- 実装は `engawa-kernel` 側に `pub fn sketch_element_is_open(elem: &SketchElement) -> bool` を提供して engawa-build からは呼び出すだけにする方が責務分離が綺麗

**追加テスト**:

- `engawa-build/tests/` または inline:
  - `t_validate_profile_closed_rejects_hyperbola`: profile に hyperbola conic 単独 → `Err(InvalidParameter { kind: "profile" })`
  - `t_validate_profile_closed_accepts_ellipse_conic`: profile に楕円 conic 単独 → OK

## 修正後の検証

- `cargo xtask ci` green (workspace test + clippy + fmt + TS drift + bun test)
- 既存 examples_smoke の `ellipse_conic` テストが pass (= schema_version: 2 に更新後も build 成功)
- 既存の他 example YAML (`circle_arc.engawa` 等) が schema_version: 1 のままで parse 成功 (migration hook 経路)

## scope 外 (deferred)

- CSR-C01 (medium): Conic 固有値分解の `A ≈ C, B ≈ 0` cancellation → Phase 11+ adaptive sampling 時に再評価
- CSR-A02 (low): non-finite reason string の精度向上 → cosmetic、本 fixup では major/minor 用に "non-finite major/minor" を分けない
- NAM01 (low): t_boundary_circle_degeneracy 改名 → 本 fixup では触らない

## Round 2 追記 (GLM dispatch #1 の clippy 残り)

### F06 [trivial]: clippy::collapsible_if 2 件解消

`crates/engawa-format/src/document.rs:112` と `:159` の `if doc.schema_version < CURRENT_SCHEMA_VERSION { if doc.schema_version == 1 { ... } }` を `if doc.schema_version < CURRENT_SCHEMA_VERSION && doc.schema_version == 1 { ... }` に collapse する。

```rust
// before (clippy error):
if doc.schema_version < CURRENT_SCHEMA_VERSION {
    if doc.schema_version == 1 {
        doc = migrate_v1_to_v2(doc);
    }
}

// after (clippy clean):
if doc.schema_version < CURRENT_SCHEMA_VERSION && doc.schema_version == 1 {
    doc = migrate_v1_to_v2(doc);
}
```

同じ修正を `:112` (from_yaml) と `:159` (近傍の同パターン) の両方に適用すること。

### F07 [maintenance]: 試した修正と結果

- Round 1 GLM dispatch (debug_spec_used=true): F01-F05 を実装したが F06 (clippy collapsible_if) で CI 赤。glm-result-r2.json status=failed, error_pattern="error: this `if` statement can be collapsed"
- Round 2 で F06 のみ追加 fixup する

## Round 3 追記 (GLM dispatch #2 で生じた test 失敗 3 件)

### F08 [test]: test_schema_version_backward_compat 更新

`crates/engawa-format/src/document.rs:t_schema_version_backward_compat` 修正。

旧:
```rust
let doc = Document::from_yaml(old_yaml).expect("old yaml should parse");
assert_eq!(doc.schema_version, 1, "missing schema_version defaults to 1");
let yaml = doc.to_yaml().unwrap();
assert!(yaml.starts_with("schema_version: 1\n"), ...);
```

新 (migration を経た doc は v2 表現になる):
```rust
let doc = Document::from_yaml(old_yaml).expect("old yaml should parse");
assert_eq!(doc.schema_version, 2, "v1 legacy is migrated to v2 on load (ADR-017 §4)");
let yaml = doc.to_yaml().unwrap();
assert!(yaml.starts_with("schema_version: 2\n"), "writer outputs current schema_version");
```

### F09 [test]: test_extruded_rect_yaml_golden 更新

`crates/engawa-format/src/document.rs:t_extruded_rect_yaml_golden` の GOLDEN 文字列を `schema_version: 1` から `schema_version: 2` に変更:

```rust
static GOLDEN: &str = concat!(
    "schema_version: 2\nversion: 0.1.0\nroot_component:\n  ...", // 先頭のみ変更
    ...残りはそのまま
);
```

### F10 [test]: test_all_example_files_have_schema_version 更新

`crates/engawa-format/src/document.rs:t_all_example_files_have_schema_version` を「v1 or v2 を許容」に変更:

```rust
assert!(
    content.starts_with("schema_version: 1\n") || content.starts_with("schema_version: 2\n"),
    "{} should start with schema_version: 1 or 2 (post-ADR-017)",
    path.display()
);
```

理由: 既存 example YAML は schema_version: 1 のまま (loader migration 経由)、新規 `ellipse_conic.engawa` は schema_version: 2 (新 variant 使用) — 両方が混在する有効状態。

### F11 [test]: 既存テスト無効化禁止 (再確認)

debug-spec 適用時は **既存テストの assertion 緩和は行うが、test 自体の削除や #[ignore] は行わない**。文脈不明な場合は問い合わせよりも assertion を ADR-017 §4 仕様に合わせる方を優先する。

## Round 4 追記 (F09 / F10 修正漏れ)

GLM #3 dispatch で F08 は通ったが F09 / F10 が未修正のため再指示。場所は明確で他に修正不要、テストアサーション 2 箇所だけを丁寧に変更すること。

### F09b [test]: extruded_rect_yaml_golden の GOLDEN 文字列先頭を "schema_version: 1" → "schema_version: 2"

`crates/engawa-format/src/document.rs:464` 付近の `static GOLDEN: &str = concat!(...);` の **最初の concat! 引数文字列**を以下のとおり修正:

```rust
// before:
static GOLDEN: &str = concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Extruded Rect\n  features:\n",
    ...
);

// after:
static GOLDEN: &str = concat!(
    "schema_version: 2\nversion: 0.1.0\nroot_component:\n  name: Extruded Rect\n  features:\n",
    ...
);
```

それ以外の引数行は完全に変更しないこと (中身は line variant のシリアライズ結果なので正しい)。

### F10b [test]: test_all_example_files_have_schema_version を v1 or v2 許容に変更

`crates/engawa-format/src/document.rs:635-650` 付近の test 内 assert を以下のとおり修正:

```rust
// before:
let content = std::fs::read_to_string(&path).unwrap();
assert!(
    content.starts_with("schema_version: 1\n"),
    "{} should start with schema_version: 1",
    path.file_name().unwrap().to_string_lossy()
);
let doc = Document::from_path(&path).unwrap();
assert_eq!(
    doc.schema_version,
    1,
    "{} should have schema_version 1",
    path.file_name().unwrap().to_string_lossy()
);

// after:
let content = std::fs::read_to_string(&path).unwrap();
assert!(
    content.starts_with("schema_version: 1\n") || content.starts_with("schema_version: 2\n"),
    "{} should start with schema_version: 1 or 2 (post-ADR-017)",
    path.file_name().unwrap().to_string_lossy()
);
let doc = Document::from_path(&path).unwrap();
assert!(
    doc.schema_version == 1 || doc.schema_version == 2,
    "{} should have schema_version 1 or 2 (post-ADR-017)",
    path.file_name().unwrap().to_string_lossy()
);
```

注意: `doc.schema_version` の assertion を `assert_eq!` から `assert!` に変えること (== 2 つの値の比較になるため)。

### 進捗管理

F09b と F10b の 2 箇所のみ修正、それ以外は一切触らない。実装後は `cargo xtask ci` で green を確認すること。

## Round 5 追記 (Codex r2 6 件 high)

### F12 [high]: 平行移動済み hyperbola の主軸符号誤り (A-F01 / C-F01 / M-F01)

現実装は `discriminant > 0` 分岐で `|fp| / |λ_i|` の絶対値を使って `(a cosh t, b sinh t)` を常に λ1 側に載せている。これは `F' > 0` (= 平行移動した hyperbola で D/E ≠ 0) のケースで実軸の取り方を間違える。

**例**: `coeffs=[1, 0, -1, 0, 4]` (= `x² - y² + 4y = 1` → `(y-2)² - x² = 3` を `F=-1` 正規化)
- A=1, B=0, C=-1, D=0, E=4, F=-1
- M = [[1,0],[0,-1]] → λ1=1, λ2=-1
- center: `t = -M⁻¹ [D/2, E/2] = -[[1,0],[0,-1]]⁻¹ [0, 2] = -[0, -2] = [0, 2]`
- F' = F + (D/2, E/2) · t = -1 + (0,2)·(0,2) = -1 + 4 = 3 > 0
- canonical: 1·X² + (-1)·Y² + 3 = 0 → X² - Y² = -3 → Y² - X² = 3 (real axis = Y)

現実装は X² 側に `cosh t` を載せて全点が `Y² - X² ≠ 3` で曲線外になる。

**修正方針** (`crates/engawa-kernel/src/tessellation/sketch.rs` hyperbola 分岐):

```rust
// 修正の中核ロジック (擬似コード):
// canonical form: lambda1*X² + lambda2*Y² + fp = 0
// → 実軸 = -fp/lambda_i > 0 となる i (= cosh を載せる方向)
// → 虚軸 = -fp/lambda_j < 0 となる j (= sinh を載せる方向)
let r1 = -fp / lambda1;  // X² 係数
let r2 = -fp / lambda2;  // Y² 係数
// hyperbola なら必ず r1, r2 異符号
let (a_sq, b_sq, swap_axes) = if r1 > 0.0 && r2 < 0.0 {
    (r1, -r2, false)  // X 軸に cosh、Y 軸に sinh
} else if r1 < 0.0 && r2 > 0.0 {
    (r2, -r1, true)   // Y 軸に cosh、X 軸に sinh
} else {
    return Err(DegenerateSketchElement { reason: "non-real hyperbola (no real points)" });
};
let semi_a = a_sq.sqrt();
let semi_b = b_sq.sqrt();
// パラメトリックサンプル
let n = base_segments * 2;
for i in 0..n {
    let t = -2.0 + 4.0 * (i as f64) / ((n - 1) as f64);
    let (canon_x, canon_y) = if !swap_axes {
        (semi_a * t.cosh(), semi_b * t.sinh())
    } else {
        (semi_b * t.sinh(), semi_a * t.cosh())
    };
    // 主軸基底に持ち上げ + 平行移動 ...
}
```

**追加テスト** (`#[cfg(test)] mod tests` in sketch.rs):

- `t_hyperbola_translated_x_axis`: `coeffs=[-1.0/3.0, 0.0, 1.0/3.0, 4.0/3.0, 0.0]` (= `(x-2)² - y² = 3` 系) → 全点が `|A x² + B xy + C y² + D x + E y - 1| < LENGTH_TOLERANCE * 100` を満たす
- `t_hyperbola_translated_y_axis`: `coeffs=[1.0, 0.0, -1.0, 0.0, 4.0]` (= `(y-2)² - x² = 3` 系) → 同様 conic 評価誤差 OK

### F13 [high]: SketchElement::deserialize の legacy fallback を v1 migration 限定にする (C-F02 / M-F02)

現実装は `SketchElement::deserialize` が `kind:` 無しの YAML を無条件に Line にする。`schema_version: 2` 文書でも `kind:` 不在を許してしまい、wire-format gate が機能しない。

**修正方針**:

オプション A (推奨): SketchElement::deserialize の legacy fallback を残しつつ、Document::from_yaml で v2 文書を読む前段で `kind:` 必須 check を入れる。これにより SketchElement 単体テストでの legacy parse は維持しつつ、Document 経由では schema gate が効く。

```rust
// crates/engawa-format/src/document.rs:from_yaml 内 (validate() 直前):
if doc.schema_version >= 2 {
    // v2 では Document 階層を再 traverse して全 SketchElement profile に kind:
    // 必須 check を入れる。但し SketchElement::deserialize はすでに通過しているため、
    // ここでは "kind: line で復元された Line を許容するか" の判定は不要 (= deserialize 後の
    // SketchElement は kind: が明示的に書かれていたかどうか痕跡を残せない)。
    // 代替案: 一度生 YAML を peek して、profile 内に kind: 無い要素があれば AmbiguousSchema 返す
    let raw_value: serde_yaml::Value = serde_yaml::from_str(yaml).map_err(...)?;
    // raw_value から root_component.features[].profile を traverse、kind: 不在を検出
    check_v2_kind_required(&raw_value)?;
}
```

オプション B (シンプル): SketchElement の Tagged enum deserialize を default にして LegacyLine fallback を削除。v1 文書は Document::migrate_v1_to_v2 で `kind: line` を YAML に補完してから serde に渡す。

**判定**: オプション B の方が責務が綺麗 (deserialize は wire format に従う only)。v1 migration は YAML 文字列レベルで `kind: line` を補完する pre-pass にする。

**修正実装** (オプション B 採用):

1. `crates/engawa-format/src/feature.rs` の `SketchElement::deserialize` 中 LegacyLine fallback ブロックを **削除** (= `kind:` 無しは serde error)
2. `crates/engawa-format/src/document.rs` の `migrate_v1_to_v2(doc: Document) -> Document` は今のように Document を受け取れない (= 既に deserialize 済みのため失敗してしまう)。代わりに **YAML 文字列レベルで legacy `from`/`to` のみの sketch element を `kind: line` 付きに rewrite する pre-pass** を `Document::from_yaml` 冒頭で実施する

```rust
// crates/engawa-format/src/document.rs:from_yaml 冒頭
pub fn from_yaml(yaml: &str) -> Result<Self, FormatError> {
    // 1. schema_version を peek
    let stage1: serde_yaml::Value = serde_yaml::from_str(yaml).map_err(...)?;
    let sv = stage1.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    // 2. v1 のときだけ legacy untagged Line を `kind: line` 付きに rewrite した YAML を再生成
    let processed = if sv == 1 {
        rewrite_v1_legacy_lines_to_tagged(stage1)
    } else {
        stage1
    };
    let yaml_processed = serde_yaml::to_string(&processed).map_err(...)?;
    // 3. 正式 deserialize
    let mut doc: Document = serde_yaml::from_str(&yaml_processed).map_err(...)?;
    // ... 既存の ref_planes 補完 / validate ...
    Ok(doc)
}

fn rewrite_v1_legacy_lines_to_tagged(mut value: serde_yaml::Value) -> serde_yaml::Value {
    // root_component.features[].profile[] を traverse
    // 各要素が Mapping で `id` + `from` + `to` を持ち `kind` が無いなら `kind: "line"` を挿入
    ...
}
```

**追加テスト**:

- `t_v1_legacy_line_migrated_via_yaml_prepass`: `schema_version: 1` + 旧 untagged Line YAML → from_yaml 成功、内部表現は `SketchElement::Line { id, from, to }`
- `t_v2_kind_missing_rejected`: `schema_version: 2` + `kind:` 無し sketch element → `FormatError::Parse` で fail
- `t_v2_kind_explicit_line_accepted`: `schema_version: 2` + `kind: line` 明示 sketch element → 成功

### F14 [high]: acceptance skeleton を削除 (M-F03)

`crates/engawa-kernel/tests/ellipse_conic_acceptance.rs` の 14 件全 `#[ignore]` + `todo!()` を **ファイルごと削除** する。inline tests (sketch.rs / feature.rs / examples_smoke.rs) で plan T ID 全カバー済のため redundancy。

代替案として inline tests のいくつかを acceptance に転記する案もあるが、責務重複と保守コスト増のため削除を採用。

### F15 [medium]: ellipse_conic smoke を body-producing fixture に変更 (C-F03)

現状 `examples/ellipse_conic.engawa` は `create_sketch` のみで `extrude` が無いため `build_bodies_from_features` が tessellation 経路に入らない。本来 smoke は build 完了まで触るのが目的なので fixture を拡張する。

**修正方針** (`examples/ellipse_conic.engawa`):

```yaml
schema_version: 2
# Issue #274: Phase 10 Ellipse / Conic sketch profile smoke test.
version: "0.1.0"
root_component:
  name: "Ellipse Conic Test"
  features:
    - type: create_sketch
      id: sketch_1
      plane: xy
      profile:
        - kind: ellipse
          id: ellipse_1
          center: [0.0, 0.0]
          major: 2.0
          minor: 1.0
          rotation: 0.0
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 5.0
```

(Conic は不正多重 profile 経路で extrude_close 検証に引っかかるため smoke 用 fixture は ellipse 単独に絞る。Conic は inline tests で十分カバー。)

### 進捗管理

F12 (translated hyperbola) / F13 (legacy fallback) / F14 (acceptance 削除) / F15 (smoke fixture) を順に修正、他に手を出さない。実装後 `cargo xtask ci` で green を確認すること。

## Round 6 追記 (F13 YAML pre-pass バグ)

### F16 [critical bug]: rewrite_v1_legacy_lines_to_tagged が `type` 判定を間違えている

`crates/engawa-format/src/document.rs:recurse_component` で feature の `type` を `Y::Mapping` (= externally-tagged enum) と仮定しているが、Document は `#[serde(tag = "type")]` の **internally-tagged** 表現で、`type` の値は `Y::String("create_sketch")`。

```rust
// before (バグ):
if let Some(Y::Mapping(type_kv)) = fmap.get(Y::String("type".into())) {
    if type_kv.get(Y::String("create_sketch".into())).is_some() {
        if let Some(profile) = fmap.get_mut(Y::String("profile".into())) {
            rewrite_profile_elements(profile);
        }
    }
}

// after (正):
let is_create_sketch = matches!(
    fmap.get(Y::String("type".into())),
    Some(Y::String(t)) if t == "create_sketch"
);
if is_create_sketch {
    if let Some(profile) = fmap.get_mut(Y::String("profile".into())) {
        rewrite_profile_elements(profile);
    }
}
```

または、より頑健に「profile キーがあれば常に rewrite_profile_elements を試す」(他 feature variant に profile が無いため副作用なし):

```rust
if let Some(profile) = fmap.get_mut(Y::String("profile".into())) {
    rewrite_profile_elements(profile);
}
```

後者を採用。これで `type` 表現に依存しない (= 将来 enum tag 名を変えても影響なし)。

これだけで全 sketch_*.engawa の YAML pre-pass が機能して legacy untagged Line が `kind: line` に bump される。`cargo xtask ci` で smoke 全 pass を期待。

### 進捗管理

F16 のみ修正、それ以外は触らない。

## Round 7 追記 (F13 partial revert — 実用 v2)

### F17 [revert/required]: SketchElement::deserialize の LegacyLine fallback を復元

F13 の strict v2 gate は API (axum Json extractor) / Playwright e2e / 既存 examples の広範囲を壊した。Codex C-F02 / M-F02 の指摘 (v2 で kind 必須) は legitimate だが、現状の consumer 互換を保つには premature。

**revert する変更**:

1. `crates/engawa-format/src/feature.rs` の `SketchElement::deserialize` に **LegacyLine fallback を復元** する (F13 で削除した部分):

```rust
impl<'de> Deserialize<'de> for SketchElement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        // 既存 Tagged enum (Line/Circle/Arc/Ellipse/Conic) deserialize ロジックはそのまま
        // ...
        // Fallback (復元): legacy Line (id, from, to without "kind")
        #[derive(Deserialize)]
        struct LegacyLine {
            id: String,
            from: [f64; 2],
            to: [f64; 2],
        }
        let legacy: LegacyLine = serde_yaml::from_value(value)
            .map_err(|e| Error::custom(format!("invalid SketchElement: {e}")))?;
        Ok(SketchElement::Line { id: legacy.id, from: legacy.from, to: legacy.to })
    }
}
```

2. `crates/engawa-format/src/document.rs` の YAML pre-pass `rewrite_v1_legacy_lines_to_tagged` および `from_yaml` 内の呼び出しを **削除** する (= 元の simple structure に戻す):

```rust
// before (F13 で追加された pre-pass):
let processed_value = if peeked_version == 1 {
    rewrite_v1_legacy_lines_to_tagged(value)
} else {
    value
};
let yaml_processed = serde_yaml::to_string(&processed_value).map_err(FormatError::Yaml)?;
let raw: RawDocument = serde_yaml::from_str(&yaml_processed)?;

// after (revert):
let raw: RawDocument = serde_yaml::from_value(value).map_err(FormatError::Yaml)?;
```

ただし schema_version peek + UnknownSchemaVersion ガードは維持 (`CURRENT_SCHEMA_VERSION = 2` 違反検出のため):

```rust
let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
let peeked_version = value
    .get("schema_version")
    .and_then(|v| v.as_u64())
    .map(|v| v as u32)
    .unwrap_or(INITIAL_SCHEMA_VERSION);
if peeked_version > CURRENT_SCHEMA_VERSION {
    return Err(FormatError::UnknownSchemaVersion {
        found: peeked_version,
        current: CURRENT_SCHEMA_VERSION,
    });
}
let raw: RawDocument = serde_yaml::from_value(value).map_err(FormatError::Yaml)?;
```

3. `migrate_v1_to_v2(doc)` 関数は **maintain** (schema_version: 2 への bump 経由を保つ。実装は Document の schema_version field を 2 にセットするだけの no-op に近い)。

4. `rewrite_v1_legacy_lines_to_tagged` ヘルパ関数を **完全削除** (= 不要)。

5. examples_smoke.rs の `Document::from_yaml` 利用は **維持** (= migration hook 経由で schema_version: 1 が parse 可能、ただし LegacyLine fallback も働く)。

### F18 [test]: F13 で追加した legacy fallback 関連テストの削除/緩和

`crates/engawa-format/src/document.rs` 内の以下テストを削除または assertion 緩和:

- `t_v1_legacy_line_migrated_via_yaml_prepass` (削除、pre-pass が無くなったため)
- `t_v2_kind_missing_rejected` (削除、strict gate が無いため)
- `t_v2_kind_explicit_line_accepted` (削除、自明な assertion)

`SketchElement::deserialize` 用 inline test で LegacyLine fallback が動くこと (`t_legacy_untagged_line_falls_back_to_line`) を確認できる test がなければ追加:

```rust
#[test]
fn t_legacy_untagged_line_falls_back_to_line() {
    let yaml = "id: l1\nfrom: [0.0, 0.0]\nto: [1.0, 0.0]\n";
    let elem: SketchElement = serde_yaml::from_str(yaml).unwrap();
    assert!(matches!(elem, SketchElement::Line { id, .. } if id == "l1"));
}
```

### F19 [doc]: Codex C-F02 / M-F02 への返答を rejection.md / 注釈に記録

`features/274-phase10-conic-ellipse-conic/rejection.md` に Codex の v2 strict gate 指摘を **本 Issue では partial accept** とする旨を追記:

- accept した部分: schema_version v1 → v2 bump、example/ellipse_conic.engawa を v2 で保存
- reject した部分: `kind:` 必須 strict gate — API/e2e 互換性を壊すため本 Issue では deferral。Phase 11 以降で wire-format strictness が必要になったタイミングで再評価 (別 ADR 起票候補)

### 進捗管理

F17 (deserialize fallback 復元 + pre-pass 削除) と F18 (関連テスト調整) のみ実施。F19 は rejection.md への追記のみで code 変更なし。

## Round 8 追記 (Codex r3 残り 4 件のうち 3 件 quick fix)

### F20 [medium]: Conic を ellipse_conic.engawa fixture に追加 (C-F02)

`examples/ellipse_conic.engawa` に既存 Ellipse profile に加えて Conic profile を含む別 sketch + extrude を追加し、Conic も engawa-format → build 経路を通すようにする。

```yaml
schema_version: 2
# Issue #274: Phase 10 Ellipse / Conic sketch profile smoke test.
version: "0.1.0"
root_component:
  name: "Ellipse Conic Test"
  features:
    - type: create_sketch
      id: sketch_ellipse
      plane: xy
      profile:
        - kind: ellipse
          id: ellipse_1
          center: [0.0, 0.0]
          major: 2.0
          minor: 1.0
          rotation: 0.0
    - type: extrude
      id: extrude_ellipse
      sketch: sketch_ellipse
      depth: 5.0
    - type: create_sketch
      id: sketch_conic
      plane: xy
      offset: 10.0
      profile:
        - kind: conic
          id: conic_1
          coeffs: [1.0, 0.0, 4.0, 0.0, 0.0]
    - type: extrude
      id: extrude_conic
      sketch: sketch_conic
      depth: 5.0
```

(Conic は `x² + 4y² = 1` 楕円 conic = 閉曲線 = extrude 可能)

### F21 [medium]: Ellipse/Conic で base_segments の下限保証 (C-F03)

`crates/engawa-kernel/src/tessellation/sketch.rs` の Ellipse / Conic 分岐で、`base_segments` を `arc_segment_count(0.0, std::f64::consts::TAU, base_segments).max(1)` または直接 `base_segments.max(1)` で下限保証する。Circle/Arc と挙動を揃える。

**追加テスト**:
- `t_boundary_ellipse_zero_segments`: `base_segments=0` でも `Ok(pts)` で `pts.len() >= 1` (= 既存 Circle と同じ契約)
- `t_boundary_conic_zero_segments`: 同様

### F22 [high]: Circle/Arc の NaN/Inf 検証 (A-F01)

`crates/engawa-kernel/src/tessellation/sketch.rs` の **Circle / Arc 分岐**にも Ellipse と同等の有限性検証を追加 (元 #273 起票時の検証漏れ):

```rust
SketchElement::Circle { id, center, radius } => {
    if !center[0].is_finite() || !center[1].is_finite() {
        return Err(DegenerateSketchElement { reason: "non-finite center" });
    }
    if !radius.is_finite() {
        return Err(DegenerateSketchElement { reason: "non-finite radius" });
    }
    if *radius < LENGTH_TOLERANCE {
        return Err(DegenerateSketchElement { reason: "radius < ε_radius" });
    }
    // 既存 tessellation ロジックは保持
}

SketchElement::Arc { id, center, radius, start_angle, end_angle } => {
    if !center[0].is_finite() || !center[1].is_finite() {
        return Err(DegenerateSketchElement { reason: "non-finite center" });
    }
    if !radius.is_finite() || !start_angle.is_finite() || !end_angle.is_finite() {
        return Err(DegenerateSketchElement { reason: "non-finite radius/angle" });
    }
    if *radius < LENGTH_TOLERANCE {
        return Err(DegenerateSketchElement { reason: "radius < ε_radius" });
    }
    let sweep = end_angle - start_angle;
    if sweep.abs() < ANGLE_TOLERANCE {
        return Err(DegenerateSketchElement { reason: "|end_angle - start_angle| < ε_angle" });
    }
    // 既存 tessellation ロジックは保持
}
```

**注意**: 既存の `t_edge_nan_radius_no_panic` / `t_edge_inf_radius_succeeds` テストは「NaN/Inf でも panic しないこと」を主張していたが、今回の変更で **DegenerateSketchElement を期待する** ように反転させる:

```rust
#[test]
fn t_edge_nan_radius_no_panic() {
    let circle = SketchElement::Circle { id: "c".into(), center: [0.0, 0.0], radius: f64::NAN };
    let err = tessellate_sketch_element(&circle, 32).unwrap_err();
    assert!(matches!(err, KernelError::DegenerateSketchElement { reason: r, .. } if r.contains("non-finite")));
}

#[test]
fn t_edge_inf_radius_succeeds() {
    // 名前は誤解を招くので意味的に反転: Inf radius は DegenerateSketchElement で reject
    let circle = SketchElement::Circle { id: "c".into(), center: [0.0, 0.0], radius: f64::INFINITY };
    let err = tessellate_sketch_element(&circle, 32).unwrap_err();
    assert!(matches!(err, KernelError::DegenerateSketchElement { .. }));
}
```

### F23 [defer]: C-F01 strict v2 wire-format gate は本 Issue では deferred (F19 と同方針)

`SketchElement::deserialize` で `schema_version: 2` 文書でも kind 欠落要素を Line にする件 (C-F01) は本 Issue で revert したため意図通り。Codex は再度同指摘するが、本 Issue の rejection.md に記録した方針 (= API/e2e 互換性優先で deferred、Phase 11+ で再評価) を維持する。

### 進捗管理

F20 + F21 + F22 を実装、F23 は code 変更なし (rejection.md の追記のみ)。実装後 `cargo xtask ci` で green 確認。

## Round 9 追記 (Codex r4 残り 4 件のうち M-F01/A-F02/M-F02 final fix)

### F24 [high regression]: validate_profile_closed の hyperbola 拒否を CreateSketch から Extrude に移動 (M-F01)

F05 で `validate_profile_closed` に hyperbola Conic 拒否を追加したが、これは `CreateSketch` 共通検証であるため standalone sketch (extrude しない sketch) まで保存・読込不可になり regression。

**修正方針** (`crates/engawa-build/src/lib.rs`):

1. `validate_profile_closed` から hyperbola 検出ロジックを **削除** (= 元の all-Line ケースのみ閉路検証する形に revert)
2. `Extrude` / `ExtrudeCut` の処理経路で profile を消費する直前に **open-curve 拒否**チェックを追加 (例: hyperbola Conic を含む profile を extrude しようとしたら error)
3. `engawa-kernel` に既存提供されている `sketch_element_is_open(elem: &SketchElement) -> bool` (= hyperbola discriminant > 0 で true) を使う

```rust
// crates/engawa-build/src/lib.rs Feature::Extrude / Feature::ExtrudeCut の処理冒頭:
// (ロケーションは GLM が grep して特定すること)
for elem in profile {
    if engawa_kernel::tessellation::sketch::sketch_element_is_open(elem) {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }
}
```

これにより standalone hyperbola sketch (= extrude しない) は保存・読込可能、extrude 時のみ拒否される。

### F25 [medium]: Conic 楕円分岐に axis_ratio degeneracy check 追加 (A-F02)

`crates/engawa-kernel/src/tessellation/sketch.rs` の Conic ellipse case (`discriminant < 0`) で `semi_a` / `semi_b` を求めた後に Ellipse と同じ axis_ratio 検証:

```rust
// Conic ellipse case の semi_a/semi_b 算出後:
let (semi_min, semi_max) = if semi_a <= semi_b { (semi_a, semi_b) } else { (semi_b, semi_a) };
if semi_min < LENGTH_TOLERANCE {
    return Err(DegenerateSketchElement { reason: "conic semi_min < ε_radius" });
}
if semi_min / semi_max < EPS_AXIS_RATIO {
    return Err(DegenerateSketchElement { reason: "conic semi_min/semi_max < ε_axis_ratio" });
}
// 既存 sampling ロジックへ続行
```

**追加テスト**:
- `t_degenerate_conic_axis_ratio`: `coeffs=[1.0, 0.0, 1e14, 0.0, 0.0]` (= ほぼ y=0 直線) → `Err(DegenerateSketchElement { reason: "conic semi_min/semi_max < ε_axis_ratio" })`

### F26 [medium]: v1 untagged line 回帰テスト追加 (M-F02)

`crates/engawa-format/src/document.rs` 内 inline test に追加 (もしくは `crates/engawa-format/tests/` integration test として):

```rust
#[test]
fn t_v1_untagged_line_round_trip_via_from_yaml() {
    let v1_yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "v1 legacy"
  features:
    - type: create_sketch
      id: sketch_1
      plane: xy
      profile:
        - id: seg_a
          from: [0.0, 0.0]
          to: [1.0, 0.0]
        - id: seg_b
          from: [1.0, 0.0]
          to: [1.0, 1.0]
"#;
    let doc = Document::from_yaml(v1_yaml).expect("v1 yaml should parse via migration");
    assert_eq!(doc.schema_version, 2, "v1 doc should be migrated to v2");
    // profile 内容を抽出して seg_a / seg_b が Line として保存されていることを確認
    let create_sketch = match &doc.root_component.features[0] {
        Feature::CreateSketch { profile, .. } => profile,
        _ => panic!("expected CreateSketch"),
    };
    assert_eq!(create_sketch.len(), 2);
    assert!(matches!(&create_sketch[0], SketchElement::Line { id, .. } if id == "seg_a"));
    assert!(matches!(&create_sketch[1], SketchElement::Line { id, .. } if id == "seg_b"));
}
```

### F27 [defer]: A-F01 strict v2 wire-format gate は本 Issue では deferred (F19 / F23 と同方針)

Codex r4 でも同じ指摘 (A-F01) が出ているが、本 Issue では引き続き deferred。rejection.md の "## Codex deferred findings (post-merge follow-up Issue 候補)" セクションに記録済。Phase 11+ で別 ADR + Issue として扱う。

### 進捗管理

F24 + F25 + F26 を実装、F27 は code 変更なし。実装後 `cargo xtask ci` で green 確認。これが本 Issue 最後の GLM dispatch。
