// extract-ci-failure-context.ts の純関数ロジック単体テスト (#150)
import { describe, expect, test } from "bun:test";
import { extractCiFailureContext } from "../extract-ci-failure-context.ts";

describe("extractCiFailureContext", () => {
  test("T01: error: を含むログ → 該当行と前後を含む抜粋を返す", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    lines[60] = "error: something failed";
    const result = extractCiFailureContext(lines.join("\n"), 10);
    expect(result).toContain("error: something failed");
    expect(result.split("\n").length).toBeLessThanOrEqual(11);
  });

  test("T02: FAILED を含むログ → 該当行と前後を含む抜粋を返す", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    lines[75] = "test result: FAILED";
    const result = extractCiFailureContext(lines.join("\n"), 10);
    expect(result).toContain("test result: FAILED");
  });

  test("T03: panicked at を含むログ → 該当行と前後を含む抜粋を返す", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    lines[80] = "thread 'main' panicked at src/lib.rs:10:5";
    const result = extractCiFailureContext(lines.join("\n"), 10);
    expect(result).toContain("panicked at");
  });

  test("T04: 何もヒットしないログ → 末尾 maxLines 行を返す", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    const result = extractCiFailureContext(lines.join("\n"), 10);
    expect(result).toContain("line 99");
    expect(result).toContain("line 90");
    expect(result).not.toContain("line 0");
  });

  test("T05: 空文字列入力 → 空文字列を返す", () => {
    expect(extractCiFailureContext("")).toBe("");
  });

  test("T_boundary_short_log: 5 行のログ・maxLines=40 → 全 5 行返す", () => {
    const input = "a\nb\nc\nd\ne";
    expect(extractCiFailureContext(input, 40)).toBe("a\nb\nc\nd\ne");
  });

  test("error[E でも検出する (rustc error code)", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    lines[40] = "error[E0277]: trait bound not satisfied";
    const result = extractCiFailureContext(lines.join("\n"), 10);
    expect(result).toContain("error[E0277]");
  });
});
