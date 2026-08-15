#!/usr/bin/env bun
// model-discover.ts — OpenCode free-model discovery CLI
//
// 使い方:
//   bun model-discover.ts [--no-network] [--raw]
//
// --no-network: opencode を呼ばず、空結果を返す (offline test 用)。
// --raw:        parse 前の opencode stdout も dump (debug)。
//
// 出力 (stdout JSON): discover.ts の DiscoveryResult
//   { eligible: [...], rejected: [...], opencodeSetExhausted: bool }
// Only the OpenCode provider is queried. Other providers belong to the explicit fallback gate.
//
// 関連: lib/discover.ts, contract §"Free-model policy"

import { discover } from "./lib/discover.ts";
import { runCmd, emitJson, emitStatus } from "./cli/helpers.ts";

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const noNetwork = args.includes("--no-network");
  const raw = args.includes("--raw");

  const res = await discover({
    runOpenCode: async (cliArgs) => {
      if (noNetwork) return { ok: false, stdout: "", stderr: "no-network" };
      const r = await runCmd(["opencode", ...cliArgs]);
      if (raw && r.ok) process.stderr.write("--- raw opencode stdout ---\n" + r.stdout + "\n");
      return r;
    },
    now: () => new Date(),
  });

  emitStatus(`discovery: eligible=${res.eligible.length} rejected=${res.rejected.length} exhausted=${res.opencodeSetExhausted}`);
  emitJson(res);
  return 0;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
