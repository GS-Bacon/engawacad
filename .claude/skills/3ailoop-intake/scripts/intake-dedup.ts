#!/usr/bin/env bun
// intake-dedup.ts — 要望文に類似する Issue を検索して Top-N 出力
//
// 類似度: タイトル + body 冒頭 200 字 と要望文の単語 overlap 比率
// 検索範囲: open Issue + 直近 30 日 close
//
// 使い方:
//   bun intake-dedup.ts --query "要望文" [--limit N]

async function runGh(args: string[]): Promise<string> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

function tokenize(text: string): Set<string> {
  // 日本語: 2-gram 風 / 英数: word 分割
  const ja = text.match(/[぀-ヿ㐀-鿿]{2,}/g) ?? [];
  const en = text.toLowerCase().match(/[a-z][a-z0-9]{2,}/g) ?? [];
  const tokens = new Set<string>();
  for (const w of [...ja, ...en]) tokens.add(w);
  // 日本語をさらに 2-gram に
  for (const seg of ja) {
    for (let i = 0; i + 2 <= seg.length; i++) tokens.add(seg.slice(i, i + 2));
  }
  return tokens;
}

function jaccard(a: Set<string>, b: Set<string>): number {
  if (a.size === 0 || b.size === 0) return 0;
  let inter = 0;
  for (const x of a) if (b.has(x)) inter++;
  const union = a.size + b.size - inter;
  return union > 0 ? inter / union : 0;
}

interface IssueRaw {
  number: number;
  title: string;
  body: string;
  state: string;
  closedAt: string | null;
}

async function fetchCandidates(): Promise<IssueRaw[]> {
  // open
  const openOut = await runGh([
    "issue", "list", "--state", "open", "--limit", "200",
    "--json", "number,title,body,state,closedAt",
  ]);
  // closed (30 日以内)
  const dt = new Date(Date.now() - 30 * 24 * 3600 * 1000).toISOString().slice(0, 10);
  const closedOut = await runGh([
    "issue", "list", "--state", "closed", "--limit", "100",
    "--search", `closed:>=${dt}`,
    "--json", "number,title,body,state,closedAt",
  ]);
  try {
    const open = JSON.parse(openOut) as IssueRaw[];
    const closed = JSON.parse(closedOut) as IssueRaw[];
    return [...open, ...closed];
  } catch {
    return [];
  }
}

async function main() {
  const args = process.argv.slice(2);
  let query = "";
  let limit = 5;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--query") query = args[++i] ?? "";
    else if (args[i] === "--limit") limit = parseInt(args[++i] ?? "5");
  }
  if (!query) {
    console.error("Usage: intake-dedup.ts --query <text> [--limit N]");
    process.exit(2);
  }
  const queryTokens = tokenize(query);

  const candidates = await fetchCandidates();
  const scored = candidates.map(c => {
    const body = (c.body ?? "").slice(0, 200);
    const text = c.title + " " + body;
    const score = jaccard(queryTokens, tokenize(text));
    return { number: c.number, title: c.title, state: c.state, score: Math.round(score * 1000) / 1000 };
  });

  scored.sort((a, b) => b.score - a.score);
  const top = scored.filter(s => s.score > 0).slice(0, limit);
  console.log(JSON.stringify(top, null, 2));
}

main().catch(e => {
  console.error(`ERROR: ${(e as Error).message}`);
  process.exit(1);
});
