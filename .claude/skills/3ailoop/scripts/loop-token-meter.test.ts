// loop-token-meter.test.ts — #228 GLM/Claude wiring の JSON parse 経路を検証

import { describe, expect, test } from "bun:test";
import { classifyAndExtractFile } from "./loop-token-meter";

const SAMPLE_GLM_RESULT = JSON.stringify({
  type: "result",
  subtype: "success",
  is_error: false,
  total_cost_usd: 3.62,
  usage: {
    input_tokens: 144568,
    cache_creation_input_tokens: 0,
    cache_read_input_tokens: 4848640,
    output_tokens: 18471,
  },
  modelUsage: {
    "GLM-5.1": {
      inputTokens: 3056,
      outputTokens: 18,
      cacheReadInputTokens: 256,
      cacheCreationInputTokens: 0,
    },
    "claude-opus-4-7": {
      inputTokens: 144568,
      outputTokens: 18471,
      cacheReadInputTokens: 4848640,
      cacheCreationInputTokens: 0,
    },
  },
});

const SAMPLE_GLM_RESULT_USAGE_ONLY = JSON.stringify({
  type: "result",
  usage: {
    input_tokens: 1000,
    output_tokens: 200,
    cache_read_input_tokens: 500,
    cache_creation_input_tokens: 100,
  },
});

const SAMPLE_CODEX_LOG = `issues:
  - id: F01
    severity: high
some other text
total_tokens: 12345
input_tokens: 8000
output_tokens: 4345
`;

describe("classifyAndExtractFile (#228)", () => {
  test("splits GLM and Claude from modelUsage in .json.raw", () => {
    const r = classifyAndExtractFile(
      "features/X/glm-result.json.raw",
      SAMPLE_GLM_RESULT,
    );
    // GLM-5.1 totals: 3056+18+256+0 = 3330
    expect(r.glm).toBe(3056 + 18 + 256 + 0);
    // claude-opus-4-7 totals: 144568+18471+4848640+0 = 5011679
    expect(r.claude).toBe(144568 + 18471 + 4848640 + 0);
    expect(r.codex).toBe(0);
  });

  test("falls back to top-level usage when modelUsage absent", () => {
    const r = classifyAndExtractFile(
      "features/X/glm-result.json.raw",
      SAMPLE_GLM_RESULT_USAGE_ONLY,
    );
    // 1000+200+500+100 = 1800 → glm (classified by filename)
    expect(r.glm).toBe(1800);
    expect(r.claude).toBe(0);
    expect(r.codex).toBe(0);
  });

  test("uses regex extraction for non-JSON files (codex log)", () => {
    const r = classifyAndExtractFile(
      "features/X/codex-final.yaml.log",
      SAMPLE_CODEX_LOG,
    );
    // extractTokens takes max per field then sums:
    // total_tokens=12345, input_tokens=8000, output_tokens=4345
    // → 12345 + 8000 + 4345 = 24690
    expect(r.codex).toBe(12345 + 8000 + 4345);
    expect(r.glm).toBe(0);
    expect(r.claude).toBe(0);
  });

  test("returns zeros for unclassifiable filename", () => {
    const r = classifyAndExtractFile(
      "features/X/random.json",
      SAMPLE_GLM_RESULT,
    );
    expect(r.claude).toBe(0);
    expect(r.glm).toBe(0);
    expect(r.codex).toBe(0);
  });

  test("handles NDJSON stream (last result line wins)", () => {
    const ndjson = [
      JSON.stringify({ type: "progress", step: 1 }),
      JSON.stringify({ type: "progress", step: 2 }),
      SAMPLE_GLM_RESULT,
    ].join("\n");
    const r = classifyAndExtractFile(
      "features/X/glm-result.json.raw",
      ndjson,
    );
    expect(r.glm).toBe(3056 + 18 + 256);
    expect(r.claude).toBe(144568 + 18471 + 4848640);
  });

  test("returns zeros for malformed JSON in .json.raw (no fallback regex match)", () => {
    const r = classifyAndExtractFile(
      "features/X/glm-result.json.raw",
      "{not valid json",
    );
    expect(r.claude + r.glm + r.codex).toBe(0);
  });

  test("classifies claude-named file with raw response correctly", () => {
    const r = classifyAndExtractFile(
      "features/X/claude-session.json",
      SAMPLE_GLM_RESULT_USAGE_ONLY,
    );
    // no modelUsage → fallback to top-level usage, classified as claude
    expect(r.claude).toBe(1000 + 200 + 500 + 100);
    expect(r.glm).toBe(0);
  });
});
