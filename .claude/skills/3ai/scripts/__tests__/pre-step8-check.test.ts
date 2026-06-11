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

  // 以下は Codex F01 (#139 STEP 7.5 review) の指摘に対する回帰テスト
  test("Codex F01a: helper 関数があっても全 #[test] が stub なら flagged (helper は無視)", () => {
    const text = `
fn build_fixture() -> u32 { 42 }

fn helper(x: u32) -> u32 { x * 2 }

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_foo() {
    todo!()
}

#[test]
#[ignore]
fn t02_bar() {
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F01a': helper 関数 + 一部 test が本実装 → not flagged", () => {
    const text = `
fn build_fixture() -> u32 { 42 }

#[test]
#[ignore]
fn t01() { todo!() }

#[test]
fn t02_real() {
    assert_eq!(build_fixture(), 42);
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(false);
  });

  test("Codex F01b: #[test] と #[ignore] の間に #[cfg(...)] が挿入されていても flagged", () => {
    const text = `
#[test]
#[cfg(feature = "ci")]
#[ignore = "..."]
fn t01_foo() {
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F01c: todo!(\"message\") メッセージ付きでも flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01_foo() {
    todo!("WIP: implement after #X")
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F01c': unimplemented!(\"see #X\") メッセージ付きでも flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01_foo() {
    unimplemented!("see issue #X")
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F01d: #[ignore] が他 attr の間にあっても認識", () => {
    const text = `
#[ignore]
#[cfg(test)]
#[test]
fn t01_foo() {
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  // F02 (#139 STEP 7.5 round 2): #[tokio::test] / async fn 対応
  test("Codex F02a: #[tokio::test] async fn 全部 stub → flagged", () => {
    const text = `
#[tokio::test]
#[ignore]
async fn a01_foo() {
    todo!()
}

#[tokio::test]
#[ignore]
async fn a02_bar() {
    unimplemented!("WIP")
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F02b: #[tokio::test] async fn に本実装 → not flagged", () => {
    const text = `
#[tokio::test]
async fn a01_real() {
    let response = client.get("/api").send().await;
    assert_eq!(response.status(), 200);
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(false);
  });

  test("Codex F02c: mixed #[test] + #[tokio::test] 全部 stub → flagged", () => {
    const text = `
#[test]
#[ignore]
fn sync_t01() {
    todo!()
}

#[tokio::test]
#[ignore]
async fn async_a01() {
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F02d: smol::test など他の async runtime test 属性も対応", () => {
    const text = `
#[smol::test]
#[ignore]
async fn s01() {
    todo!()
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  // F02 round 3 (#139): マクロ delimiter 形式 todo!{} / todo![]
  test("Codex F02e: todo!{} curly delimiter → flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01() {
    todo!{}
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F02f: todo![] square delimiter → flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01() {
    todo![]
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });

  test("Codex F02g: unimplemented!{\"WIP\"} メッセージ + curly → flagged", () => {
    const text = `
#[test]
#[ignore]
fn t01() {
    unimplemented!{"WIP"}
}
`;
    expect(isAllTodoIgnoreStub(text)).toBe(true);
  });
});
