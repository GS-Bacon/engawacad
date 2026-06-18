# Plan: fix(3ai) TS drift CI check vs STEP 8-only commit 規約の構造的不整合解消

## 自律判断ログ (Claude 直接実装方針)

本 Issue は `.claude/skills/3ai/SKILL.md` (3ai プロトコル本文) と `.claude/skills/3ai/scripts/` (helper script 新規) のみ改訂する。`crates/**` は触らない。

Issue 本文の 3 候補のうち **(b) 3ai 規約に「STEP 6 以降 `web/src/generated/` の branch 内 intermediate commit は許容」を明文化** + **helper script で自動化** を採用。理由:

- (a) `xtask ci` の TS drift check 緩和は `crates/xtask` を触る必要があり、guard-crates の self-modification リスク + 全 user の CI 体験を変える副作用大
- (c) STEP 5.5 で gen-ts 事前実行は新規型がまだ Rust 側に存在しないタイミングのため不可能 (gen-ts は現 Rust 型を読む)
- (b) は SKILL.md ドキュメント + 30 行未満の TS helper のみ。`web/src/generated/` の intermediate commit は STEP 8 の squash で 1 commit に集約されるため最終 commit 履歴は変わらない (clean な 1-commit-per-Issue 規約維持)

### 構造問題の事実確認

`crates/xtask/src/main.rs` L828-L853 を Read:

```rust
println!("\n=== Checking TS drift ===");
let tracked = Command::new("git").args(["ls-files", "web/src/generated/"]).output()...;
if tracked.stdout.is_empty() { ... } else {
    let output = Command::new("git").args([
        "status", "--porcelain", "--untracked-files=all", "--", "web/src/generated/",
    ]).output()...;
    if !stdout.trim().is_empty() {
        eprintln!("FAILED: TypeScript types are out of sync with committed versions");
        return ExitCode::FAILURE;
    }
}
```

`web()` (L797) は `ci()` 冒頭で gen-ts を走らせるため、working tree の TS は常に現 Rust 型に追従する。drift check は **HEAD ↔ working tree** の差分を検出する純粋な「未コミット gen-ts 出力」検出器。

3ai プロトコル (SKILL.md 「禁止事項」):

> git commit/push は STEP 8 以外で行わない（`finalize-feature.ts` の commit は STEP 8 の一部として許可）

→ 新規 Rust 型を追加する Issue では `web/src/generated/{Type}.ts` が ?? (untracked) になり、`cargo xtask ci` が必ず FAILED を返す構造。STEP 8 の最終 ci も commit 前に走るため STEP 8 でも fail する。

### 提案する workflow

```
STEP 6 GLM core_impl → 新規 Rust 型を追加 (uncommitted)
   ↓
新ステップ: web/src/generated/ が dirty なら intermediate commit
   bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts --issue N
   ↓
cargo xtask ci → web() で gen-ts 走る → TS は HEAD と一致 (今 commit したばかり) → drift 0 → PASS
   ↓
STEP 6.5/6.6/7/7.5 進行 (TS は更に変わる可能性あるので各 ci 前に同 script を再実行可)
   ↓
STEP 8: 最終 ci 前に同 script → 最後の gen-ts commit
   ↓
git merge --squash で cad/N-slug の全コミット (Rust + 複数 intermediate TS) を 1 commit に集約
   ↓
最終 commit は通常通り 1 つ
```

intermediate commit のメッセージは `chore(3ai): #N gen-ts intermediate (squashed in STEP 8)` で squash 後に消える。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `.claude/skills/3ai/scripts/maybe-commit-generated-ts.ts` 新規 (`web/src/generated/` dirty 検出 → intermediate commit) | `crates/xtask/src/main.rs` の drift check ロジック改訂 (上記 (a) は採用しない) |
| SKILL.md の STEP 6 / STEP 8 にスクリプト呼び出しを追加 | gen-ts の決定性検証 (xtask に既に test t06/t07 あり、別 Issue で扱う) |
| SKILL.md 「禁止事項」セクションに `web/src/generated/` intermediate commit 例外を追記 | 既存 Issue #241 の TS 出力 retro-fix (該当 Issue 既 closed、影響なし) |
| `bun test` 単体テスト: dirty 検出 / clean 時 noop / commit message format | GitHub Actions などの外部 CI 整合性確認 (本リポジトリは現状 `cargo xtask ci` が唯一の gate) |

