import { describe, expect, test } from "bun:test";
import { readFileSync } from "fs";
import { lintAdr } from "./loop-adr-decision-matrix-lint";

describe("loop-adr-decision-matrix-lint", () => {
  test("ADR-013 (本物) は pass", () => {
    const md = readFileSync("docs/decisions/013-adr-auto-accept-flow.md", "utf-8");
    const r = lintAdr(md);
    if (!r.ok) {
      console.error(r.issues);
    }
    expect(r.ok).toBe(true);
  });

  test("Decision セクション欠落で fail", () => {
    const md = `# ADR-X\n\n## Context\nfoo\n\n## Consequences\nbar\n`;
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "decision_section")).toBe(true);
  });

  test("Options 2 案で fail", () => {
    const md = [
      "# ADR-X",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "Trade-off: A は速い、B は遅い",
      "採用: A",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
      "Related: ADR-001",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "options_count")).toBe(true);
  });

  test("Trade-off 欠落で fail", () => {
    const md = [
      "# ADR-X",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "採用: A",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
      "Related: ADR-001",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "trade_off")).toBe(true);
  });

  test("Trigger 欠落で fail", () => {
    const md = [
      "# ADR-X",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "Trade-off: A 速い B 遅い",
      "採用: A",
      "## Consequences",
      "Related: ADR-001",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "revisit_trigger")).toBe(true);
  });

  test("ADR 関係欠落で fail", () => {
    const md = [
      "# ADR-X",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "Trade-off: A 速い B 遅い",
      "採用: A",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "adr_relations")).toBe(true);
  });

  test("Independent 明記で adr_relations pass", () => {
    const md = [
      "# ADR-X",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "Trade-off: A 速い B 遅い",
      "採用: A",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
      "Independent.",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(true);
  });
});
