// discover.test.ts — OpenCode free-model discovery + filtering + rejection
//
// 検証観点 (contract acceptance):
//   - model filtering and priority
//   - explicit local/Gemini rejection
//   - nonzero/unknown price rejection
//   - inactive / no-toolcall rejection
//   - opencodeSetExhausted when all rejected

import { describe, expect, test } from "bun:test";
import {
  evaluate,
  filterModels,
  isFreeCost,
  detectForbidden,
  discover,
  parseOpenCodeModels,
  type OpenCodeModel,
} from "../lib/discover.ts";

const now = () => new Date("2026-08-13T00:00:00Z");

function model(p: Partial<OpenCodeModel>): OpenCodeModel {
  return {
    id: p.id ?? "x",
    providerID: p.providerID ?? "opencode",
    name: p.name ?? "X",
    status: p.status ?? "active",
    cost: p.cost ?? { input: 0, output: 0 },
    capabilities: p.capabilities ?? { toolcall: true },
    ...p,
  } as OpenCodeModel;
}

describe("free-model filter", () => {
  test("active + zero cost + toolcall → eligible", () => {
    const r = evaluate(model({ id: "deepseek-v4-flash-free", providerID: "opencode" }), now);
    expect("identity" in r).toBe(true);
    if ("identity" in r) {
      expect(r.identity).toBe("opencode/deepseek-v4-flash-free");
      expect(r.evidence.input_cost).toBe(0);
      expect(r.evidence.output_cost).toBe(0);
      expect(r.evidence.toolcall).toBe(true);
    }
  });

  test("nonzero cost → rejected nonzero-cost", () => {
    const r = evaluate(model({ cost: { input: 0.001, output: 0 } }), now);
    expect("reason" in r && r.reason).toBe("nonzero-cost");
  });

  test("unknown cost (undefined fields) → rejected unknown-cost (unknown is NOT free)", () => {
    const r = evaluate(model({ cost: undefined }), now);
    expect("reason" in r && r.reason).toBe("unknown-cost");
    const r2 = evaluate(model({ cost: { input: 0, output: undefined as unknown as number } }), now);
    expect("reason" in r2 && r2.reason).toBe("unknown-cost");
  });

  test("inactive status → rejected inactive", () => {
    const r = evaluate(model({ status: "inactive" }), now);
    expect("reason" in r && r.reason).toBe("inactive");
  });

  test("no toolcall → rejected no-toolcall", () => {
    const r = evaluate(model({ capabilities: { toolcall: false } }), now);
    expect("reason" in r && r.reason).toBe("no-toolcall");
  });
});

describe("explicit forbidden rejection", () => {
  test("Ollama → forbidden-local", () => {
    expect(detectForbidden(model({ id: "llama3", providerID: "ollama" }))).toBe("forbidden-local");
  });
  test("LM Studio (in name) → forbidden-local", () => {
    expect(detectForbidden(model({ name: "LM Studio model" }))).toBe("forbidden-local");
  });
  test("Gemini → forbidden-gemini", () => {
    expect(detectForbidden(model({ id: "gemini-2.5", providerID: "google" }))).toBe("forbidden-gemini");
  });
  test("local in api.url → forbidden-local", () => {
    expect(detectForbidden(model({ api: { url: "http://localhost:11434" } }))).toBe("forbidden-local");
  });
  test("a free opencode model is NOT forbidden", () => {
    expect(detectForbidden(model({ id: "deepseek-v4-flash-free", providerID: "opencode" }))).toBeNull();
  });
});

describe("priority ordering", () => {
  test("larger context first, then newer release_date", () => {
    const big = model({ id: "big", limit: { context: 256000 }, release_date: "2026-07-01" });
    const small = model({ id: "small", limit: { context: 100000 }, release_date: "2026-08-01" });
    const res = filterModels([small, big], now);
    expect(res.eligible[0].identity).toBe("opencode/big");
  });
});

describe("opencodeSetExhausted", () => {
  test("all-rejected set is exhausted", () => {
    const res = filterModels([
      model({ status: "inactive" }),
      model({ cost: { input: 1, output: 0 } }),
    ], now);
    expect(res.eligible.length).toBe(0);
    expect(res.opencodeSetExhausted).toBe(true);
  });
  test("mixed set is not exhausted", () => {
    const res = filterModels([
      model({ id: "ok" }),
      model({ status: "inactive" }),
    ], now);
    expect(res.eligible.length).toBe(1);
    expect(res.opencodeSetExhausted).toBe(false);
  });
});

describe("isFreeCost unit", () => {
  test("0/0 → free", () => { expect(isFreeCost({ input: 0, output: 0 }).free).toBe(true); });
  test("undefined → unknown", () => { expect(isFreeCost(undefined).reason).toBe("unknown-cost"); });
});

describe("parseOpenCodeModels", () => {
  test("parses id-line + JSON block pairs", () => {
    const stdout = [
      "opencode/big-pickle",
      JSON.stringify(model({ id: "big-pickle", providerID: "opencode", name: "Big Pickle" }), null, 0),
      "opencode/mini",
      JSON.stringify(model({ id: "mini", providerID: "opencode" }), null, 0),
    ].join("\n");
    const parsed = parseOpenCodeModels(stdout);
    expect(parsed.length).toBe(2);
    expect(parsed[0].id).toBe("big-pickle");
  });

  test("skips non-JSON noise lines", () => {
    const stdout = "some log noise\nopencode/x\n" + JSON.stringify(model({ id: "x" })) + "\nmore noise";
    const parsed = parseOpenCodeModels(stdout);
    expect(parsed.length).toBe(1);
    expect(parsed[0].id).toBe("x");
  });
});

describe("OpenCode provider boundary", () => {
  test("queries only the opencode provider", async () => {
    let seen: string[] = [];
    await discover({
      runOpenCode: async args => {
        seen = args;
        return { ok: true, stdout: "", stderr: "" };
      },
      now,
    });
    expect(seen).toEqual(["models", "opencode", "--verbose"]);
  });
});
