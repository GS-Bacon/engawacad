// zai-model.test.ts — GLM モデル識別子の単一真実源を検証 (#309)

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  DEFAULT_GLM_MODEL,
  resolveGlmModel,
  buildAnthropicModelEnv,
} from "./zai-model.ts";

let savedGlmModel: string | undefined;

beforeEach(() => {
  savedGlmModel = process.env.GLM_MODEL;
  delete process.env.GLM_MODEL;
});

afterEach(() => {
  if (savedGlmModel === undefined) delete process.env.GLM_MODEL;
  else process.env.GLM_MODEL = savedGlmModel;
});

describe("resolveGlmModel (#309)", () => {
  test("T01_default: GLM_MODEL 未設定 → GLM-5.1", () => {
    delete process.env.GLM_MODEL;
    expect(resolveGlmModel()).toBe("GLM-5.1");
    expect(DEFAULT_GLM_MODEL).toBe("GLM-5.1");
  });

  test("T02_override: GLM_MODEL 設定 → その値", () => {
    process.env.GLM_MODEL = "glm-4.6";
    expect(resolveGlmModel()).toBe("glm-4.6");
  });
});

describe("buildAnthropicModelEnv (#309)", () => {
  // 3 スクリプト (dispatch-glm / glm-via-zai / dispatch-glm-review) が
  // Z.AI に渡す ANTHROPIC_DEFAULT_*_MODEL が全経路で同一値になることを担保する。
  test("T03_default_env: 未設定 → 3 キーとも GLM-5.1", () => {
    delete process.env.GLM_MODEL;
    const env = buildAnthropicModelEnv();
    expect(env.ANTHROPIC_DEFAULT_OPUS_MODEL).toBe("GLM-5.1");
    expect(env.ANTHROPIC_DEFAULT_SONNET_MODEL).toBe("GLM-5.1");
    expect(env.ANTHROPIC_DEFAULT_HAIKU_MODEL).toBe("GLM-5.1");
  });

  test("T04_explicit_model: 明示 model → 3 キーともその値 (dispatch-glm --model 経路)", () => {
    const env = buildAnthropicModelEnv("GLM-5.1-pro");
    expect(env.ANTHROPIC_DEFAULT_OPUS_MODEL).toBe("GLM-5.1-pro");
    expect(env.ANTHROPIC_DEFAULT_SONNET_MODEL).toBe("GLM-5.1-pro");
    expect(env.ANTHROPIC_DEFAULT_HAIKU_MODEL).toBe("GLM-5.1-pro");
  });

  test("T05_env_override: GLM_MODEL 設定 → default 引数がその値で解決", () => {
    process.env.GLM_MODEL = "glm-4.6";
    const env = buildAnthropicModelEnv();
    expect(env.ANTHROPIC_DEFAULT_OPUS_MODEL).toBe("glm-4.6");
    expect(env.ANTHROPIC_DEFAULT_HAIKU_MODEL).toBe("glm-4.6");
  });
});