## Non-Goals

- `crates/xtask` の改訂 (上記理由で見送り、別 Issue 化もしない — script + protocol で十分機能する)
- gen-ts の出力フォーマット変更 (現状の ts-rs 経由をそのまま使用)
- `web/src/generated/` 以外の auto-generated 領域への展開 (現状なし)
- intermediate commit の回数最小化 (現状実装は「ci の都度 dirty なら commit」、最小化は別 Issue)
- `crates/<crate>/tests/<feature>_acceptance.rs` の skeleton 追加 (本 Issue は `crates/` を一切触らず Rust acceptance test の concept が適用不可、state shim で acceptance_skeleton passed をセットして STEP 6 ゲートを通す)

## 実装対象

### A. 新規 script: `.claude/skills/3ai/scripts/maybe-commit-generated-ts.ts`

```typescript
#!/usr/bin/env bun
// maybe-commit-generated-ts.ts (#248)
// web/src/generated/ が dirty (modified or untracked) なら intermediate commit。
// STEP 8 の squash で全部 1 commit にまとまる前提の WIP commit。
import { spawnSync } from "child_process";

function gitStatusGenerated(): string {
  const r = spawnSync("git", [
    "status", "--porcelain", "--untracked-files=all", "--", "web/src/generated/",
  ], { encoding: "utf-8" });
  return (r.stdout ?? "").trim();
}

function main() {
  const argv = process.argv;
  const issueArg = argv.indexOf("--issue");
  const issueNum = issueArg >= 0 ? argv[issueArg + 1] : undefined;

  const status = gitStatusGenerated();
  if (!status) {
    console.log("OK: web/src/generated/ is clean — no intermediate commit needed");
    return;
  }
  console.log(`Detected drift:\n${status}`);

  const addRes = spawnSync("git", ["add", "web/src/generated/"], { stdio: "inherit" });
  if (addRes.status !== 0) { console.error("ERROR: git add failed"); process.exit(1); }
  const msg = issueNum
    ? `chore(3ai): #${issueNum} gen-ts intermediate (squashed in STEP 8)`
    : `chore(3ai): gen-ts intermediate (squashed in STEP 8)`;
  const r = spawnSync("git", ["commit", "-m", msg], { stdio: "inherit" });
  if (r.status !== 0) { console.error("ERROR: git commit failed"); process.exit(1); }
  console.log(`OK: intermediate commit created: ${msg}`);
}

main();
```

挙動:
- `git status --porcelain web/src/generated/` で dirty 判定 (modified / untracked 両方含む)
- clean → no-op で exit 0
- dirty → `git add web/src/generated/` + commit
- commit メッセージは `chore(3ai): #N gen-ts intermediate (squashed in STEP 8)`
- 非ゼロ exit は git add / commit 失敗のみ

### B. SKILL.md 改訂 (3 箇所)

#### B-1: STEP 6-A の `cargo xtask ci` 実行例の後に追記

`dispatch-glm.ts ... --mode core` の直後に来るブロックに以下を追加:

```bash
# TS drift mitigation (#248): generated TS files の dirty を intermediate commit
bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts --issue $ISSUE_NUM
```

#### B-2: STEP 8 直前に追記

```bash
bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts --issue $ISSUE_NUM
bun .claude/skills/3ai/scripts/pre-step8-check.ts ...
cargo xtask ci  # 最終 green
```

#### B-3: 「禁止事項」セクションに例外文言

