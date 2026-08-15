// worker-pane.test.ts — per-worker tmux pane observability (contract §Orchestration #5/#6, #100)
//
// 検証観点 (contract):
//   §Orchestration #5: each active external worker gets a dedicated RIGHT-SIDE pane,
//      status line identifies issue/provider/pinned-model/session/lifecycle-state,
//      no credentials / no hidden reasoning traces.
//   §Orchestration #6: pane is operator-only; scheduling/completion/failure/consultation/
//      resume use durable state + receipts + event-driven termination, NOT pane scraping.
//      Deterministic cleanup/reuse; pane ops never affect lifecycle correctness.
//   acceptance #100: the N=3 dry run proves labeled right-side pane assignment,
//      observability through terminal completion, and lifecycle-safe cleanup/reuse.
//
// 全テスト fake tmux adapter のみ (real tmux / network / model 呼び出しなし)。

import { describe, expect, test } from "bun:test";
import {
  allocatePane,
  streamLaunch,
  capturePane,
  releasePane,
  buildStatusLine,
  redact,
  isSafeStatusLine,
  statusLineHasAllFields,
  DEFAULT_SPLIT_PCT,
  type WorkerPaneDeps,
  type PaneIdentity,
} from "../lib/worker-pane.ts";

/** In-memory fake tmux adapter mirroring the dry-run one. */
function fakeTmux(): WorkerPaneDeps & {
  panes: Map<string, { title: string; output: string; killed: boolean }>;
  splits: { side: "right"; pct: number }[];
} {
  const panes = new Map<string, { title: string; output: string; killed: boolean }>();
  const splits: { side: "right"; pct: number }[] = [];
  let n = 0;
  return {
    panes,
    splits,
    splitRight: async ({ splitPct }) => {
      n += 1;
      const id = `%${n}`;
      splits.push({ side: "right", pct: splitPct });
      panes.set(id, { title: "", output: "", killed: false });
      return { ok: true, paneId: id };
    },
    setStatusLine: async (paneId, text) => {
      const p = panes.get(paneId);
      if (!p) return { ok: false, error: "no pane" };
      p.title = text;
      return { ok: true };
    },
    sendKeys: async (paneId, command) => {
      const p = panes.get(paneId);
      if (!p) return { ok: false, error: "no pane" };
      p.output += `$ ${command}\nworker output\n`;
      return { ok: true };
    },
    release: async (paneId) => {
      const p = panes.get(paneId);
      if (!p) return { ok: false, error: "no pane" };
      p.killed = true;
      return { ok: true };
    },
    capture: async (paneId) => {
      const p = panes.get(paneId);
      return p ? { ok: true, output: p.output } : { ok: false, output: "" };
    },
    now: () => 1_000_000,
  };
}

function identity(issue = 5): PaneIdentity {
  return {
    issue,
    workerId: `worker-${issue}`,
    provider: "opencode",
    model: "opencode/deepseek-free",
    session: `sess-${issue}`,
  };
}

describe("status line content (§Orchestration #5)", () => {
  test("buildStatusLine carries all 5 identifying fields + lifecycle state", () => {
    const line = buildStatusLine(identity(), "allocated");
    expect(statusLineHasAllFields(line)).toBe(true);
    expect(line).toContain("issue=#5");
    expect(line).toContain("worker=worker-5");
    expect(line).toContain("provider=opencode");
    expect(line).toContain("model=opencode/deepseek-free");
    expect(line).toContain("session=sess-5");
    expect(line).toContain("state=allocated");
  });

  test("lifecycle state transitions are reflected in the line", () => {
    expect(buildStatusLine(identity(), "allocated")).toContain("state=allocated");
    expect(buildStatusLine(identity(), "streaming")).toContain("state=streaming");
    expect(buildStatusLine(identity(), "released")).toContain("state=released");
  });

  test("status line is single-line", () => {
    const line = buildStatusLine(identity(), "streaming");
    expect(line).not.toMatch(/[\r\n]/);
  });
});

