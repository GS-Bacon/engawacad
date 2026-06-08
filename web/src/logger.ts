export type LogEntry = {
  ts: number;
  type: string;
  data?: Record<string, unknown>;
};

const _entries: LogEntry[] = [];

const COLLECTOR = "http://127.0.0.1:7879/log";

function send(entry: LogEntry): void {
  fetch(COLLECTOR, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(entry),
    keepalive: true,
  }).catch(() => {}); // サイドカー未起動でも無視
}

export function log(type: string, data?: Record<string, unknown>): void {
  const entry: LogEntry = { ts: Date.now(), type, ...(data !== undefined ? { data } : {}) };
  _entries.push(entry);
  send(entry);
}

export function clearLog(): void {
  _entries.length = 0;
  fetch(COLLECTOR, { method: "DELETE" }).catch(() => {});
}

export function getEntries(): readonly LogEntry[] {
  return _entries;
}

export function formatLog(): string {
  return _entries
    .map((e) => {
      const t = new Date(e.ts).toISOString().replace("T", " ").slice(0, -1);
      const d = e.data !== undefined ? " " + JSON.stringify(e.data) : "";
      return `${t} ${e.type}${d}`;
    })
    .join("\n");
}
