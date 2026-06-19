// #261: loop-split-detector の milestone 継承を単体テスト
import { describe, expect, test } from "bun:test";
import { buildCreateChildArgs, fetchParentMilestone } from "./loop-split-detector";

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
