# Quality Tools Manual Execution Guide

This document describes how to run the quality tools introduced in Phase 9:
proptest, criterion, cargo-fuzz, cargo-llvm-cov, and Playwright.

## 1. Proptest

Property-based tests are integrated into the test suite and run with `cargo test`.

```bash
cargo test -p engawa-kernel --test cuboid_determinism_proptest
```

To run with more cases (default 256):
```bash
PROPTEST_CASES=1000 cargo test -p engawa-kernel --test cuboid_determinism_proptest
```

## 2. Criterion Benchmarks

Manual benchmark execution:
```bash
cargo bench -p engawa-kernel --bench tessellation
```

To save a local baseline for comparison:
```bash
cargo bench -p engawa-kernel --bench tessellation -- --save-baseline local
```

To compare against your local baseline:
```bash
cargo bench -p engawa-kernel --bench tessellation -- --baseline local
```

The Phase 9 reference snapshot (`bench-results/baseline-phase9.json`) captures the performance at that phase for historical comparison. It is not a Criterion-replayable baseline—use `--save-baseline local` to create your own baseline for accurate comparisons.

## 3. Cargo-Fuzz

Fuzz targets are in `crates/engawa-format/fuzz/fuzz_targets/`.

To run a fuzzer (requires `cargo-fuzz` installed):
```bash
cargo install cargo-fuzz
cd crates/engawa-format/fuzz
cargo fuzz run from_yaml
```

With a corpus:
```bash
cargo fuzz run from_yaml fuzz/corpus/from_yaml/
```

To build fuzz targets without running:
```bash
cargo +nightly fuzz build from_yaml
```

## 4. LLVM Code Coverage (cargo-llvm-cov)

Requires `cargo-llvm-cov` installed:
```bash
cargo install cargo-llvm-cov
```

Generate HTML coverage report:
```bash
cargo llvm-cov --workspace --html
```

Open report:
```bash
open target/llvm-cov/html/index.html
```

Run without report (faster, for CI integration):
```bash
cargo llvm-cov --workspace --no-report
```

Coverage percentage only:
```bash
cargo llvm-cov --workspace --summary-only
```

## 5. Playwright

E2E tests are in `web/tests/`.

To run Playwright tests:
```bash
cd web
npm run playwright
```

Or directly:
```bash
cd web
npx playwright test
```

With UI mode:
```bash
cd web
npx playwright test --ui
```

Run specific test:
```bash
cd web
npx playwright test smoke.spec.ts
```

## Notes

- Benchmarks and fuzzing are manual-only operations (not integrated into `cargo xtask ci`)
- Proptest tests run as part of the regular test suite
- Playwright tests are included in `cargo xtask ci` via the existing xtask integration
