#!/usr/bin/env bun
// loop-notify.ts — /3ailoop の各 L ステップから 1 行通知を Discord Webhook へ流す
//
// 設計方針:
// - env DISCORD_WEBHOOK_URL 未設定なら silent skip (exit 0)。loop は環境に依存しない。
// - timeout 5s + 1 リトライ (429 のみ Retry-After を尊重)。それでも失敗なら諦める。
// - 例外・タイムアウト・HTTP error のいずれも exit 0。stderr に 1 行 warn を出すのみ。
//   loop 全体を絶対に止めないため。
//
// 使い方:
//   bun loop-notify.ts --kind <event-key> --text "<1 行メッセージ>"
//
// 引数:
//   --kind  イベント種別 (例: loop-stop / issue-start / issue-done / needs-human / token-limit)
//           現状は payload に含めないが、将来の分岐に備えて必須化。
//   --text  Discord に流す本文 (1 行)。
//
// Webhook URL の `?thread_id=<id>` クエリで投稿先スレッドを固定する想定 (env に含めて運用)。

const TIMEOUT_MS = 5000;
const MAX_LEN = 1900; // Discord content limit 2000 − 余白

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

function warn(msg: string): void {
  process.stderr.write(`[notify] ${msg}\n`);
}

async function postOnce(url: string, content: string): Promise<{ ok: boolean; status: number; retryAfterMs?: number; reason?: string }> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ content }),
      signal: ctrl.signal,
    });
    if (res.status === 429) {
      const ra = parseFloat(res.headers.get("retry-after") ?? "");
      const retryAfterMs = isFinite(ra) && ra > 0 ? Math.min(ra * 1000, 5000) : 1000;
      return { ok: false, status: 429, retryAfterMs, reason: "rate-limited" };
    }
    if (res.status >= 200 && res.status < 300) return { ok: true, status: res.status };
    return { ok: false, status: res.status, reason: `http ${res.status}` };
  } catch (e: any) {
    const name = e?.name === "AbortError" ? "timeout" : "fetch-error";
    return { ok: false, status: 0, reason: `${name}: ${e?.message ?? e}` };
  } finally {
    clearTimeout(timer);
  }
}

async function main(): Promise<void> {
  const kind = arg("--kind");
  const text = arg("--text");
  if (!kind || !text) {
    warn("usage: loop-notify.ts --kind <key> --text \"<msg>\"");
    process.exit(0); // exit 0 を維持 (loop を止めない)
    return;
  }

  const url = process.env.DISCORD_WEBHOOK_URL;
  if (!url) {
    warn("DISCORD_WEBHOOK_URL not set, skip");
    process.exit(0);
    return;
  }

  const content = text.length > MAX_LEN ? text.slice(0, MAX_LEN - 3) + "..." : text;

  const first = await postOnce(url, content);
  if (first.ok) {
    process.exit(0);
    return;
  }

  if (first.status === 429 && first.retryAfterMs) {
    await new Promise(r => setTimeout(r, first.retryAfterMs));
  }
  const second = await postOnce(url, content);
  if (second.ok) {
    process.exit(0);
    return;
  }
  warn(`notify-failed (kind=${kind}): ${second.reason ?? first.reason ?? "unknown"}`);
  process.exit(0);
}

if (import.meta.main) {
  await main();
}
