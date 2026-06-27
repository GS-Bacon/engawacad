# 役割: Codex 独立レビュー (3 persona 並列)

## レビュー対象 commit
2e88da2 fix(3ai): Codex usage/rate limit を auto-raise 対象外にする

## 変更ファイル
- .claude/skills/3ai/scripts/raise-issue-on-failure.ts (shouldSkipError export 追加 + main で skip)
- .claude/skills/3ai/scripts/dispatch-codex.ts (detectCodexUsageLimit export + WARN ログ)
- 上記 2 ファイルのテスト追加 (合計 8 + 7 新規ケース、全 pass)

## レビュー観点
- shouldSkipError の regex が意図しないエラー (例: "request limit exceeded for X", "memory limit") を誤検知しないか
- detectCodexUsageLimit の検出シグナルが Codex CLI の実出力と一致するか (You've hit your usage limit, reset 6:06 PM)
- skip ロジックが silent fail を生まないか (#229 dedup や intent-check skip との相互作用)
- /3ai オーケストレーターが errorSummary に [CODEX_USAGE_LIMIT] WARN を含めない経路がないか
- ADR-006 粒度 (skip ガード追加は単一 PR で問題ないか)

## 出力 yaml
3 persona (architect / contrarian / migration) 並列。
各 persona は issues[] + verdict (pass|fail) を出す。
