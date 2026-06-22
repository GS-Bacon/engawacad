#!/usr/bin/env bun
// glm-via-zai.ts — Z.AI 経由で `claude -p` を起動する低レベル primitive (#281)
//
// 役割:
// - Z.AI API key を `.env` ファイルから読む
// - ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN を切り替えて `claude -p` を spawn
// - stdout 末尾の JSON から `result` フィールドを抽出して返す
//
// 利用箇所:
// - loop-adr-auto-accept.ts (#251 で導入された ADR 3 persona GLM fallback)
// - dispatch-codex-3persona.ts (#281 で導入する STEP 7.5-B GLM 3 persona fallback)
//
// 各 caller は本モジュールの返す result を独自に解釈する (ADR は verdict regex、3p は
// yaml issues/verdict block の抽出)。本モジュールは「Z.AI 経由 claude -p の生 stdout
// から JSON の result を取り出すところまで」を共通化する。

import { existsSync, readFileSync } from "fs";
import { runChecked } from "../../3ailoop/scripts/loop-spawn-checked.ts";

export interface GlmViaZaiOpts {
  /** claude -p に渡す prompt 文字列。 */
  prompt: string;
  /** Z.AI 認証情報の .env ファイル。省略時は ZAI_ENV 環境変数 → ~/AutoClaudeKMP/.env の順。 */
  envFilePath?: string;
  /** claude -p の --max-turns。省略時 3。 */
  maxTurns?: number;
  /** Z.AI 呼び出しの API_TIMEOUT_MS。省略時 600000 (10 分)。 */
  apiTimeoutMs?: number;
}

export interface GlmViaZaiResult {
  /** claude -p が exit 0 で result が抽出できたら true。それ以外 (env 不在 / spawn 失敗 / JSON 取れず) false。 */
  ok: boolean;
  /** spawn の exit code。env 不在等で spawn 前に失敗したら -1。 */
  exitCode: number;
  /** spawn の生 stdout。 */
  stdout: string;
  /** spawn の生 stderr。 */
  stderr: string;
  /** stdout 末尾の JSON から取り出した "result" 文字列。失敗時は ""。 */
  result: string;
  /** 不可能系の理由 (env 不在 / Z_AI_API_KEY missing 等)。spawn 失敗時は exit/stderr に詰まる。 */
  error?: string;
}

/** `.env` 形式のファイルを読み KEY=VAL の Record に変換する。コメント行と空行はスキップ、
 *  値の前後の単一/二重引用符は剥がす。 */
export function parseEnvFile(path: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of readFileSync(path, "utf-8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx === -1) continue;
    const key = trimmed.slice(0, eqIdx).trim();
    let val = trimmed.slice(eqIdx + 1).trim();
    if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
      val = val.slice(1, -1);
    }
    result[key] = val;
  }
  return result;
}

/** Z.AI 経由で `claude -p <prompt>` を実行し、stdout 末尾の JSON から `result` を取り出して返す。 */
export async function runGlmViaZAI(opts: GlmViaZaiOpts): Promise<GlmViaZaiResult> {
  const envFilePath =
    opts.envFilePath ?? process.env.ZAI_ENV ?? `${process.env.HOME}/AutoClaudeKMP/.env`;

  if (!existsSync(envFilePath)) {
    return {
      ok: false,
      exitCode: -1,
      stdout: "",
      stderr: "",
      result: "",
      error: `Z.AI env not found at ${envFilePath}`,
    };
  }
  const envVars = parseEnvFile(envFilePath);
  if (!envVars.Z_AI_API_KEY) {
    return {
      ok: false,
      exitCode: -1,
      stdout: "",
      stderr: "",
      result: "",
      error: `Z_AI_API_KEY missing in ${envFilePath}`,
    };
  }

  const glmEnv: Record<string, string> = {
    ANTHROPIC_BASE_URL: "https://api.z.ai/api/anthropic",
    ANTHROPIC_AUTH_TOKEN: envVars.Z_AI_API_KEY,
    ANTHROPIC_DEFAULT_OPUS_MODEL: "glm-4.6",
    ANTHROPIC_DEFAULT_SONNET_MODEL: "glm-4.6",
    ANTHROPIC_DEFAULT_HAIKU_MODEL: "glm-4.6",
    API_TIMEOUT_MS: String(opts.apiTimeoutMs ?? 600000),
  };
  // CLAUDECODE が立っていると claude CLI が外側のセッションを引き継いでしまうため削る。
  // runChecked は process.env を base にして env を merge するので、削除指示を空文字で代用。
  // (Bun.spawn は env: { ... } を渡すと process.env を継承しないが、loop-spawn-checked は
  //  process.env をスプレッドする。よって明示的に空文字を入れて CLAUDECODE を打ち消す。)
  glmEnv.CLAUDECODE = "";

  const maxTurns = String(opts.maxTurns ?? 3);
  const r = await runChecked(
    ["claude", "-p", opts.prompt, "--max-turns", maxTurns, "--output-format", "json"],
    { allowFailure: true, env: glmEnv },
  );

  if (r.exitCode !== 0) {
    return {
      ok: false,
      exitCode: r.exitCode,
      stdout: r.stdout,
      stderr: r.stderr,
      result: "",
    };
  }

  // stdout の末尾から JSON 行を遡って探し、"result" フィールドを拾う。
  // claude -p --output-format=json は最後に {result, ...} を 1 行 JSON で吐く想定。
  let result = "";
  const lines = r.stdout.trim().split("\n").reverse();
  for (const line of lines) {
    try {
      const obj = JSON.parse(line);
      if (typeof obj === "object" && obj !== null && "result" in obj) {
        result = String(obj.result);
        break;
      }
    } catch {
      // 非 JSON 行はスキップ
    }
  }

  return {
    ok: result !== "",
    exitCode: r.exitCode,
    stdout: r.stdout,
    stderr: r.stderr,
    result,
  };
}
