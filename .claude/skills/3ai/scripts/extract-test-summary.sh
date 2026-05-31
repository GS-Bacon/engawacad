#!/usr/bin/env bash
# extract-test-summary.sh — CI ログからテスト結果を構造化 JSON に変換
# 使い方:
#   extract-test-summary.sh --ci-log <ci.log> --output <test-summary.json> [--base <branch>]
set -euo pipefail

CI_LOG=""
OUTPUT_FILE=""
BASE_BRANCH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ci-log) CI_LOG="$2";      shift 2 ;;
    --output) OUTPUT_FILE="$2"; shift 2 ;;
    --base)   BASE_BRANCH="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

[[ -z "$CI_LOG"      ]] && { echo "ERROR: --ci-log required"  >&2; exit 1; }
[[ -z "$OUTPUT_FILE" ]] && { echo "ERROR: --output required"  >&2; exit 1; }
[[ ! -f "$CI_LOG"    ]] && { echo "ERROR: ci.log not found: $CI_LOG" >&2; exit 1; }

# ベースブランチが未指定の場合は origin/HEAD から解決
if [[ -z "$BASE_BRANCH" ]]; then
  BASE_BRANCH="$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@' || echo "")"
fi

python3 - "$CI_LOG" "$OUTPUT_FILE" "$BASE_BRANCH" <<'PY'
import sys, re, json, subprocess

ci_log_path, output_path, base_branch = sys.argv[1], sys.argv[2], sys.argv[3]

log = open(ci_log_path, encoding="utf-8", errors="replace").read()

# --- totals ---
totals = {"passed": 0, "failed": 0, "ignored": 0}
for m in re.finditer(
    r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',
    log
):
    totals["passed"]  += int(m.group(1))
    totals["failed"]  += int(m.group(2))
    totals["ignored"] += int(m.group(3))

# --- by_crate ---
by_crate = {}
for m in re.finditer(
    r'Running (?:unittests |tests/)?[^\s]+\s+\(([^)]+)\)[^\n]*\n(?:.*\n)*?'
    r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed',
    log
):
    path = m.group(1)
    crate = re.search(r'/([^/]+)/target/', path)
    crate_name = crate.group(1) if crate else path.split("/")[-1]
    entry = by_crate.setdefault(crate_name, {"passed": 0, "failed": 0})
    entry["passed"] += int(m.group(2))
    entry["failed"] += int(m.group(3))

# --- added_in_round: git diff から追加 test 関数名を取得 ---
added_in_round = []
if base_branch:
    try:
        diff = subprocess.check_output(
            ["git", "diff", f"{base_branch}...HEAD", "--", "*.rs"],
            encoding="utf-8", errors="replace", stderr=subprocess.DEVNULL
        )
        for m in re.finditer(r'^\+\s*(?:async\s+)?fn\s+(test_[A-Za-z0-9_]+)', diff, re.MULTILINE):
            name = m.group(1)
            kind = "other"
            if re.search(r'determin|repeat|twice|idempotent', name):
                kind = "determinism"
            elif re.search(r'degenerat|zero_|empty_|degenerate', name):
                kind = "degenerate"
            elif re.search(r'max|min|inf|nan|boundary|overflow|extreme', name):
                kind = "boundary"
            elif re.search(r'golden|roundtrip|yaml|serialize', name):
                kind = "golden"
            elif re.search(r'edge|case', name):
                kind = "edge_case"
            added_in_round.append({"name": name, "kind": kind})
    except Exception:
        pass

# --- coverage_hints ---
hints = {k: sum(1 for t in added_in_round if t["kind"] == k)
         for k in ("determinism", "degenerate", "boundary", "golden", "edge_case")}
hints["total_added"] = len(added_in_round)

result = {
    "totals": totals,
    "by_crate": by_crate,
    "added_in_round": added_in_round,
    "coverage_hints": hints,
}

with open(output_path, "w", encoding="utf-8") as f:
    json.dump(result, f, ensure_ascii=False, indent=2)

print(f"=== extract-test-summary: passed={totals['passed']} failed={totals['failed']} added={hints['total_added']} ===", file=sys.stderr)
PY
