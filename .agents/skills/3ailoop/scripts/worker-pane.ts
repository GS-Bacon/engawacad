#!/usr/bin/env bun
// worker-pane.ts — per-worker tmux pane lifecycle CLI (real tmux adapter)
//
// contract §"Orchestration and authority" #5/#6 を満たす operator-facing pane 管理。
// lib/worker-pane.ts の pure logic に real tmux adapter を注入する thin CLI。
//
// 3 つの subcommand:
//   allocate  : 右側に pane を split → status line 設定 → paneId を返す
//   stream    : 既存 pane に launch command を send (worker CLI 出力を流す)
//   release   : terminal 後の決定的 cleanup (reuse / kill)
//
// 使い方:
//   bun worker-pane.ts allocate --issue <N> --worker-id <id> \
//       --provider <p> --model <m> --session <s> [--split-pct 50]
//   bun worker-pane.ts stream --pane-id <%n> --command '<cmd string>'
//   bun worker-pane.ts release --pane-id <%n>
//
// 出力 (stdout JSON): { ok, pane_id?, status_line?, error? }
// exit: 0=成功 / 1=pane 操作失敗 (bounded, lifecycle には影響しない) / 2=usage
//
// 注意: この CLI は tmux が利用可能でかつ tmux session 内 ($TMUX または target session)
// でのみ動く。tmux 未導入 / session 外では allocate は bounded 失敗 (exit 1, error) となり、
// loop の lifecycle (CAS/registry) は一切変更しない (contract #6)。

import { arg, emitJson, emitStatus, runCmd } from "./cli/helpers.ts";
import {
  allocatePane,
  streamLaunch,
  releasePane,
  DEFAULT_SPLIT_PCT,
  type WorkerPaneDeps,
  type PaneRecord,
  type PaneIdentity,
} from "./lib/worker-pane.ts";

/** Real tmux adapter. All tmux invocations go through runCmd so behavior is uniform and
 *  the same surface is faked in tests. */
function realTmuxDeps(): WorkerPaneDeps {
  return {
    splitRight: async ({ splitPct }) => {
      // -h: right-side split. -P -F: print the new pane's id. -l: width percent.
      const r = await runCmd([
        "tmux", "split-window", "-h", "-P", "-F", "#{pane_id}", "-l", `${splitPct}%`,
      ]);
      if (!r.ok) return { ok: false, error: r.stderr.trim() || `tmux exit ${r.exitCode}` };
      const paneId = r.stdout.trim();
      if (!/^%?\d+$/.test(paneId)) return { ok: false, error: `unexpected pane id: '${paneId}'` };
      return { ok: true, paneId };
    },
    setStatusLine: async (paneId, text) => {
      // pane_title is the durable, scrape-resistant status surface (old runtime used it too).
      const r = await runCmd(["tmux", "select-pane", "-t", paneId, "-T", text]);
      return r.ok ? { ok: true } : { ok: false, error: r.stderr.trim() };
    },
    sendKeys: async (paneId, command) => {
      const r = await runCmd(["tmux", "send-keys", "-t", paneId, command, "Enter"]);
      return r.ok ? { ok: true } : { ok: false, error: r.stderr.trim() };
    },
    release: async (paneId) => {
      // kill-pane is the deterministic cleanup; a released pane is gone, freeing the slot.
      // (Reuse is realized by allocating a fresh pane for the next worker; we never recycle
      // a pane id because lifecycle correctness must not depend on pane state — contract #6.)
      const r = await runCmd(["tmux", "kill-pane", "-t", paneId]);
      return r.ok ? { ok: true } : { ok: false, error: r.stderr.trim() };
    },
    capture: async (paneId) => {
      const r = await runCmd(["tmux", "capture-pane", "-t", paneId, "-p"]);
      return { ok: r.ok, output: r.stdout };
    },
    now: () => Date.now(),
  };
}

async function cmdAllocate(args: string[]): Promise<number> {
  const issueStr = arg(args, "--issue");
  const workerId = arg(args, "--worker-id");
  const provider = arg(args, "--provider");
  const model = arg(args, "--model");
  const session = arg(args, "--session");
  const splitPct = arg(args, "--split-pct") ? parseInt(arg(args, "--split-pct")!, 10) : undefined;

  if (!issueStr || !workerId || !provider || !model || !session) {
    emitStatus("Usage: worker-pane.ts allocate --issue <N> --worker-id <id> --provider <p> --model <m> --session <s> [--split-pct 50]");
    return 2;
  }
  const identity: PaneIdentity = {
    issue: parseInt(issueStr, 10),
    workerId,
    provider,
    model,
    session,
  };
  const res = await allocatePane(identity, realTmuxDeps(), { splitPct });
  if (!res.ok) {
    emitJson({ ok: false, error: res.error });
    return 1;
  }
  emitStatus(`allocated pane ${res.pane!.paneId} for worker-${workerId} issue=#${issueStr}`);
  emitJson({ ok: true, pane_id: res.pane!.paneId, status_line: res.pane!.statusLine });
  return 0;
}

async function cmdStream(args: string[]): Promise<number> {
  const paneId = arg(args, "--pane-id");
  const command = arg(args, "--command");
  const issueStr = arg(args, "--issue");
  const workerId = arg(args, "--worker-id");
  const provider = arg(args, "--provider");
  const model = arg(args, "--model");
  const session = arg(args, "--session");
  if (!paneId || !command || !issueStr || !workerId || !provider || !model || !session) {
    emitStatus("Usage: worker-pane.ts stream --pane-id <%n> --command '<cmd>' --issue <N> --worker-id <id> --provider <p> --model <m> --session <s>");
    return 2;
  }
  // Reconstruct the in-process record so the streaming status line carries the real identity.
  const identity: PaneIdentity = { issue: parseInt(issueStr, 10), workerId, provider, model, session };
  const pane: PaneRecord = { paneId, identity, state: "allocated", statusLine: "", allocatedAt: Date.now() };
  const res = await streamLaunch(pane, command, realTmuxDeps());
  if (!res.ok) {
    emitJson({ ok: false, pane_id: paneId, error: res.error });
    return 1;
  }
  emitJson({ ok: true, pane_id: paneId, status_line: pane.statusLine });
  return 0;
}

async function cmdRelease(args: string[]): Promise<number> {
  const paneId = arg(args, "--pane-id");
  if (!paneId) {
    emitStatus("Usage: worker-pane.ts release --pane-id <%n>");
    return 2;
  }
  // Stateless release: identity optional, cosmetic status refresh skipped, pane killed.
  const pane: PaneRecord = { paneId, state: "allocated", statusLine: "", allocatedAt: Date.now() };
  const res = await releasePane(pane, realTmuxDeps());
  if (!res.ok) {
    emitJson({ ok: false, pane_id: paneId, error: res.error });
    return 1;
  }
  emitJson({ ok: true, pane_id: paneId, state: "released" });
  return 0;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const sub = args[0];
  const rest = args.slice(1);
  switch (sub) {
    case "allocate": return cmdAllocate(rest);
    case "stream": return cmdStream(rest);
    case "release": return cmdRelease(rest);
    default:
      emitStatus("Usage: worker-pane.ts (allocate | stream | release) ...");
      return 2;
  }
}

void DEFAULT_SPLIT_PCT;

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