既存 bullet:
> - git commit/push は STEP 8 以外で行わない（`finalize-feature.ts` の commit は STEP 8 の一部として許可）

の直下に追記:

> - **例外**: `web/src/generated/` の auto-generated TS は STEP 6 以降 `maybe-commit-generated-ts.ts` 経由の intermediate commit を許容 (#248)。STEP 8 squash で最終 1 commit に集約される。

### C. 単体テスト: `.claude/skills/3ai/scripts/maybe-commit-generated-ts.test.ts`

`bun test` 経由で実行。一時ディレクトリに git repo を作り、`maybe-commit-generated-ts.ts` を spawn して挙動を検証。

## 数値モデル

該当なし。

## 幾何的不変条件チェックリスト

- Boolean op: N/A
- Partition: N/A
- Assemble: N/A

## テスト計画 (ID 付き)

| ID | 種別 | 検証 | 方法 |
|----|------|------|------|
| T01_determinism | 決定性 | 同一 working tree で 2 回連続 invoke → 2 回目は no-op (1 回目で commit して clean になる) | bun test fixture を整え、2 回 spawn |
| T02_clean_noop | 正常 | `web/src/generated/` が clean → no-op exit 0、commit count 変化なし | `git log --oneline \| wc -l` の差分 0 |
| T03_modified_commit | 正常 | tracked TS を改変 → commit 1 つ追加 | `git log --oneline \| wc -l` の差分 1 + `git show HEAD --stat` で対象ファイル含む |
| T04_untracked_commit | 正常 | 新規 TS を追加 → commit 1 つ追加 + untracked → tracked に変わる | T03 と同様 |
| T05_issue_msg | 正常 | `--issue 248` でメッセージに `#248` が含まれる | `git log -1 --pretty=%B \| grep "#248"` |
| T_DEG_outside_gen | 退化 | `web/src/generated/` 外のファイル (例: `web/src/main.ts`) の dirty は対象外 | `git status` で main.ts は依然 dirty、generated/ のみ clean |
| T_BOUNDARY_no_issue | 境界 | `--issue` 引数なし → メッセージに `#` が含まれない | `git log -1 --pretty=%B` を確認 |

## 検証手順

```bash
# 1. script 存在確認
[ -f .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts ] && echo PASS-script

# 2. 直接実行 (現在の working tree は clean なので no-op が期待)
bun .claude/skills/3ai/scripts/maybe-commit-generated-ts.ts --issue 248

# 3. bun test で T01〜T_BOUNDARY を実行
bun test .claude/skills/3ai/scripts/maybe-commit-generated-ts.test.ts

# 4. CI 確認 (script 追加だけだから cargo xtask ci 自体は無関係に green)
cargo xtask ci
```

STEP 5.5 acceptance_skeleton は **state shim** でセット (Rust 触らない docs/script-only 改訂のため):

```bash
bun .claude/skills/3ai/scripts/state.ts set features/248-3ai-step-a-esc1/state.json acceptance_skeleton passed
```

## STEP 7.5 (Codex) で見るべき観点

- `maybe-commit-generated-ts.ts` の `git status --porcelain` 解釈が正しいか (`?? new.ts` と ` M file.ts` 両方を dirty と判定)
- intermediate commit メッセージのフォーマットが将来の `git log` 集計や `finalize-feature.ts` の squash 動作に影響しないか
- SKILL.md の STEP 6/8 への script 挿入位置が他 STEP の前提を壊さないか
- 「禁止事項」例外の文言が他の auto-generated 領域 (将来の `web/src/generated/` 以外) への適用を誤認させないか
- self-modification リスク評価: `.claude/skills/3ai/scripts/` 改訂は次サイクルの 3ai 実行に影響するが、本 helper は副作用なし (clean なら no-op) のため安全
- `git add web/src/generated/` が `.gitignore` 等で想定外のファイルを include しないか
