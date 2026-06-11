// pre-step8-check.ts の腐敗 stub 検出ロジックの単体テスト (#139)
import { describe, expect, test } from "bun:test";
import { isAllTodoIgnoreStub, isOnelineStub } from "../pre-step8-check.ts";

describe("isOnelineStub", () => {
  test("temporary probe with safely deleted phrase → flagged", () => {
    const text = `
// This file was a temporary probe and can be safely deleted.
// Keeping it empty to avoid breaking the build.
`;
    expect(isOnelineStub(text)).toBe(true);
  });

  test("temporary diagnostic file removed → flagged", () => {
    expect(isOnelineStub("// temporary diagnostic file removed\n")).toBe(true);
  });

  test("legitimate Removed: tombstone (no temporary/probe/diagnostic) → not flagged", () => {
    const text = `
// Removed: acceptance tests for Issue #59 are covered by existing
// golden_examples.rs and examples_smoke.rs tests.
`;
    expect(isOnelineStub(text)).toBe(false);
  });

  test("4+ non-empty lines → not flagged regardless of keywords", () => {
    const text = `
// temporary probe
// line 2
// line 3
// line 4
`;
    expect(isOnelineStub(text)).toBe(false);
  });

  test("empty file → not flagged", () => {
    expect(isOnelineStub("")).toBe(false);
    expect(isOnelineStub("\n\n\n")).toBe(false);
  });

  test("temporary alone (without probe/diagnostic/debug) → not flagged", () => {
    expect(isOnelineStub("// temporary placeholder for X\n")).toBe(false);
  });
});

describe("isAllTodoIgnoreStub", () => {
  test("all #[test] are #[ignore] + todo!() → flagged", () => {
    const text = `
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_foo() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t02_bar() {
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("partial implementation (one fn has real assertion) → not flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01_foo() {
    todo!()
}

#[test]
fn t02_bar() {
    assert_eq!(1 + 1, 2);
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(false);
  });

  test("all real implementations → not flagged", () => {
    const text = `
#[test]
fn t01_foo() {
    let x = compute();
    assert_eq!(x, 42);
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(false);
  });

  test("comments and whitespace inside todo!() body → still flagged", () => {
    const text = `
#[test]
#[ignore = "..."]
fn t01_foo() {
    // 説明コメント
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("unimplemented!() body → flagged (同じ未実装マーカー)", () => {
    const text = `
#[test]
#[ignore]
fn t01_foo() {
    unimplemented!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("no #[test] functions → not flagged", () => {
    expect(isAllTodoIgnoreStub("// just comments")).toBe(false);
  });

  test("#[ignore] missing on one #[test] → not flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01() { todo!() }

#[test]
fn t02() { todo!() }
`;
    expect(isAllTodoIgnoreStub(text)).toBe(false);
  });
});
