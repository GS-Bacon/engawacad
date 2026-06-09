## Codex r3 non-blocking findings

### F01 (medium)
depth.abs() > 1e12 カーネルガードに対し、フロントは Number.MAX_VALUE を valid と期待したまま（T07）。
フロントで 1e12 超を reject するか、カーネル上限を contract として明示的に文書化する必要がある。
→ 次 Issue (Phase 7 スケッチ) 着手時に web/src/extrude.ts へ `depth > 1e12 → null` を追加予定。
