// lint-fuzz-corpus-updated.test.ts — schema 変更時 fuzz corpus 追加チェックの単体テスト
//
// git subprocess は起動せず、CheckDeps を injection してロジックだけを検証する。
// T07-T09 は Issue #318 のテスト計画 ID と対応。

import { describe, expect, test } from "bun:test";
import {
  detectSubstantiveSchemaChange,
  filterSchemaFiles,
  parseDiffNames,
  runCheck,
  type CheckDeps,
  type CheckOptions,
} from "./lint-fuzz-corpus-updated.ts";

const SCHEMA_GLOB = "crates/engawa-format/src/**/*.rs";
const CORPUS_DIR = "fuzz/corpus/parser";

function makeDeps(overrides: Partial<CheckDeps> = {}): CheckDeps {
  return {
    runDiffNames: () => "",
    runDiffContent: () => "",
    runDiffCorpusAdds: () => "",
    dirExists: () => true,
    ...overrides,
  };
}

const baseOpts: CheckOptions = {
  base: "main",
  head: "HEAD",
  corpusDir: CORPUS_DIR,
  schemaGlob: SCHEMA_GLOB,
};

// ------------------------------------------------------------
// parseDiffNames
// ------------------------------------------------------------

describe("parseDiffNames", () => {
  test("name-only (tab 無し) は各行がそのまま path", () => {
    const raw = "crates/engawa-format/src/feature.rs\nfuzz/corpus/parser/seed_1";
    expect(parseDiffNames(raw)).toEqual([
      "crates/engawa-format/src/feature.rs",
      "fuzz/corpus/parser/seed_1",
    ]);
  });

  test("name-status で filterAdd=true → A 行のみ", () => {
    const raw = [
      "A\tfuzz/corpus/parser/seed_new_op.engawa",
      "M\tcrates/engawa-format/src/feature.rs",
      "D\tsome/deleted.rs",
    ].join("\n");
    expect(parseDiffNames(raw, true)).toEqual([
      "fuzz/corpus/parser/seed_new_op.engawa",
    ]);
  });

  test("name-status で filterAdd=false → 全 status", () => {
    const raw = [
      "A\tfoo.rs",
      "M\tbar.rs",
    ].join("\n");
    expect(parseDiffNames(raw, false)).toEqual(["foo.rs", "bar.rs"]);
  });

  test("rename (R100 old\\tnew) は new path を採用", () => {
    const raw = "R100\told/path.rs\tnew/path.rs";
    expect(parseDiffNames(raw, false)).toEqual(["new/path.rs"]);
  });

  test("空文字列 / whitespace-only 行はスキップ", () => {
    expect(parseDiffNames("")).toEqual([]);
    expect(parseDiffNames("\n\n\n")).toEqual([]);
    expect(parseDiffNames("  \n\t\n")).toEqual([]);
  });

  test("CRLF は許容", () => {
    const raw = "foo.rs\r\nbar.rs\r\n";
    expect(parseDiffNames(raw)).toEqual(["foo.rs", "bar.rs"]);
  });
});

// ------------------------------------------------------------
// detectSubstantiveSchemaChange
// ------------------------------------------------------------

