import { appendFileSync, writeFileSync, readFileSync, existsSync } from "fs";

const LOG_FILE = "/tmp/engawa-action.log";

if (!existsSync(LOG_FILE)) writeFileSync(LOG_FILE, "");

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, DELETE, OPTIONS",
};

Bun.serve({
  port: 7879,
  async fetch(req) {
    if (req.method === "OPTIONS") return new Response(null, { status: 204, headers: CORS });

    const url = new URL(req.url);
    if (url.pathname !== "/log") return new Response("not found", { status: 404 });

    if (req.method === "GET") {
      const content = existsSync(LOG_FILE) ? readFileSync(LOG_FILE, "utf-8") : "";
      return new Response(content, { headers: { ...CORS, "Content-Type": "text/plain" } });
    }

    if (req.method === "DELETE") {
      writeFileSync(LOG_FILE, "");
      return new Response("cleared", { headers: CORS });
    }

    if (req.method === "POST") {
      const line = await req.text();
      appendFileSync(LOG_FILE, line + "\n");
      return new Response("ok", { headers: CORS });
    }

    return new Response("method not allowed", { status: 405 });
  },
});

console.log(`Action log collector: http://localhost:7879/log  →  ${LOG_FILE}`);
