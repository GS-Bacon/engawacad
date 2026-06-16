#!/usr/bin/env bash
# loop-tmux-smoke.sh — /3ailoop tmux ランタイムの smoke test
#
# 検証項目:
#  S-1: bun test (pure 関数) が緑
#  S-2: loop-tmux-start.ts --dry-run の出力に必要な tmux コマンドが含まれる
#  S-3: loop-tmux-stop.ts --dry-run の出力に必要なステップが含まれる
#  S-4: 一時 tmux session でワーカー pane (bash) を作り send-keys 配線が通る
#  S-5: watcher の差分検知 (state.json mock 書き換え → DRY-RUN の send-keys が出力される)
#  Cleanup: 一時 tmux session を kill、tmp dir を rm
#
# 関連: ADR-012, Issue #181

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
cd "$REPO_ROOT"

WATCHER="$SCRIPT_DIR/loop-tmux-watcher.ts"
START="$SCRIPT_DIR/loop-tmux-start.ts"
STOP="$SCRIPT_DIR/loop-tmux-stop.ts"
TEST="$SCRIPT_DIR/loop-tmux-watcher.test.ts"

TMP_SESSION="smoke-3ailoop-$$"
TMP_DIR="$(mktemp -d -t 3ailoop-smoke-XXXXXX)"
ORIG_LOOP_DIR="$REPO_ROOT/features/.loop"
BACKUP_DIR=""

red() { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }

cleanup() {
  if tmux has-session -t "$TMP_SESSION" 2>/dev/null; then
    tmux kill-session -t "$TMP_SESSION" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR" 2>/dev/null || true
  if [[ -n "$BACKUP_DIR" && -d "$BACKUP_DIR" ]]; then
    rm -rf "$ORIG_LOOP_DIR/tmux" 2>/dev/null || true
    mv "$BACKUP_DIR" "$ORIG_LOOP_DIR/tmux" 2>/dev/null || true
  fi
}
trap cleanup EXIT

require_tmux() {
  if ! command -v tmux >/dev/null 2>&1; then
    red "SKIP: tmux 未インストール"
    exit 0
  fi
}

# --- S-1 ---
echo "=== S-1: bun test (pure 関数) ==="
bun test "$TEST"
green "S-1 OK"

# --- S-2 ---
echo
echo "=== S-2: loop-tmux-start.ts --dry-run ==="
S2_OUT="$(env -u TMUX TMUX=/tmp/fake-tmux-socket,1,1 bun "$START" --dry-run 2>&1 || true)"
echo "$S2_OUT" | grep -F 'new-window' >/dev/null || { red "S-2 FAIL: new-window 欠落"; echo "$S2_OUT"; exit 1; }
echo "$S2_OUT" | grep -F '3ailoop-worker' >/dev/null || { red "S-2 FAIL: window 名欠落"; exit 1; }
echo "$S2_OUT" | grep -F '/3ailoop' >/dev/null || { red "S-2 FAIL: /3ailoop 投入欠落"; exit 1; }
echo "$S2_OUT" | grep -F 'loop-tmux-watcher.ts' >/dev/null || { red "S-2 FAIL: watcher daemon spawn 欠落"; exit 1; }
green "S-2 OK"

# --- S-3 ---
echo
echo "=== S-3: loop-tmux-stop.ts --dry-run ==="
S3_OUT="$(bun "$STOP" --dry-run 2>&1)"
echo "$S3_OUT" | grep -F 'window' >/dev/null || { red "S-3 FAIL"; exit 1; }
echo "$S3_OUT" | grep -F 'release' >/dev/null || { red "S-3 FAIL: lock release 欠落"; exit 1; }
green "S-3 OK"

# --- S-4 ---
require_tmux
echo
echo "=== S-4: 一時 tmux session で send-keys 配線確認 ==="
tmux new-session -d -s "$TMP_SESSION" -n init "bash"
tmux new-window -d -t "$TMP_SESSION" -n 3ailoop-worker "bash"
sleep 0.5
tmux send-keys -t "$TMP_SESSION:=3ailoop-worker" 'echo SMOKE_MARKER_42' Enter
sleep 0.5
BUF="$(tmux capture-pane -t "$TMP_SESSION:=3ailoop-worker" -p)"
echo "$BUF" | grep -F 'SMOKE_MARKER_42' >/dev/null || { red "S-4 FAIL: marker が pane に出ない"; echo "$BUF"; exit 1; }
green "S-4 OK"

# --- S-5 ---
echo
echo "=== S-5: watcher 差分検知 + DRY-RUN restart ==="
# features/.loop/tmux を退避して mock state を仕込む
if [[ -d "$ORIG_LOOP_DIR/tmux" ]]; then
  BACKUP_DIR="$(mktemp -d -t 3ailoop-bak-XXXXXX)/tmux"
  mkdir -p "$(dirname "$BACKUP_DIR")"
  mv "$ORIG_LOOP_DIR/tmux" "$BACKUP_DIR"
fi
mkdir -p "$ORIG_LOOP_DIR/tmux"
# worker.window を smoke 用 window 名に固定 → watcher が tmuxWindowExists で alive と判定する
echo "3ailoop-worker" > "$ORIG_LOOP_DIR/tmux/worker.window"
# 既存 state.json は触らないが、ended_at が現値とは違う値を last-cycle-ended-at に書いて
# 差分検知を強制する
CURR_ENDED="$(bun -e 'try { const s=JSON.parse(require("fs").readFileSync("features/.loop/state.json","utf-8")); const c=s.recent_cycles?.at(-1)?.ended_at; if(c) console.log(c); } catch {}')"
if [[ -z "$CURR_ENDED" ]]; then
  echo "S-5 SKIP: state.json に recent_cycles が無いため差分検知の実環境テスト不可"
  green "S-5 SKIP-OK"
else
  echo "FORCE-DIFF-$(date +%s)" > "$ORIG_LOOP_DIR/tmux/last-cycle-ended-at"
  # dry-run watcher を起動: 1 周で send-clear-and-restart を吐いて exit
  # --mock-worker-alive: 現セッションに 3ailoop-worker が無くても worker-gone にしない
  S5_OUT="$(LOOP_TMUX_POLL_SEC=1 LOOP_TMUX_CLEAR_WAIT_SEC=0 bun "$WATCHER" run --dry-run --mock-worker-alive 2>&1 || true)"
  echo "$S5_OUT" | grep -F 'DRY-RUN: tmux send-keys' >/dev/null && \
    echo "$S5_OUT" | grep -F '/clear' >/dev/null && \
    echo "$S5_OUT" | grep -F '/3ailoop' >/dev/null || {
      red "S-5 FAIL: dry-run send-keys が出ない"
      echo "$S5_OUT"
      exit 1
    }
  green "S-5 OK"
fi

echo
green "===== smoke test all passed ====="