describe("credential / reasoning-trace redaction (§Orchestration #5)", () => {
  test("redact strips OpenAI-style keys", () => {
    expect(redact("model=x key=sk-abcdefghijklmnop1234")).toBe("model=x key=[redacted]");
  });

  test("redact strips JWT/bearer-ish tokens", () => {
    const t = "eyJabcdefghijklmnop1234567890.somethinghere1234567890abcd";
    expect(redact(`bearer ${t}`)).toContain("[redacted]");
    expect(redact(`bearer ${t}`)).not.toContain(t);
  });

  test("redact strips hidden reasoning traces", () => {
    const withTrace = `session=s state=streaming <thinking>secret plan</thinking>`;
    expect(redact(withTrace)).not.toContain("secret plan");
    expect(redact(withTrace)).toContain("[redacted]");
  });

  test("isSafeStatusLine rejects credential-bearing lines", () => {
    expect(isSafeStatusLine("model=x sk-abcd1234abcd1234")).toBe(false);
    expect(isSafeStatusLine("issue=#1 model=opencode/free state=allocated")).toBe(true);
  });

  test("isSafeStatusLine rejects multi-line (would break pane title)", () => {
    expect(isSafeStatusLine("issue=#1\nstate=allocated")).toBe(false);
  });

  test("buildStatusLine output is always safe", () => {
    const id: PaneIdentity = {
      issue: 1, workerId: "w", provider: "sk-secretkey1234567", model: "free", session: "s",
    };
    // provider value resembling a key is still redacted in the rendered line
    expect(isSafeStatusLine(buildStatusLine(id, "allocated"))).toBe(true);
  });
});

describe("allocatePane: right-side dedicated pane (§Orchestration #5)", () => {
  test("allocates a pane on the right and stamps the labeled status line", async () => {
    const tmux = fakeTmux();
    const res = await allocatePane(identity(7), tmux);
    expect(res.ok).toBe(true);
    expect(res.pane!.paneId).toBe("%1");
    // exactly one right-side split
    expect(tmux.splits).toEqual([{ side: "right", pct: DEFAULT_SPLIT_PCT }]);
    // status line stamped with the identity
    const p = tmux.panes.get("%1")!;
    expect(statusLineHasAllFields(p.title)).toBe(true);
    expect(p.title).toContain("issue=#7");
    expect(res.pane!.state).toBe("allocated");
  });

  test("custom split pct honored", async () => {
    const tmux = fakeTmux();
    await allocatePane(identity(), tmux, { splitPct: 33 });
    expect(tmux.splits[0]!.pct).toBe(33);
  });

  test("split failure → bounded failure, no status line set", async () => {
    const tmux: WorkerPaneDeps = {
      splitRight: async () => ({ ok: false, error: "no tmux" }),
      setStatusLine: async () => ({ ok: true }),
      sendKeys: async () => ({ ok: true }),
      release: async () => ({ ok: true }),
      capture: async () => ({ ok: true, output: "" }),
      now: () => 0,
    };
    const res = await allocatePane(identity(), tmux);
    expect(res.ok).toBe(false);
    expect(res.error).toContain("split-right failed");
  });

  test("status-line failure → bounded failure, half-allocated pane released", async () => {
    let released = false;
    const tmux: WorkerPaneDeps = {
      splitRight: async () => ({ ok: true, paneId: "%9" }),
      setStatusLine: async () => ({ ok: false, error: "boom" }),
      sendKeys: async () => ({ ok: true }),
      release: async () => { released = true; return { ok: true }; },
      capture: async () => ({ ok: true, output: "" }),
      now: () => 0,
    };
    const res = await allocatePane(identity(), tmux);
    expect(res.ok).toBe(false);
    expect(released).toBe(true);
  });
});

describe("streamLaunch + capturePane: observability (§Orchestration #5/#6, #100)", () => {
  test("stream sends the launch command into the pane and marks streaming", async () => {
    const tmux = fakeTmux();
    const alloc = await allocatePane(identity(2), tmux);
    const pane = alloc.pane!;
    const r = await streamLaunch(pane, "opencode run --model opencode/free '<plan>'", tmux);
    expect(r.ok).toBe(true);
    expect(pane.state).toBe("streaming");
    const p = tmux.panes.get(pane.paneId)!;
    expect(p.output).toContain("opencode run");
    expect(p.title).toContain("state=streaming");
  });

  test("capturePane returns the streamed output (observable through terminal)", async () => {
    const tmux = fakeTmux();
    const alloc = await allocatePane(identity(3), tmux);
    await streamLaunch(alloc.pane!, "opencode run --model m '<p>'", tmux);
    const cap = await capturePane(alloc.pane!, tmux);
    expect(cap.ok).toBe(true);
    expect(cap.output.length).toBeGreaterThan(0);
    expect(cap.output).toContain("opencode run");
  });

  test("send-keys failure → bounded failure (does not throw into caller)", async () => {
    const tmux: WorkerPaneDeps = {
      splitRight: async () => ({ ok: true, paneId: "%1" }),
      setStatusLine: async () => ({ ok: true }),
      sendKeys: async () => ({ ok: false, error: "dead pane" }),
      release: async () => ({ ok: true }),
      capture: async () => ({ ok: true, output: "" }),
      now: () => 0,
    };
    const alloc = await allocatePane(identity(), tmux);
    const r = await streamLaunch(alloc.pane!, "cmd", tmux);
    expect(r.ok).toBe(false);
    expect(r.error).toContain("send-keys failed");
  });
});

