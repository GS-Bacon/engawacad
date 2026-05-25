## 修正対象
`crates/xtask/src/main.rs` の `ci()` 関数内の TS drift check を修正する。

## 問題
drift check (`git status --porcelain --untracked-files=all -- web/src/generated/`) は、
`web/src/generated/` 内のファイルが一度もコミットされていない場合(初回生成)に
必ず `?? ...` を返して FAIL してしまう。
生成物の初回コミット前には drift check が意味を持たない。

## 修正内容
`ci()` 内の drift check セクションを以下に変更する:

```rust
println!("\n=== Checking TS drift ===");
// If no TS files are tracked in git yet, skip drift check (first-time generation).
let tracked = Command::new("git")
    .args(["ls-files", "web/src/generated/"])
    .output()
    .expect("failed to run git ls-files");
if tracked.stdout.is_empty() {
    println!("No committed TS files found — skipping drift check (run `cargo xtask gen-ts` and commit the results).");
} else {
    let output = Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "web/src/generated/",
        ])
        .output()
        .expect("failed to run git status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        eprintln!("FAILED: TypeScript types are out of sync with committed versions");
        eprintln!("Run `cargo xtask gen-ts` and commit the results.");
        eprintln!("Drift detected:\n{stdout}");
        return ExitCode::FAILURE;
    }
}
```

## 完了条件
1. 上記 Edit を適用する
2. `cargo clippy --workspace -- -D warnings` が通る
3. `cargo xtask ci` が green になる (web/src/generated/ がまだ未コミットの状態で)
4. 結果を glm-result.json に書き出す: { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "..." }

## 禁止
- git commit/push は行わない
