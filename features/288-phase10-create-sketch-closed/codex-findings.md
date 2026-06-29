# Codex final review findings (non-blocking)

Round 2 verdict: `blocking == 0` / 3 persona aggregate=pass。
Medium 4 件は記録のみ非 block。**いずれも本サイクル内で対応済**。

## C-F01 / M-F02 (medium) — T05 を `[Circle, Line]` に差し替え

**finding**: `t05_extrudecut_rejects_multi_closed` が `[Circle, Circle]` を使っているため、
`ExtrudeCut` 分岐の `validate_sketch_profile_contours` 呼び出しが downstream luck で
同 error variant に落ちる入力でカバーされており、guard 固有の固定にならない。

**対応**: T05 を plan.md 通り `[Circle, Line]` に差し替え、関数名も
`t05_extrudecut_rejects_circle_plus_line` に rename。これで `lib.rs:367` の
ExtrudeCut 側 call site を guard 削除すると fail する形で明示的に固定。

## C-F02 / M-F01 (medium) — 正常系 body 数 assert

**finding**: `build_with_profile` / `build_with_profile_extrude_cut` が
`.map(|_| ())` で BuiltBodies を捨てているため、T01/T02/T08 の正常系は `Err`
にならないことしか検証できておらず、plan の「1 Solid 生成」契約を実際に固定できない。

**対応**:
- ヘルパー signature を `Result<BuiltBodies, KernelError>` に変更
- T01 / T02 で `bodies.all().len() == 1` を assert (Codex 提案通り)
- T08 (単一 Ellipse) は make_extrusion が Ellipse profile の場合に Ok を返すか
  downstream で別 error に落ちるか未確定のため、既存の `!matches!(Err(InvalidParameter))`
  形式を維持 (guard 通過のみを assert する design 維持、body 数 assert は
  別 Issue で扱う Ellipse extrusion 正規実装後に追加)

## verdict

Codex round 2 で aggregate `verdict: pass / blocking: 0`、本サイクルで全 medium に
対応済。`cargo test -p engawa-build --test create_sketch_closed_acceptance` で
13 件全件通過確認 → STEP 8 squash merge へ進む。