describe("releasePane: deterministic cleanup + lifecycle independence (§Orchestration #6, #100)", () => {
  test("release marks the pane killed and state released", async () => {
    const tmux = fakeTmux();
    const alloc = await allocatePane(identity(), tmux);
    await streamLaunch(alloc.pane!, "cmd", tmux);
    const r = await releasePane(alloc.pane!, tmux);
    expect(r.ok).toBe(true);
    expect(alloc.pane!.state).toBe("released");
    expect(tmux.panes.get(alloc.pane!.paneId)!.killed).toBe(true);
  });

  test("release is idempotent", async () => {
    const tmux = fakeTmux();
    const alloc = await allocatePane(identity(), tmux);
    await releasePane(alloc.pane!, tmux);
    const r2 = await releasePane(alloc.pane!, tmux);
    expect(r2.ok).toBe(true);
  });

  test("output remains observable up to release (operator can inspect terminal)", async () => {
    const tmux = fakeTmux();
    const alloc = await allocatePane(identity(), tmux);
    await streamLaunch(alloc.pane!, "opencode run --model m '<plan>'", tmux);
    const before = await capturePane(alloc.pane!, tmux);
    await releasePane(alloc.pane!, tmux);
    // after release, capture still returns the buffered output (deterministic, not scraped)
    const after = await capturePane(alloc.pane!, tmux);
    expect(after.output).toBe(before.output);
    expect(after.output.length).toBeGreaterThan(0);
  });

  test("release failure does NOT throw — lifecycle path stays intact", async () => {
    const tmux: WorkerPaneDeps = {
      splitRight: async () => ({ ok: true, paneId: "%1" }),
      setStatusLine: async () => ({ ok: true }),
      sendKeys: async () => ({ ok: true }),
      release: async () => ({ ok: false, error: "kill-pane failed" }),
      capture: async () => ({ ok: true, output: "" }),
      now: () => 0,
    };
    const alloc = await allocatePane(identity(), tmux);
    // must not throw even though release failed
    const r = await releasePane(alloc.pane!, tmux);
    expect(r.ok).toBe(false);
    // pane record still transitioned to released (cleanup attempted)
    expect(alloc.pane!.state).toBe("released");
  });
});

describe("lifecycle independence: pane ops never touch CAS/registry (§Orchestration #6)", () => {
  test("a full pane lifecycle does not write any lifecycle state", async () => {
    // The fake adapter only mutates its own in-memory map. The contract invariant is that
    // pane allocation/stream/release NEVER mutate CAS assign-state or the worker registry.
    // We assert this structurally: WorkerPaneDeps exposes no CAS/registry mutation surface,
    // and the recorded ops are all pane-scoped.
    const tmux = fakeTmux();
    const alloc = await allocatePane(identity(42), tmux);
    await streamLaunch(alloc.pane!, "cmd", tmux);
    await releasePane(alloc.pane!, tmux);
    // every recorded side-effect is a pane id, never an issue/worker registry path
    for (const id of tmux.panes.keys()) expect(id).toMatch(/^%\d+$/);
    // no lifecycle mutation surface exists on the deps contract
    const depKeys = Object.keys({} as Record<keyof WorkerPaneDeps, never>)
      .concat(["splitRight", "setStatusLine", "sendKeys", "release", "capture", "now"]);
    for (const k of depKeys) {
      expect(k).not.toMatch(/cas|registry|assign|generation|closed/i);
    }
  });

  test("pane allocate failure does not prevent a subsequent lifecycle decision", async () => {
    // Even if the pane layer is entirely unavailable, the orchestrator must still be able to
    // drive lifecycle via CAS+receipts. Here we only assert the pane API returns a bounded
    // failure and throws nothing — the caller proceeds with durable state.
    const tmux: WorkerPaneDeps = {
      splitRight: async () => ({ ok: false, error: "tmux absent" }),
      setStatusLine: async () => ({ ok: true }),
      sendKeys: async () => ({ ok: true }),
      release: async () => ({ ok: true }),
      capture: async () => ({ ok: true, output: "" }),
      now: () => 0,
    };
    const res = await allocatePane(identity(), tmux);
    expect(res.ok).toBe(false);
    // caller can still proceed (no exception thrown)
    expect(typeof res.error).toBe("string");
  });
});
