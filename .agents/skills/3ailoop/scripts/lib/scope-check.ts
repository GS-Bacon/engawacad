#!/usr/bin/env bun
// scope-check.ts — Issue scope enforcement for landing (new runtime)
//
// contract §"State, receipts, Git, and landing" #4:
//   default-branch mutation の前に scope を再検証。scope に収まらない変更は scope-drift。
//
// contract §"In scope" / §"Out of scope":
//   worker の変更は Issue の宣言 scope 内にとどまる。product 本体 (crates/ web/ examples/ の CAD 挙動)
//   は out-of-scope なので、宣言 scope が無ければ安全側に倒して fail (fail-closed) する。
//   これにより land.ts が scope を常に ok にして任意の変更を通すのを防ぐ。
//
// 設計:
// - scope manifest (JSON) は worker の worktree に `.worker/scope.json` として置かれる。
//   { "allowed_paths": ["<glob>", ...] }  (glob: * は単一 segment, ** は任意深さ)
// - 変更 file が 1 つでも allowed_paths にマッチしなければ scope-drift。
// - manifest が無い/壊れている → fail-closed (scope-drift)。絶対に無条件 ok にしない。
//
// DI 設計: glob match は純粋関数。manifest 読み込みは IO。
//
// 関連: contract §"State, receipts, Git, and landing" #4, lib/land-serial.ts checkScope

import { existsSync, readFileSync } from "fs";
import { resolve } from "path";

export interface ScopeManifest {
  /** allowed path globs (repo-relative, POSIX separators). A changed file must match at least one. */
  allowed_paths: string[];
}

export interface ScopeCheckResult {
  ok: boolean;
  detail: string;
  /** files outside the declared scope (empty when ok). */
  outOfScope: string[];
}

/** Match a single POSIX path against a glob pattern (supports * and **). Pure. */
export function matchGlob(path: string, pattern: string): boolean {
  // normalize: collapse consecutive slashes, strip leading ./
  const p = path.replace(/\/+/g, "/").replace(/^\.\//, "");
  return matchSegment(p.split("/"), pattern.replace(/\/+/g, "/").replace(/^\.\//, "").split("/"));
}

function matchSegment(pathParts: string[], patParts: string[]): boolean {
  for (let i = 0; i < patParts.length; i++) {
    const pat = patParts[i];
    if (pat === "**") {
      // ** matches zero or more whole segments; try every remainder
      if (i === patParts.length - 1) return true; // trailing ** matches all remaining
      for (let j = i; j <= pathParts.length; j++) {
        if (matchSegment(pathParts.slice(j), patParts.slice(i + 1))) return true;
      }
      return false;
    }
    if (i >= pathParts.length) return false;
    if (!matchSingle(pathParts[i], pat)) return false;
  }
  // pattern exhausted: ok only if path also exhausted
  return pathParts.length === patParts.length;
}

/** Match one path segment against one pattern segment (* = any chars within the segment). */
function matchSingle(seg: string, pat: string): boolean {
  if (pat === "*") return seg.length > 0;
  if (!pat.includes("*")) return seg === pat;
  // split pattern on * and verify seg walks through the literal anchors in order
  const parts = pat.split("*");
  if (parts.length === 1) return seg === pat;
  let idx = 0;
  for (let k = 0; k < parts.length; k++) {
    const lit = parts[k];
    if (k === 0) {
      if (!seg.startsWith(lit, idx)) return false;
      idx += lit.length;
    } else if (k === parts.length - 1) {
      if (!seg.endsWith(lit) || idx > seg.length - lit.length) return false;
    } else {
      const found = seg.indexOf(lit, idx);
      if (found < 0) return false;
      idx = found + lit.length;
    }
  }
  return true;
}

/** Pure: given changed files + allowed globs, decide scope. Fail-closed when allowed is empty. */
export function checkFilesScope(changedFiles: string[], allowedGlobs: string[]): ScopeCheckResult {
  if (allowedGlobs.length === 0) {
    return {
      ok: false,
      detail: "no scope manifest (allowed_paths empty/absent) — fail-closed",
      outOfScope: changedFiles.slice().sort(),
    };
  }
  const outOfScope = changedFiles
    .filter(f => !allowedGlobs.some(g => matchGlob(f, g)))
    .sort();
  return {
    ok: outOfScope.length === 0,
    detail: outOfScope.length === 0
      ? `all ${changedFiles.length} changed file(s) within declared scope`
      : `${outOfScope.length} changed file(s) outside declared scope`,
    outOfScope,
  };
}

/** Load a scope manifest from `<worktree>/.worker/scope.json`. Returns null when absent/invalid
 *  (caller treats null as fail-closed). Pure-ish (one read). */
export function loadScopeManifest(worktree: string): ScopeManifest | null {
  const p = resolve(worktree, ".worker", "scope.json");
  if (!existsSync(p)) return null;
  try {
    const obj = JSON.parse(readFileSync(p, "utf-8")) as Partial<ScopeManifest>;
    if (Array.isArray(obj.allowed_paths) && obj.allowed_paths.every(x => typeof x === "string")) {
      return { allowed_paths: obj.allowed_paths };
    }
    return null;
  } catch {
    return null;
  }
}

/** Full scope check: load the manifest for the worktree and verify every changed file is in scope.
 *  Pure-ish (reads manifest). Fails closed when no manifest. */
export function checkScope(worktree: string, changedFiles: string[]): ScopeCheckResult {
  const manifest = loadScopeManifest(worktree);
  return checkFilesScope(changedFiles, manifest?.allowed_paths ?? []);
}