describe("detectSubstantiveSchemaChange", () => {
  test("T_bonus_new_variant: `+    NewOp { param: f64 },` は substantive", () => {
    const diff = [
      "diff --git a/crates/engawa-format/src/feature.rs b/crates/engawa-format/src/feature.rs",
      "@@ -100,0 +100,3 @@",
      "+    NewOp {",
      "+        param: f64,",
      "+    },",
    ].join("\n");
    expect(detectSubstantiveSchemaChange(diff)).toBe(true);
  });

  test("T_bonus_new_variant tuple 形式 `+    NewVariant(String),`", () => {
    const diff = "+    NewVariant(String),";
    expect(detectSubstantiveSchemaChange(diff)).toBe(true);
  });

  test("T_bonus_new_field: `+    pub new_field: u32,` は substantive", () => {
    const diff = "+    pub new_field: u32,";
    expect(detectSubstantiveSchemaChange(diff)).toBe(true);
  });

  test("T_bonus_new_field: `+    pub(crate) inner: String,` も substantive", () => {
    const diff = "+    pub(crate) inner: String,";
    expect(detectSubstantiveSchemaChange(diff)).toBe(true);
  });

  test("T_bonus_doc_only_schema_change: `///` コメントのみは non-substantive", () => {
    const diff = [
      "diff --git a/crates/engawa-format/src/feature.rs b/crates/engawa-format/src/feature.rs",
      "@@ -10,0 +10,2 @@",
      "+    /// Doc comment describing the variant.",
      "+    /// More docs.",
    ].join("\n");
    expect(detectSubstantiveSchemaChange(diff)).toBe(false);
  });

  test("T_bonus_whitespace_only: 空行 / 空白のみは non-substantive", () => {
    const diff = ["+", "+    ", "+"].join("\n");
    expect(detectSubstantiveSchemaChange(diff)).toBe(false);
  });

  test("diff header `+++ b/xxx` は substantive にならない", () => {
    const diff = "+++ b/crates/engawa-format/src/NewFoo.rs";
    expect(detectSubstantiveSchemaChange(diff)).toBe(false);
  });

  test("行頭が `-` の削除行は substantive にならない", () => {
    const diff = "-    OldVariant { x: u32 },\n-    pub old_field: i64,";
    expect(detectSubstantiveSchemaChange(diff)).toBe(false);
  });

  test("インデント 2 空白 (関数内ローカル) は substantive にならない", () => {
    const diff = "+  let x = 1;\n+  Foo {};";
    expect(detectSubstantiveSchemaChange(diff)).toBe(false);
  });

  test("小文字識別子 `+    foo_op(x)` は variant として拾わない", () => {
    const diff = "+    foo_op(x)";
    expect(detectSubstantiveSchemaChange(diff)).toBe(false);
  });

  test("複数変更が混在 (doc-only + 実 field) → substantive", () => {
    const diff = [
      "+    /// Doc",
      "+    pub added: bool,",
    ].join("\n");
    expect(detectSubstantiveSchemaChange(diff)).toBe(true);
  });
});

// ------------------------------------------------------------
// filterSchemaFiles
// ------------------------------------------------------------

describe("filterSchemaFiles", () => {
  test("T_bonus_glob_match: engawa-format 直下のみ拾い、engawa-kernel は落とす", () => {
    const paths = [
      "crates/engawa-format/src/feature.rs",
      "crates/engawa-kernel/src/topo.rs",
    ];
    expect(filterSchemaFiles(paths, "crates/engawa-format/src/**/*.rs")).toEqual([
      "crates/engawa-format/src/feature.rs",
    ]);
  });

  test("深いサブディレクトリも `**` で拾う", () => {
    const paths = ["crates/engawa-format/src/sub/deep/foo.rs"];
    expect(filterSchemaFiles(paths, "crates/engawa-format/src/**/*.rs")).toEqual([
      "crates/engawa-format/src/sub/deep/foo.rs",
    ]);
  });

  test("拡張子違い (.md) は落ちる", () => {
    const paths = ["crates/engawa-format/src/README.md"];
    expect(filterSchemaFiles(paths, "crates/engawa-format/src/**/*.rs")).toEqual([]);
  });

  test("空配列は空", () => {
    expect(filterSchemaFiles([], "crates/engawa-format/src/**/*.rs")).toEqual([]);
  });
});

// ------------------------------------------------------------
// runCheck — 統合ロジック
// ------------------------------------------------------------

