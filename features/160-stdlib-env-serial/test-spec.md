# Test Spec: Issue #160 — STEP 6.5 (Claude が作成)

## 実装差分サマリ

STEP 6 で適用された差分 (`git diff` 結果):

| ファイル | 変更内容 |
|---|---|
| `crates/engawa-build/Cargo.toml` | `[dev-dependencies]` に `serial_test = { workspace = true }` を追加 |
| `crates/engawa-build/tests/assembly_acceptance.rs` | `use serial_test::file_serial;` を l.7 に追加 / `t02_stdlib_reference_resolved` (l.115) と `t08_stdlib_root_not_set_error` (l.297) に `#[file_serial(engawa_stdlib_path)]` 付与 |
| `crates/engawa-build/tests/stdlib_env_race_acceptance.rs` | 新規 skeleton (`t03/t04` を `#[ignore]` + `#[file_serial(engawa_stdlib_path)]` + `todo!()` で配置済み、`use serial_test::file_serial;` 含む) |
| `Cargo.lock` | serial_test 関連の lock 自動更新 |

実装は plan.md の §実装対象 (A〜D) と完全一致。**plan に書いていなかったが生じた分岐・ケースなし**。

## 期待値乖離チェック

`check-spec-divergence.ts` 実行結果: 実装差分が未コミットのため git diff が空、検出対象なし → 期待値乖離 **なし**。
plan の T01〜T04 の期待値と、現実装 (`#[file_serial(engawa_stdlib_path)]` + `todo!()` の組み合わせ) は整合している。

## 不足テスト (plan 計画分)

STEP 6.6 で GLM が実装する対象。skeleton は配置済みだが本体が `todo!()` のまま。

### T03_meta_lock_protects_concurrent_resolve

**ファイル**: `crates/engawa-build/tests/stdlib_env_race_acceptance.rs`
**関数**: `t03_meta_lock_protects_concurrent_resolve`
**目的**: cross-binary で同一 lock (`engawa_stdlib_path`) が `file_serial` により直列化されていることを、別 binary でも `ENGAWA_STDLIB_PATH` を mutate するテストが実在する状態として保証する。

**想定実装**:

```rust
#[test]
#[file_serial(engawa_stdlib_path)]
fn t03_meta_lock_protects_concurrent_resolve() {
    use engawa_format::component::{Component, ComponentRef};
    use engawa_format::document::Document;
    use engawa_kernel::brep::topology::IdGenerator;

    let tmp = tempfile::tempdir().unwrap();
    let stdlib_dir = tmp.path().join("stdlib");
    std::fs::create_dir_all(&stdlib_dir).unwrap();

    // stdlib 下に最小の参照先 component yaml を配置
    let part_yaml = "\
name: dummy_part
features: []
children: []
";
    std::fs::write(stdlib_dir.join("dummy_part.engawa"), part_yaml).unwrap();

    // env を set し、build_assembly が stdlib reference を解決できることを確認
    let old = std::env::var("ENGAWA_STDLIB_PATH").ok();
    std::env::set_var("ENGAWA_STDLIB_PATH", &stdlib_dir);

    let doc = Document {
        name: "root".into(),
        features: vec![],
        children: vec![ComponentRef::StdLib { path: "dummy_part.engawa".into() }],
    };
    let mut id_gen = IdGenerator::new();
    let result = build_assembly(&doc, &mut id_gen);
    let _ = result.expect("stdlib reference must resolve when env is set");

    // restore
    match old {
        Some(v) => std::env::set_var("ENGAWA_STDLIB_PATH", v),
        None => std::env::remove_var("ENGAWA_STDLIB_PATH"),
    }
}
```

**期待**: `build_assembly` が `Ok(_)` を返す (stdlib reference 解決成功)。

⚠️ **想定実装は参考値**: 実コード上の `Document` 型 / `ComponentRef::StdLib` の variant 名 / `build_assembly` のシグネチャは GLM が現状を grep して確定すること。plan §C で示した擬似コードからの分岐は許容する (= 期待値ではなく挙動の意味で「stdlib 参照が解決される」が満たされれば OK)。

### T04_meta_boundary_remove_set_remove_is_isolated (退化/境界)

**ファイル**: `crates/engawa-build/tests/stdlib_env_race_acceptance.rs`
**関数**: `t04_meta_boundary_remove_set_remove_is_isolated`
**目的**: env を `remove → set → remove` のシーケンスで遷移させ、`build_assembly` の結果が「`Err(StdlibRoot 系)` → `Ok` → `Err(StdlibRoot 系)`」と遷移すること。これにより `file_serial` lock 内では env 状態が一貫し、他テストに漏れないことを担保する。

**想定実装**:

```rust
#[test]
#[file_serial(engawa_stdlib_path)]
fn t04_meta_boundary_remove_set_remove_is_isolated() {
    use engawa_format::component::{Component, ComponentRef};
    use engawa_format::document::Document;
    use engawa_kernel::brep::topology::IdGenerator;

    let old = std::env::var("ENGAWA_STDLIB_PATH").ok();
    let tmp = tempfile::tempdir().unwrap();
    let stdlib_dir = tmp.path().join("stdlib");
    std::fs::create_dir_all(&stdlib_dir).unwrap();
    std::fs::write(stdlib_dir.join("p.engawa"), "name: p\nfeatures: []\nchildren: []\n").unwrap();

    let doc = Document {
        name: "root".into(),
        features: vec![],
        children: vec![ComponentRef::StdLib { path: "p.engawa".into() }],
    };

    // Phase 1: remove → エラー
    // ただし repo-local `stdlib/` が存在するとフォールバック解決される可能性があるため、
    // repo-local fallback を temporarily 無視する方策として、参照名 `p.engawa` は repo-local stdlib に存在しない名前を選ぶ。
    std::env::remove_var("ENGAWA_STDLIB_PATH");
    let r1 = build_assembly(&doc, &mut IdGenerator::new());
    assert!(r1.is_err(), "env unset (and repo-local stdlib に未配置) で参照解決が失敗すること");

    // Phase 2: set → 成功
    std::env::set_var("ENGAWA_STDLIB_PATH", &stdlib_dir);
    let r2 = build_assembly(&doc, &mut IdGenerator::new());
    assert!(r2.is_ok(), "env set で参照解決が成功すること");

    // Phase 3: remove → 再びエラー
    std::env::remove_var("ENGAWA_STDLIB_PATH");
    let r3 = build_assembly(&doc, &mut IdGenerator::new());
    assert!(r3.is_err(), "env 再 remove で参照解決が失敗すること");

    // restore (元値が None ならすでに remove 済み)
    if let Some(v) = old { std::env::set_var("ENGAWA_STDLIB_PATH", v); }
}
```

**期待**: `is_err() → is_ok() → is_err()` の遷移。

⚠️ **実装上の注意 (重要)**:
- `resolve_stdlib_root` (`crates/engawa-build/src/lib.rs:520-535`) は env 未設定時に **repo-local `stdlib/` ディレクトリへ fallback** する。リポジトリ直下に `stdlib/` が存在する場合、env を remove しても解決が成功してしまう可能性がある。
- 対策案 A: `t04` のテスト参照名は repo-local `stdlib/` に存在しない (= `p.engawa` のような未登録名) を使うことで「env unset → 参照ファイル不在エラー」を引き起こす。これは fallback が走っても結果として `Err` になるためテスト意図と整合する。
- 対策案 B: 参照名でなく resolve 自体の挙動 (`resolve_stdlib_root` が `Some(env_path)` vs `Some(repo_local_path)` vs `None` のいずれを返すか) を間接的に build_assembly のエラーメッセージで観察する。
- GLM は **対策案 A** を採用すること (シンプルかつ意図と整合)。

## 実装差分から追加すべきテスト

なし。
plan に列挙した T01〜T04 + T_REGRESSION_workspace_3runs を満たせば差分の全カバレッジに到達する。

## エッジケース・退化入力

- T04 が退化境界 (env の remove → set → remove 遷移) を担当。
- 他のエッジケース (例: env を空文字列に set した場合) は `resolve_stdlib_root` が `p.trim().is_empty()` で空文字列を None 扱いする (`crates/engawa-build/src/lib.rs:522-526`) ことから、env 空文字列も「unset と同じ」挙動になる。これは plan のスコープ外 (中期対処) で扱うため本 Issue では追加テストしない。

## 数値境界

N/A (test infrastructure 修正、tolerance/ε 不要)。

## 決定性

N/A (production code 不変、`IdGenerator` 等への影響なし)。

## 類似ケース（未カバー）

- `ENGAWA_STDLIB_PATH` を mutate するテストは grep 全件 (`std::env::set_var.*ENGAWA_STDLIB_PATH` / `std::env::remove_var.*ENGAWA_STDLIB_PATH`) で `assembly_acceptance.rs` の `t02` / `t08` および新規 `stdlib_env_race_acceptance.rs` の `t03` / `t04` のみ。**未カバーの類似呼び出しなし** (Explore agent 確認済み)。
- 他の env var (`RUST_LOG` 等) を mutate するテストは本 Issue のスコープ外。`file_serial` の cross-binary lock は env var ごとに lock 名を分ければ独立して保護できる。

## STEP 6.6 GLM dispatch 仕様

- **mode**: `test`
- **対象ファイル**: `crates/engawa-build/tests/stdlib_env_race_acceptance.rs` のみ
- **やること**:
  1. `t03_meta_lock_protects_concurrent_resolve` の `todo!()` を上記想定実装で置換 (`Document` / `ComponentRef` / `build_assembly` の正確なシグネチャは `crates/engawa-build/src/lib.rs` / `crates/engawa-format/src/component.rs` を grep して確定)
  2. `t04_meta_boundary_remove_set_remove_is_isolated` の `todo!()` を同様に実装 (対策案 A 採用)
  3. 両テストの `#[ignore = "..."]` を **削除** (`#[file_serial(...)]` は残す)
  4. `cargo test -p engawa-build --test stdlib_env_race_acceptance` で 2 件 PASS を確認
  5. `cargo test --workspace` で全 binary PASS を確認 (T_REGRESSION の検証)
- **やらないこと**:
  - `assembly_acceptance.rs` への変更 (STEP 6 で完了済み)
  - `Cargo.toml` への変更 (STEP 6 で完了済み)
  - production code (`crates/engawa-build/src/**`) への変更 (Non-Goals)
