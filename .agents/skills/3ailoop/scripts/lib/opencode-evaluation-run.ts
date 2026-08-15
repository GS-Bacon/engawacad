import { appendFileSync } from "fs";
import { EvaluationDeadline, type EvaluationTimeoutConfig, type EvaluationTimeoutKind } from "./evaluation-timeout.ts";

export type InfrastructureKind = "rate-limit" | "upstream" | "connection" | "unavailable";

export type OpenCodeEvaluationRun = {
  code: number;
  timedOut: boolean;
  timeoutKind: EvaluationTimeoutKind | null;
  infrastructureFailure: boolean;
  infrastructureKind: InfrastructureKind | null;
  retryAfterMs: number | null;
  elapsed: number;
  text: string;
  sessionId: string | null;
  firstOutputMs: number | null;
  lastOutputMs: number | null;
};

export type OpenCodeEvaluationOptions = {
  model: string;
  root: string;
  prompt: string;
  title: string;
  timeouts: EvaluationTimeoutConfig;
  sessionId?: string;
  logPath?: string;
  onLine?: (line: string) => void;
};

type InfrastructureEvidence = {
  kind: InfrastructureKind;
  retryAfterMs: number | null;
};

function allText(value: unknown): string {
  try { return JSON.stringify(value); } catch { return String(value); }
}

export function classifyInfrastructureEvent(event: unknown): InfrastructureEvidence | null {
  const value = event as any;
  const source = value?.type === "session.error"
    ? value.properties?.error
    : value?.type === "session.status" && value.properties?.status?.type === "retry"
      ? value.properties.status
      : value?.type === "message.part.updated" && value.properties?.part?.type === "retry"
        ? value.properties.part.error
        : null;
  if (!source) return null;
  const text = allText(source);
  if (/\b429\b|FreeUsageLimitError|rate.?limit|free usage (?:exceeded|limit)|subscribe to Go/i.test(text)) {
    return { kind: "rate-limit", retryAfterMs: retryAfter(event) };
  }
  if (/\b50[234]\b|upstream (?:error|timeout)|temporar(?:y|ily) unavailable/i.test(text)) {
    return { kind: "upstream", retryAfterMs: retryAfter(event) };
  }
  if (/connection (?:reset|refused)|network error|socket connection was closed/i.test(text)) {
    return { kind: "connection", retryAfterMs: retryAfter(event) };
  }
  return null;
}

function retryAfter(event: unknown): number | null {
  const headers = (event as any)?.properties?.error?.data?.responseHeaders
    ?? (event as any)?.properties?.part?.error?.data?.responseHeaders
    ?? (event as any)?.error?.data?.responseHeaders;
  if (!headers || typeof headers !== "object") return null;
  const ms = Number(headers["retry-after-ms"]);
  if (Number.isFinite(ms) && ms >= 0) return ms;
  const value = headers["retry-after"];
  if (typeof value !== "string") return null;
  const seconds = Number(value);
  if (Number.isFinite(seconds) && seconds >= 0) return seconds * 1_000;
  const date = Date.parse(value);
  return Number.isFinite(date) ? Math.max(0, date - Date.now()) : null;
}

export function decodeSseChunk(buffer: string): { events: unknown[]; rest: string } {
  const blocks = buffer.replaceAll("\r\n", "\n").split("\n\n");
  const rest = blocks.pop() ?? "";
  const events: unknown[] = [];
  for (const block of blocks) {
    const data = block.split("\n")
      .filter(line => line.startsWith("data:"))
      .map(line => line.slice(5).trimStart())
      .join("\n");
    if (!data) continue;
    try { events.push(JSON.parse(data)); } catch { /* malformed server event is ignored */ }
  }
  return { events, rest };
}

