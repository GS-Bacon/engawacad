---
name: 3ailoop-phase
description: Maintain ADR / Phase / retrospective / dashboard state. Preserves the user-visible Phase close-check, Phase seed, retrospective metrics, and cumulative dashboard from the prior loop, without depending on ClaudeCode or GLM. Reads ROADMAP.md and features/.loop logs; emits drafts and verdicts only (no GitHub writes).
allowed-tools: Read, Bash, Glob, Grep
---

# $3ailoop-phase — Phase maintenance

ADR/Phase/retrospective/dashboard のユーザー可視機能を維持する (contract §"Compatibility and cutover" #1)。
ClaudeCode/GLM 非依存。

## サブコマンド

```bash
# Phase 完了条件検証 (+ apply 時は ROADMAP 🚧→✅ 更新と milestone close 指示を Skill に出す)
bun .agents/skills/3ailoop/scripts/phase.ts close-check --phase <N> [--apply]

# Phase 昇格後の起点 Issue 草案生成 (粒度ガード付き, ADR-002)
bun .agents/skills/3ailoop/scripts/phase.ts seed --phase <N>

# Phase 運用メトリクス集計 (pause 率 / needs-human / codex-skip / Skill 改修効果)
bun .agents/skills/3ailoop/scripts/phase.ts retrospective --phase <N> [--dry-run]

# features/.dashboard.md 累積俯瞰 (再)生成
bun .agents/skills/3ailoop/scripts/phase.ts dashboard

# Codex 3 personaの独立結果を決定的に集約（GLM fallbackなし）
bun .agents/skills/3ailoop/scripts/compatibility.ts adr-verdict --input <verdicts.json>

# split proposalを親milestone継承済みの子Issue草案へ変換（GitHub書込なし）
bun .agents/skills/3ailoop/scripts/compatibility.ts split-drafts \
  --review <review.yaml> --parent <N> --milestone "Phase N: ..."
```

## 権限と互換

- 本 CLI は **草案と判定** まで。GitHub への起票/push/milestone 変更は行わない (delivery worker は write 禁止)。
  Issue 起票・ROADMAP 更新・milestone close は Codex/ユーザー明示承認経路で行う (ADR-002)。
- 旧 `loop-phase-close-check.ts` / `loop-phase-seeder.ts` / `loop-phase-retrospective.ts` /
  `loop-dashboard.ts` のユーザー可視判定を、Claude/GLM非依存のsnapshot/draft処理として引き継ぐ。
- ADR review本文はCodexがarchitect/contrarian/migrationを独立に作成し、scriptは3結果の完全性と結論だけを判定する。
- split childは草案まで生成し、ADR-006粒度guardとlabel lintを通した後にのみ、Codexの承認済みGitHub経路で起票する。

詳細は [../3ailoop/references/runtime.md](../3ailoop/references/runtime.md) の Cutover / Compatibility 節。
