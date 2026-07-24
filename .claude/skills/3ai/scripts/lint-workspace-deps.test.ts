// lint-workspace-deps.test.ts — [dependencies] workspace 集約チェックの単体テスト

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  scanCrateCargoToml,
  scanWorkspace,
  stripInlineComment,
  type Violation,
} from "./lint-workspace-deps.ts";

let workDir: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "lint-workspace-deps-"));
});

afterEach(() => {
  rmSync(workDir, { recursive: true, force: true });
});

function writeCargoToml(cratePath: string, contents: string): string {
  mkdirSync(cratePath, { recursive: true });
  const p = join(cratePath, "Cargo.toml");
  writeFileSync(p, contents, "utf-8");
  return p;
}

describe("lint-workspace-deps", () => {
  test("T01_workspace_deps_ok: workspace=true と dotted-key はどちらも OK", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = { workspace = true }
baz.workspace = true
qux = { workspace = true, features = ["a", "b"] }
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations).toEqual([]);
  });

  test("T02_workspace_deps_ng: 裸バージョン + version-only inline table は違反", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = "1.0.0"
baz = { version = "2.0" }
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations.length).toBe(2);
    const names = violations.map((v) => v.name).sort();
    expect(names).toEqual(["bar", "baz"]);
    for (const v of violations) {
      expect(v.file).toBe(file);
      expect(v.line).toBeGreaterThan(0);
      expect(v.detail.length).toBeGreaterThan(0);
    }
  });

  test("T_DEG_workspace_deps_empty: 空 [dependencies] は OK", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("T_DEG_workspace_deps_missing_section: [dependencies] 自体が無くても OK", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("bonus: [dev-dependencies] のバージョン literal はスコープ外 (OK)", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = { workspace = true }

[dev-dependencies]
approx = "0.5"
serde_json = { version = "1.0" }
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("bonus: [build-dependencies] もスコープ外", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = { workspace = true }

[build-dependencies]
cc = "1.0"
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("bonus: target.<cfg>.dependencies もスコープ外", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = { workspace = true }

[target.'cfg(unix)'.dependencies]
libc = "0.2"
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("bonus: 複数行 inline table + workspace=true は OK", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = {
  workspace = true,
  features = ["a", "b"],
}
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("bonus: 複数行 inline table で version のみは違反", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[package]
name = "foo"

[dependencies]
bar = {
  version = "1.0",
  features = ["a"],
}
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations.length).toBe(1);
    expect(violations[0].name).toBe("bar");
  });

  test("bonus: git dep (workspace なし) は違反", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[dependencies]
bar = { git = "https://example.com/repo.git" }
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations.length).toBe(1);
    expect(violations[0].name).toBe("bar");
  });

  test("bonus: path dep (workspace なし) は違反", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[dependencies]
bar = { path = "../bar" }
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations.length).toBe(1);
    expect(violations[0].name).toBe("bar");
  });

  test("bonus: [dependencies.<name>] subtable + workspace = true は OK", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[dependencies.bar]
workspace = true
features = ["a"]
`,
    );
    expect(scanCrateCargoToml(file)).toEqual([]);
  });

  test("bonus: [dependencies.<name>] subtable で workspace 無しは違反", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[dependencies.bar]
version = "1.0"
features = ["a"]
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations.length).toBe(1);
    expect(violations[0].name).toBe("bar");
  });

  test("bonus: 行末コメント # workspace = true で騙されない", () => {
    const file = writeCargoToml(
      join(workDir, "crates", "foo"),
      `[dependencies]
bar = "1.0"  # workspace = true (comment, not actual)
`,
    );
    const violations = scanCrateCargoToml(file);
    expect(violations.length).toBe(1);
    expect(violations[0].name).toBe("bar");
  });

  test("stripInlineComment: quote 内の # は保持、外の # は切る", () => {
    expect(stripInlineComment(`foo = "bar"  # comment`)).toBe(`foo = "bar"  `);
    expect(stripInlineComment(`foo = "a#b"`)).toBe(`foo = "a#b"`);
    expect(stripInlineComment(`foo = "a#b" # trailing`)).toBe(
      `foo = "a#b" `,
    );
    expect(stripInlineComment(`# whole line comment`)).toBe(``);
  });

  test("scanWorkspace: crates/*/Cargo.toml を集約して検査", async () => {
    writeCargoToml(
      join(workDir, "crates", "aaa"),
      `[dependencies]
foo = { workspace = true }
`,
    );
    writeCargoToml(
      join(workDir, "crates", "bbb"),
      `[dependencies]
bar = "1.0"
`,
    );
    // crates 直下でない Cargo.toml (workspace root など) はスコープ外
    writeFileSync(
      join(workDir, "Cargo.toml"),
      `[workspace]
members = ["crates/*"]
[dependencies]
should_not_be_scanned = "9.9"
`,
      "utf-8",
    );
    const violations = await scanWorkspace(workDir);
    expect(violations.length).toBe(1);
    expect(violations[0].name).toBe("bar");
    expect(violations[0].file).toContain("bbb/Cargo.toml");
  });
});