export function normalizeOpenCodeEvent(event: any, sessionId: string): { line: string | null; modelActivity: boolean; terminal: "idle" | "error" | null } {
  const ownSession = event?.properties?.sessionID ?? event?.properties?.part?.sessionID ?? event?.properties?.info?.sessionID;
  if (ownSession && ownSession !== sessionId) return { line: null, modelActivity: false, terminal: null };
  if (event?.type === "message.part.updated") {
    const part = event.properties?.part;
    if (!part || part.sessionID !== sessionId) return { line: null, modelActivity: false, terminal: null };
    if (part.type === "reasoning") return { line: null, modelActivity: true, terminal: null };
    if (part.type === "step-start") return { line: JSON.stringify({ type: "step_start", sessionID: sessionId }), modelActivity: true, terminal: null };
    if (part.type === "tool") return { line: JSON.stringify({ type: "tool_use", sessionID: sessionId, part: { tool: part.tool, state: part.state } }), modelActivity: true, terminal: null };
    if (part.type === "text") return { line: JSON.stringify({ type: "text", sessionID: sessionId, part: { text: event.properties?.delta ?? part.text ?? "" } }), modelActivity: true, terminal: null };
    if (part.type === "retry") return { line: JSON.stringify({ type: "retry", sessionID: sessionId, attempt: part.attempt, error: part.error }), modelActivity: false, terminal: null };
  }
  if ((event?.type === "session.idle" || (event?.type === "session.status" && event.properties?.status?.type === "idle")) && event.properties?.sessionID === sessionId) {
    return { line: JSON.stringify({ type: "session_idle", sessionID: sessionId }), modelActivity: false, terminal: "idle" };
  }
  if (event?.type === "session.status" && event.properties?.sessionID === sessionId) {
    return { line: JSON.stringify({ type: "session_status", sessionID: sessionId, status: event.properties.status }), modelActivity: false, terminal: null };
  }
  if (event?.type === "session.error" && (!event.properties?.sessionID || event.properties.sessionID === sessionId)) {
    return { line: JSON.stringify({ type: "error", sessionID: sessionId, error: event.properties?.error }), modelActivity: false, terminal: "error" };
  }
  return { line: null, modelActivity: false, terminal: null };
}

function reservePort(): number {
  const listener = Bun.listen({
    hostname: "127.0.0.1",
    port: 0,
    socket: { data() {} },
  });
  const port = listener.port;
  listener.stop(true);
  return port;
}

async function jsonRequest(url: string, root: string, authorization: string, init: RequestInit): Promise<any> {
  const response = await fetch(url, {
    ...init,
    headers: { "content-type": "application/json", "x-opencode-directory": root, authorization, ...(init.headers ?? {}) },
  });
  if (!response.ok) throw new Error(`OpenCode server ${init.method ?? "GET"} ${new URL(url).pathname} failed: ${response.status} ${await response.text()}`);
  if (response.status === 204) return null;
  return response.json();
}

async function startServer(root: string): Promise<{ url: string; authorization: string; proc: ReturnType<typeof Bun.spawn> }> {
  const port = reservePort();
  const password = crypto.randomUUID();
  const authorization = `Basic ${btoa(`opencode:${password}`)}`;
  const proc = Bun.spawn(["opencode", "serve", "--hostname=127.0.0.1", `--port=${port}`], {
    cwd: root,
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, OPENCODE_SERVER_PASSWORD: password },
  });
  const url = `http://127.0.0.1:${port}`;
  const started = new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("OpenCode server startup timeout")), 10_000);
    const consume = async (stream: ReadableStream<Uint8Array>) => {
      const reader = stream.getReader();
      let pending = "";
      while (true) {
        const item = await reader.read();
        if (item.done) break;
        pending += new TextDecoder().decode(item.value);
        for (const line of pending.split("\n").slice(0, -1)) {
          if (line.includes("opencode server listening")) {
            clearTimeout(timer);
            resolve(url);
          }
        }
        pending = pending.split("\n").at(-1) ?? "";
      }
    };
    void consume(proc.stdout);
    void consume(proc.stderr);
    void proc.exited.then(code => {
      clearTimeout(timer);
      reject(new Error(`OpenCode server exited during startup: ${code}`));
    });
  });
  try {
    await started;
    return { url, authorization, proc };
  } catch (error) {
    proc.kill();
    await proc.exited.catch(() => undefined);
    throw error;
  }
}

