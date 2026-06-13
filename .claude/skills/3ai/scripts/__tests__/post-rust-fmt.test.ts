// post-rust-fmt.ts の extractRustFilePath ロジックの単体テスト (#149)
import { describe, expect, test } from "bun:test";
import { extractRustFilePath } from "../post-rust-fmt.ts";

describe("extractRustFilePath", () => {
  test("T01: Edit tool with .rs file → returns path", () => {
    const input = JSON.stringify({
      tool_name: "Edit",
      tool_input: { file_path: "/home/bacon/engawa/crates/foo/src/bar.rs" },
    });
    expect(extractRustFilePath(input)).toBe("/home/bacon/engawa/crates/foo/src/bar.rs");
  });

  test("T02: Write tool with .toml file → returns null (non-rust)", () => {
    const input = JSON.stringify({
      tool_name: "Write",
      tool_input: { file_path: "/home/bacon/engawa/Cargo.toml" },
    });
    expect(extractRustFilePath(input)).toBeNull();
  });

  test("T03: missing tool_input → returns null", () => {
    const input = JSON.stringify({ tool_name: "Edit" });
    expect(extractRustFilePath(input)).toBeNull();
  });

  test("T04: missing file_path → returns null", () => {
    const input = JSON.stringify({ tool_name: "Edit", tool_input: {} });
    expect(extractRustFilePath(input)).toBeNull();
  });

  test("T05: malformed JSON → returns null", () => {
    expect(extractRustFilePath("not json")).toBeNull();
  });

  test("T_boundary_rs_bak: file_path ending with .rs.bak → returns null", () => {
    const input = JSON.stringify({
      tool_name: "Write",
      tool_input: { file_path: "/tmp/foo.rs.bak" },
    });
    expect(extractRustFilePath(input)).toBeNull();
  });
});
