// #261: loop-split-detector の milestone 継承を単体テスト
// intent-check yaml (aligned=no + split_proposal 同居) パーサ回帰テストも本 file 内で管理
import { describe, expect, test } from "bun:test";
import {
  buildCreateChildArgs,
  fetchParentMilestone,
  parseSplitProposalYaml,
} from "./loop-split-detector";

describe("fetchParentMilestone (#261)", () => {
  test("親に milestone が紐付いている → title を返す", async () => {
    const ghFn = async () => ({
      stdout: JSON.stringify({ milestone: { title: "Phase 9: 履歴編集" } }),
      exit: 0,
    });
    expect(await fetchParentMilestone(244, ghFn)).toBe("Phase 9: 履歴編集");
  });

  test("親に milestone なし (null) → null", async () => {
    const ghFn = async () => ({ stdout: JSON.stringify({ milestone: null }), exit: 0 });
    expect(await fetchParentMilestone(244, ghFn)).toBeNull();
  });

  test("親 Issue の取得失敗 (gh exit != 0) → null", async () => {
    const ghFn = async () => ({ stdout: "", exit: 1 });
    expect(await fetchParentMilestone(999, ghFn)).toBeNull();
  });

  test("gh 出力が JSON でない → null", async () => {
    const ghFn = async () => ({ stdout: "not json at all", exit: 0 });
    expect(await fetchParentMilestone(244, ghFn)).toBeNull();
  });

  test("milestone フィールドが欠落 → null", async () => {
    const ghFn = async () => ({ stdout: JSON.stringify({}), exit: 0 });
    expect(await fetchParentMilestone(244, ghFn)).toBeNull();
  });

  test("milestone.title が undefined → null", async () => {
    const ghFn = async () => ({ stdout: JSON.stringify({ milestone: {} }), exit: 0 });
    expect(await fetchParentMilestone(244, ghFn)).toBeNull();
  });

  // #261 F02 r2 (Codex r2 medium): ghFn 自身が throw した場合も null
  test("ghFn が throw → null (silent skip 契約)", async () => {
    const ghFn = async () => { throw new Error("spawn failed"); };
    expect(await fetchParentMilestone(244, ghFn)).toBeNull();
  });
});

// #261 F02 (Codex medium): createChild の引数組み立てを単体テスト。
// 実 gh は呼ばず、buildCreateChildArgs だけで --milestone 付与と決定性を検証。
describe("buildCreateChildArgs (#261 F02)", () => {
  const entry = { title: "child A", body: "body A", labels: ["type: feature", "batch:kernel"] };

  test("parentMilestone あり → --milestone <title> が含まれる", () => {
    const args = buildCreateChildArgs(entry, 244, "Phase 9: 履歴編集", "/tmp/x.md");
    expect(args).toContain("--milestone");
    expect(args[args.indexOf("--milestone") + 1]).toBe("Phase 9: 履歴編集");
    expect(args).toContain("--title");
    expect(args[args.indexOf("--title") + 1]).toBe("child A");
    expect(args).toContain("--label");
    expect(args[args.indexOf("--label") + 1]).toBe("type: feature,batch:kernel,parent-blocked-by-split:244");
  });

  test("parentMilestone なし (null) → --milestone は含まれない", () => {
    const args = buildCreateChildArgs(entry, 244, null, "/tmp/x.md");
    expect(args).not.toContain("--milestone");
    expect(args).toContain("--title");
  });

  test("T01 決定性: 同一入力 2 回で同一 args", () => {
    const args1 = buildCreateChildArgs(entry, 244, "Phase 9: X", "/tmp/x.md");
    const args2 = buildCreateChildArgs(entry, 244, "Phase 9: X", "/tmp/x.md");
    expect(args1).toEqual(args2);
  });

  test("labels が undefined でも parent-blocked-by-split は付与", () => {
    const args = buildCreateChildArgs({ title: "c" }, 99, null, "/tmp/x.md");
    expect(args[args.indexOf("--label") + 1]).toBe("parent-blocked-by-split:99");
  });
});

// intent-check auto-split 経路の再現テスト。
// #275-#278 で観測された「intent-check aligned=no + 粒度違反 → split-detector 未到達」
// の構造欠陥を修正する PR の一部。intent-check yaml (aligned + reason + split_proposal
// 同居 format) を既存 codex-final.yaml パーサで消化できることを固定する。
describe("parseSplitProposalYaml — intent-check yaml 互換性", () => {
  test("STEP 7.5 codex-final.yaml 相当 (split_proposal のみ) → 全 entry 抽出", () => {
    const yaml = `
split_proposal:
  - title: "Child A"
    body: "body-a"
    labels: ["type: feature", "batch:kernel"]
  - title: "Child B"
    body: "body-b"
    labels: ["type: feature", "batch:kernel"]
`;
    const entries = parseSplitProposalYaml(yaml);
    expect(entries).toHaveLength(2);
    expect(entries.map(e => e.title)).toEqual(["Child A", "Child B"]);
    expect(entries[0].labels).toEqual(["type: feature", "batch:kernel"]);
  });

  test("intent-check yaml format: aligned=no + reason + split_proposal 同居 → 子 entry 抽出成功", () => {
    // #277 実観測相当 (Codex が intent-check で aligned=no + reason + split_proposal 併記した想定)
    const yaml = `aligned: no
reason: |
  Offset / Fillet / Chamfer の 3 機能が 1 Issue に混在しており粒度過大 (ADR-006 §1 違反)。
  機能ごとに分割すること。
split_proposal:
  - title: "Sketch Offset 実装"
    body: |
      分割元: #277
      In-Scope: Sketch Offset のみ
    labels: ["type: feature", "batch:kernel"]
  - title: "Sketch Fillet 実装"
    body: |
      分割元: #277
      In-Scope: Sketch Fillet のみ
    labels: ["type: feature", "batch:kernel"]
  - title: "Sketch Chamfer 実装"
    body: |
      分割元: #277
      In-Scope: Sketch Chamfer のみ
    labels: ["type: feature", "batch:kernel"]
`;
    const entries = parseSplitProposalYaml(yaml);
    expect(entries).toHaveLength(3);
    expect(entries.map(e => e.title)).toEqual([
      "Sketch Offset 実装",
      "Sketch Fillet 実装",
      "Sketch Chamfer 実装",
    ]);
    for (const e of entries) {
      expect(e.labels).toEqual(["type: feature", "batch:kernel"]);
      expect(e.body).toContain("分割元: #277");
    }
  });

  test("aligned=no + reason のみ (粒度違反以外の refute) → 空配列", () => {
    const yaml = `aligned: no
reason: |
  完了条件が「正常に動作する」で計測不能。
`;
    expect(parseSplitProposalYaml(yaml)).toEqual([]);
  });

  test("aligned=yes → 空配列 (通常 pass 経路)", () => {
    expect(parseSplitProposalYaml("aligned: yes\ncomment: ok\n")).toEqual([]);
  });
});
