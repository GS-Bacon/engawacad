# debug-spec for #243 R3 — baseline 参考スナップショット化 + bench 精度改善 + clippy fix

## R2 で残った Codex 指摘 (blocking=1, 他は medium/low)

| ID | severity | persona | 対応 |
|----|----------|---------|------|
| C-F01 | high | contrarian | **修正必須**: baseline JSON が Criterion 標準形式でなく `--baseline phase9` で再現不能 → 参考スナップショット化して docs 訂正 |
| C-F02 | medium | contrarian | **修正**: tessellate bench に IdGenerator + make_cuboid を含めて測定している → `iter_batched` で setup 分離 |
| A-F02 | low | architect | **修正 (簡単)**: `from_yaml.rs` の未使用 `CStr` import 削除 |
| A-F01 | medium | architect | **棄却 (後続)**: proptest が `validate_manifold` / Euler-Poincare を assert していない → 本 Issue scope は "1 件最小 setup"、validation 強化は別 Issue (proptest を Boolean/Tessellation 不変量に拡張するのは Phase 10+) |

## R3 修正方針

### 1. baseline JSON 参考スナップショット化 (C-F01)

`bench-results/baseline-phase9.json` を **参考スナップショット** として明示し、docs/QUALITY_TOOLS.md の `--baseline phase9` 手順を訂正:

修正後の `bench-results/baseline-phase9.json` ヘッダコメント (JSON 内 metadata field):
```json
{
  "_note": "Reference snapshot only. Not a Criterion-replayable baseline. Generate fresh baseline locally with 'cargo bench --bench tessellation -- --save-baseline local' on your machine, then compare with '--baseline local'.",
  "captured_at": "phase9",
  "samples": [...]
}
```

`docs/QUALITY_TOOLS.md` の bench 章を訂正:
- `--baseline phase9` の cmd を削除し、ローカル baseline 保存 (`--save-baseline local`) → ローカル比較 (`--baseline local`) フローに変更
- `bench-results/baseline-phase9.json` は「Phase 9 時点の参考値 (絶対時間ではなく相対傾向比較用)」と明示

### 2. bench iter_batched 分離 (C-F02)

`crates/engawa-kernel/benches/tessellation.rs`:

```rust
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use engawa_kernel::primitives::make_cuboid;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::tessellate_solid;

fn bench_tessellate_cuboid(c: &mut Criterion) {
    c.bench_function("tessellate_cuboid_10x20x30", |b| {
        b.iter_batched(
            || {
                let mut gen = IdGenerator::new(0);
                make_cuboid(10.0, 20.0, 30.0, "cuboid", &mut gen).unwrap()
            },
            |solid| tessellate_solid(&solid).unwrap(),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_tessellate_cuboid);
criterion_main!(benches);
```

### 3. 未使用 import 削除 (A-F02)

`crates/engawa-format/fuzz/fuzz_targets/from_yaml.rs` の `use std::ffi::CStr;` (またはそれに類する未使用 import) を削除。

## 試した修正と結果

- [x] R1 GLM core: 5 ツール最小 setup → CI green
- [x] R1 STEP 7.5: Codex blocking=5 (criterion harness 未実装 + fuzz [[bin]] 抜け + docs cmd 誤り)
- [x] R2 GLM core: criterion 化 + fuzz [[bin]] 追加 + docs cmd 修正 → CI green
- [x] R2 STEP 7.5: Codex blocking=1 (architect+migration pass, contrarian のみ高: baseline 再現性)

## 次にやること

GLM core dispatch で C-F01/C-F02/A-F02 修正 → CI green → STEP 7.5 R3 再 Codex review。
A-F01 (manifold assert) は rejection.md に記録 (後続 Issue 候補)。
