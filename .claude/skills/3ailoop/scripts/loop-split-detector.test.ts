// #261: loop-split-detector の milestone 継承を単体テスト
import { describe, expect, test } from "bun:test";
import { fetchParentMilestone } from "./loop-split-detector";

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
});
