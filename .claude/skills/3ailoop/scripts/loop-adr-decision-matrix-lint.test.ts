import { describe, expect, test } from "bun:test";
import { readFileSync } from "fs";
import {
  lintAdr,
  extractRelatedAdrs,
  extractBodyAdrReferences,
  detectSelfAdrNumber,
  hasSensitiveTopic,
} from "./loop-adr-decision-matrix-lint";

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

  test("Independent 明記で adr_relations pass (sensitive topic 無し)", () => {
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

describe("extractRelatedAdrs", () => {
  test("Related 行から複数 ADR 抽出", () => {
    const md = "# ADR-100\n\n**Related**: ADR-001 (B-rep), ADR-010 (Sketch), ADR-013\n\n## Decision";
    expect(extractRelatedAdrs(md)).toEqual([1, 10, 13]);
  });
  test("Related 行無し → 空", () => {
    const md = "# ADR-100\n\n## Decision\nADR-010 を引用するが Related 行なし";
    expect(extractRelatedAdrs(md)).toEqual([]);
  });
  test("preamble の後の section 内 Related は無視", () => {
    const md = "# ADR-100\n\n## Section\n**Related**: ADR-010\n";
    expect(extractRelatedAdrs(md)).toEqual([]);
  });
});

describe("extractBodyAdrReferences", () => {
  test("body 全体から引用抽出、自己参照除外", () => {
    const md = "# ADR-100\n\nADR-001 と ADR-010 を引用。ADR-100 自身も書ける。";
    expect(extractBodyAdrReferences(md, 100)).toEqual([1, 10]);
  });
  test("self が null なら除外なし", () => {
    const md = "ADR-001, ADR-010, ADR-100";
    expect(extractBodyAdrReferences(md, null)).toEqual([1, 10, 100]);
  });
});

describe("detectSelfAdrNumber", () => {
  test("# ADR-NNN: 形式から番号取得", () => {
    expect(detectSelfAdrNumber("# ADR-017: Phase 10 ...")).toBe(17);
  });
  test("非標準タイトル → null", () => {
    expect(detectSelfAdrNumber("# Foo")).toBe(null);
  });
});

describe("hasSensitiveTopic", () => {
  test("schema_version 含む → true", () => {
    expect(hasSensitiveTopic("schema_version を bump する")).toBe(true);
  });
  test("format 含む → true", () => {
    expect(hasSensitiveTopic("`.engawa` format を扱う")).toBe(true);
  });
  test("互換 含む → true", () => {
    expect(hasSensitiveTopic("既存 YAML 互換を維持")).toBe(true);
  });
  test("無関係なトピック → false", () => {
    expect(hasSensitiveTopic("ロギング戦略を決める ADR")).toBe(false);
  });
});

describe("lintAdr: sensitive topic + Related 強化", () => {
  test("sensitive topic を含むのに Independent 単独 → fail", () => {
    const md = [
      "# ADR-200",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "Trade-off: A 速い B 遅い",
      "採用: A。schema_version を bump する。",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
      "Independent.",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "adr_relations_required_for_sensitive_topic")).toBe(true);
  });

  test("sensitive topic + Related 行に ADR-NNN 1 件 → pass", () => {
    const md = [
      "# ADR-200",
      "",
      "**Related**: ADR-015 (schema_version 規約)",
      "",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "Trade-off: A 速い B 遅い",
      "採用: A。schema_version を bump する。",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
    ].join("\n");
    const r = lintAdr(md);
    if (!r.ok) console.error(r.issues);
    expect(r.ok).toBe(true);
  });

  test("本文中の ADR-NNN 引用が Related 行に無い → orphan fail", () => {
    const md = [
      "# ADR-200",
      "",
      "**Related**: ADR-001",
      "",
      "## Decision",
      "- A. opt1",
      "- B. opt2",
      "- C. opt3",
      "Trade-off: A 速い B 遅い",
      "採用: A。なお ADR-010 を踏襲する。",
      "## Consequences",
      "採用前提崩壊 trigger: foo が観測されたら見直し",
    ].join("\n");
    const r = lintAdr(md);
    expect(r.ok).toBe(false);
    expect(r.issues.some(i => i.rule === "adr_relations_body_orphan")).toBe(true);
  });

  test("本物 ADR-015 (amend 後) は pass", () => {
    const md = readFileSync("docs/decisions/015-phase9-design-foundations.md", "utf-8");
    const r = lintAdr(md);
    if (!r.ok) console.error(r.issues);
    expect(r.ok).toBe(true);
  });

  test("本物 ADR-017 は pass", () => {
    const md = readFileSync("docs/decisions/017-phase10-sketch-curves-and-edits.md", "utf-8");
    const r = lintAdr(md);
    if (!r.ok) console.error(r.issues);
    expect(r.ok).toBe(true);
  });
});