export async function runOpenCodeEvaluation(options: OpenCodeEvaluationOptions): Promise<OpenCodeEvaluationRun> {
  const startedAt = Date.now();
  const server = await startServer(options.root);
  const eventAbort = new AbortController();
  const requestAbort = new AbortController();
  let text = "";
  let firstOutputMs: number | null = null;
  let lastOutputMs: number | null = null;
  let timeoutKind: EvaluationTimeoutKind | null = null;
  let infra: InfrastructureEvidence | null = null;
  let terminalCode: number | null = null;
  let finish!: () => void;
  const finished = new Promise<void>(resolve => { finish = resolve; });
  let sessionId = options.sessionId ?? null;
  const emit = (line: string) => {
    text += line + "\n";
    if (options.logPath) appendFileSync(options.logPath, line + "\n");
    options.onLine?.(line);
  };
  const deadline = new EvaluationDeadline(options.timeouts, kind => {
    timeoutKind = kind;
    terminalCode = 143;
    requestAbort.abort();
    finish();
  });

  try {
    const eventResponse = await fetch(`${server.url}/event?directory=${encodeURIComponent(options.root)}`, {
      headers: { "x-opencode-directory": options.root, authorization: server.authorization },
      signal: eventAbort.signal,
    });
    if (!eventResponse.ok || !eventResponse.body) throw new Error(`OpenCode event stream failed: ${eventResponse.status}`);
    const eventTask = (async () => {
      const reader = eventResponse.body!.getReader();
      let pending = "";
      while (true) {
        const item = await reader.read();
        if (item.done) break;
        pending += new TextDecoder().decode(item.value);
        const decoded = decodeSseChunk(pending);
        pending = decoded.rest;
        for (const event of decoded.events) {
          if (!sessionId) continue;
          const normalized = normalizeOpenCodeEvent(event, sessionId);
          if (normalized.line) emit(normalized.line);
          if (normalized.modelActivity) {
            const elapsed = Date.now() - startedAt;
            if (firstOutputMs === null) firstOutputMs = elapsed;
            lastOutputMs = elapsed;
            deadline.activity();
          }
          const evidence = classifyInfrastructureEvent(event);
          if (evidence) {
            infra = evidence;
            terminalCode = 75;
            requestAbort.abort();
            finish();
            return;
          }
          if (normalized.terminal) {
            terminalCode = normalized.terminal === "idle" ? 0 : 1;
            requestAbort.abort();
            finish();
            return;
          }
        }
      }
      if (terminalCode === null) {
        terminalCode = 1;
        infra = { kind: "connection", retryAfterMs: null };
        finish();
      }
    })().catch(error => {
      if (!eventAbort.signal.aborted) {
        emit(JSON.stringify({ type: "error", sessionID, error: { message: String(error) } }));
        terminalCode = 1;
        infra = { kind: "connection", retryAfterMs: null };
        finish();
      }
    });

    if (!sessionId) {
      const created = await jsonRequest(`${server.url}/session?directory=${encodeURIComponent(options.root)}`, options.root, server.authorization, {
        method: "POST",
        body: JSON.stringify({ title: options.title }),
      });
      sessionId = created?.id;
      if (!sessionId) throw new Error("OpenCode server did not return a session id");
    }
    const [providerID, ...modelParts] = options.model.split("/");
    if (!providerID || modelParts.length === 0) throw new Error(`invalid model identity: ${options.model}`);
    deadline.start();
    try {
      await jsonRequest(`${server.url}/session/${encodeURIComponent(sessionId)}/prompt_async?directory=${encodeURIComponent(options.root)}`, options.root, server.authorization, {
        method: "POST",
        signal: requestAbort.signal,
        body: JSON.stringify({
          model: { providerID, modelID: modelParts.join("/") },
          agent: "build",
          parts: [{ type: "text", text: options.prompt }],
        }),
      });
    } catch (error) {
      if (timeoutKind === null && terminalCode === null) throw error;
    }
    await finished;
    deadline.stop();
    if (terminalCode !== 0) {
      await fetch(`${server.url}/session/${encodeURIComponent(sessionId)}/abort?directory=${encodeURIComponent(options.root)}`, {
        method: "POST",
        headers: { "content-type": "application/json", "x-opencode-directory": options.root, authorization: server.authorization },
      }).catch(() => undefined);
    }
    eventAbort.abort();
    await eventTask;
  } finally {
    deadline.stop();
    eventAbort.abort();
    requestAbort.abort();
    server.proc.kill();
    await server.proc.exited.catch(() => undefined);
  }

  return {
    code: terminalCode ?? 1,
    timedOut: timeoutKind !== null,
    timeoutKind,
    infrastructureFailure: infra !== null || timeoutKind === "first-output",
    infrastructureKind: infra?.kind ?? (timeoutKind === "first-output" ? "unavailable" : null),
    retryAfterMs: infra?.retryAfterMs ?? null,
    elapsed: Date.now() - startedAt,
    text,
    sessionId,
    firstOutputMs,
    lastOutputMs,
  };
}
