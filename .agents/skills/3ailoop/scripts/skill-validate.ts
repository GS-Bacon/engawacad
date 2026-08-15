#!/usr/bin/env bun
// skill-validate.ts — Skills の構造的 validation (official validator 相当の structural check)
//
// contract acceptance criteria:
//   "Repository discovery finds the new entry and component Skills in `.agents/skills`;
//    each validates with the official skill validator or equivalent structural checks."
//   "Every Skill must have valid `SKILL.md` frontmatter and matching `agents/openai.yaml` metadata."
//
// 検証項目 (per skill dir):
//   1. SKILL.md が存在
//   2. SKILL.md が YAML frontmatter を持つ (--- で挟まれたブロック)
//   3. frontmatter に name (kebab/英数) と description (非空) が含まれる
//   4. agents/openai.yaml が存在し、name/description を持ち SKILL.md と一致する
//
// 使い方:
//   bun skill-validate.ts --root <dir>          # <dir> 下の各サブディレクトリを skill として検証
//   bun skill-validate.ts --root .agents/skills # default
//
// 出力 (stdout JSON): { checked: N, ok: M, failures: [...] }
// exit: 0=全 ok / 1=一部失敗

import { existsSync, readdirSync, readFileSync, statSync } from "fs";
import { join, resolve } from "path";
import { arg, emitJson, emitStatus, REPO_ROOT } from "./cli/helpers.ts";

interface SkillFailure {
  skill: string;
  errors: string[];
}

function parseFrontmatter(text: string): Record<string, string> | null {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return null;
  const out: Record<string, string> = {};
  for (const line of m[1].split(/\r?\n/)) {
    const mm = line.match(/^([A-Za-z0-9_]+):\s*(.*)$/);
    if (mm) out[mm[1]] = mm[2].trim();
  }
  return out;
}

function parseSimpleYaml(text: string): Record<string, string> {
  // tiny YAML reader: top-level `key: value` lines only (sufficient for our metadata files)
  const out: Record<string, string> = {};
  for (const line of text.split(/\r?\n/)) {
    const mm = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (mm) out[mm[1]] = mm[2].trim();
  }
  return out;
}

function validateSkill(dir: string): string[] {
  const errors: string[] = [];
  const skillMd = join(dir, "SKILL.md");
  if (!existsSync(skillMd)) {
    errors.push("missing SKILL.md");
    return errors;
  }
  const md = readFileSync(skillMd, "utf-8");
  const fm = parseFrontmatter(md);
  if (!fm) {
    errors.push("SKILL.md has no YAML frontmatter (--- ... ---)");
    return errors;
  }
  if (!fm.name || !/^[a-z0-9][a-z0-9-]*$/i.test(fm.name)) {
    errors.push(`frontmatter 'name' missing or invalid: ${fm.name ?? "(none)"}`);
  }
  if (!fm.description || fm.description.length < 8) {
    errors.push("frontmatter 'description' missing or too short (<8 chars)");
  }

  const openaiYaml = join(dir, "agents", "openai.yaml");
  if (!existsSync(openaiYaml)) {
    errors.push("missing agents/openai.yaml metadata");
  } else {
    const y = parseSimpleYaml(readFileSync(openaiYaml, "utf-8"));
    if (!y.name) errors.push("agents/openai.yaml missing 'name'");
    if (!y.description) errors.push("agents/openai.yaml missing 'description'");
    if (fm.name && y.name && fm.name !== y.name) {
      errors.push(`name mismatch: SKILL.md='${fm.name}' vs openai.yaml='${y.name}'`);
    }
  }
  return errors;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const root = arg(args, "--root") ?? resolve(REPO_ROOT, ".agents", "skills"); // .agents/skills by default
  const rootAbs = resolve(root);

  if (!existsSync(rootAbs) || !statSync(rootAbs).isDirectory()) {
    emitJson({ error: "root not found", root: rootAbs });
    return 2;
  }

  const entries = readdirSync(rootAbs).filter(e => statSync(join(rootAbs, e)).isDirectory());
  const failures: SkillFailure[] = [];
  let ok = 0;
  for (const e of entries.sort()) {
    const errs = validateSkill(join(rootAbs, e));
    if (errs.length === 0) ok++;
    else failures.push({ skill: e, errors: errs });
  }

  emitStatus(`skill-validate: checked=${entries.length} ok=${ok} failed=${failures.length}`);
  emitJson({ checked: entries.length, ok, failures, root: rootAbs });
  return failures.length === 0 ? 0 : 1;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