describe("runCheck", () => {
  test("T07_fuzz_corpus_schema_no_change: schema 未変更 → exit 0", () => {
    const deps = makeDeps({
      // engawa-format 外 (kernel) のみ変更
      runDiffNames: () => "M\tcrates/engawa-kernel/src/topo.rs\n",
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(0);
    expect(r.status).toBe("ok-no-schema-change");
    expect(r.message).toContain("schema 未変更");
  });

  test("T08_fuzz_corpus_schema_changed_seed_added: 新 variant + corpus A entry → exit 0", () => {
    const deps = makeDeps({
      runDiffNames: () => "M\tcrates/engawa-format/src/feature.rs\n",
      runDiffContent: () =>
        [
          "diff --git a/crates/engawa-format/src/feature.rs b/crates/engawa-format/src/feature.rs",
          "@@ -100,0 +200,3 @@",
          "+    NewOp {",
          "+        param: f64,",
          "+    },",
        ].join("\n"),
      dirExists: () => true,
      runDiffCorpusAdds: () => "A\tfuzz/corpus/parser/seed_new_op.engawa\n",
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(0);
    expect(r.status).toBe("ok-seed-added");
    expect(r.message).toContain("1 件");
  });

  test("T09_fuzz_corpus_schema_changed_seed_missing: schema 変更 + fuzz dir 有り + seed 未追加 → exit 1", () => {
    const deps = makeDeps({
      runDiffNames: () => "M\tcrates/engawa-format/src/feature.rs\n",
      runDiffContent: () => "+    pub new_field: u32,",
      dirExists: () => true,
      runDiffCorpusAdds: () => "", // 追加無し
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(1);
    expect(r.status).toBe("error-seed-missing");
    expect(r.message).toContain("ERROR");
    expect(r.message).toContain("crates/engawa-format/src/feature.rs");
    expect(r.message).toContain(CORPUS_DIR);
  });

  test("T_bonus_fuzz_dir_missing: schema 変更 + fuzz dir 無 → exit 0 + WARN", () => {
    const deps = makeDeps({
      runDiffNames: () => "M\tcrates/engawa-format/src/feature.rs\n",
      runDiffContent: () => "+    NewOp {",
      dirExists: () => false,
      runDiffCorpusAdds: () => "", // 呼ばれない想定
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(0);
    expect(r.status).toBe("warn-fuzz-missing");
    expect(r.message).toContain("WARN");
    expect(r.message).toContain(CORPUS_DIR);
  });

  test("T_bonus_doc_only_schema_change: schema file 変更あるが doc-only → exit 0 non-substantive", () => {
    const deps = makeDeps({
      runDiffNames: () => "M\tcrates/engawa-format/src/feature.rs\n",
      runDiffContent: () =>
        [
          "+    /// New doc comment",
          "+    /// Another doc line",
        ].join("\n"),
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(0);
    expect(r.status).toBe("ok-non-substantive");
    expect(r.message).toContain("doc-only or refactor");
  });

  test("schema 変更が glob 外のみ (document.rs 以外の path) → OK", () => {
    const deps = makeDeps({
      runDiffNames: () =>
        [
          "M\tcrates/engawa-kernel/src/lib.rs",
          "M\tdocs/decisions/006-issue-decomposition.md",
        ].join("\n"),
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(0);
    expect(r.status).toBe("ok-no-schema-change");
  });

  test("schema 実質変更あり + corpus 2 件追加 → exit 0 (数を message に含む)", () => {
    const deps = makeDeps({
      runDiffNames: () => "M\tcrates/engawa-format/src/document.rs\n",
      runDiffContent: () => "+    pub extra: Vec<String>,",
      dirExists: () => true,
      runDiffCorpusAdds: () =>
        [
          "A\tfuzz/corpus/parser/seed_a.engawa",
          "A\tfuzz/corpus/parser/seed_b.engawa",
        ].join("\n"),
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(0);
    expect(r.status).toBe("ok-seed-added");
    expect(r.message).toContain("2 件");
  });

  test("複数 schema file が同時に変更されても、1 つでも substantive なら判定 true", () => {
    const deps = makeDeps({
      runDiffNames: () =>
        [
          "M\tcrates/engawa-format/src/feature.rs",
          "M\tcrates/engawa-format/src/document.rs",
        ].join("\n"),
      // どちらか片方に substantive change
      runDiffContent: () =>
        [
          "diff --git a/crates/engawa-format/src/feature.rs b/crates/engawa-format/src/feature.rs",
          "+    /// doc-only in feature.rs",
          "diff --git a/crates/engawa-format/src/document.rs b/crates/engawa-format/src/document.rs",
          "+    pub added: bool,",
        ].join("\n"),
      dirExists: () => true,
      runDiffCorpusAdds: () => "",
    });
    const r = runCheck(baseOpts, deps);
    expect(r.exitCode).toBe(1);
    expect(r.status).toBe("error-seed-missing");
  });
});
