#!/usr/bin/env bun
// model-retire.ts — model の retirement と report 出力 CLI (policy #8)
//
// 使い方:
//   bun model-retire.ts --identity <provider/model> --reason "<reason>" [--write-report]
//
// 流れ:
//   1. model-registry の該当 entry を retired にする。
//   2. --write-report 指定時、features/.loop/model-retirements.jsonl に report を追記。
//
// 出力 (stdout JSON): RetirementReport or { error }
// exit: 0 / 1 (not found) / 2 (usage)

import { readModelRegistry, mutateModelRegistry, retire, appendRetirementReport, MODEL_REGISTRY_PATHS } from "./lib/model-registry.ts";
import { arg, emitJson, emitStatus } from "./cli/helpers.ts";

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  const identity = arg(args, "--identity");
  const reason = arg(args, "--reason") ?? "unspecified";
  const writeReport = args.includes("--write-report");
  const registryPath = arg(args, "--registry-path") ?? MODEL_REGISTRY_PATHS.DEFAULT_REGISTRY_PATH;
  const reportPath = MODEL_REGISTRY_PATHS.DEFAULT_REPORT_PATH;

  if (!identity) {
    emitStatus("Usage: model-retire.ts --identity <provider/model> --reason \"...\" [--write-report]");
    return 2;
  }

  const reg = readModelRegistry(registryPath);
  if (!reg.models[identity]) {
    emitJson({ error: "not-found", identity });
    return 1;
  }

  let report: ReturnType<typeof retire>["report"] = null;
  const registry = mutateModelRegistry(registryPath, r => {
    const result = retire(r, identity, reason, () => new Date());
    report = result.report;
    return result.registry;
  });

  if (writeReport && report) {
    appendRetirementReport(report, reportPath);
    emitStatus(`retired ${identity}; report appended to ${reportPath}`);
  } else {
    emitStatus(`retired ${identity}`);
  }
  emitJson(report ?? { identity, retired: true });
  void registry;
  return 0;
}

if (import.meta.main) {
  main().then(c => process.exit(c)).catch(e => {
    console.error(e);
    process.exit(2);
  });
}
