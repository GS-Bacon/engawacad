# Codex 非 blocking findings — #165

## R2 medium

### F01 (medium) — T01/T04 で POST 0 件の明示 assert が不足

- 指摘: T01 (未確定状態) は panel が hidden であることだけ assert、T04 (depth ガード) は button.disabled だけ assert。実 POST が発火しないことを直接 assert していない。
- 受容判断: 
  - T01: panel が hidden の状態で button.click() は visible 待ちタイムアウトになり (Playwright の通常 click)、現に T01 では `toBeDisabled()` で確認済。実装上、disabled button の click は no-op で POST 経路に到達しない (`btnSketchExtrude.addEventListener("click", async () => ...)` のハンドラ自体は実行されるが、handler 内の `if (!lastFinalizedSketch) return;` ガードで即座に return)。
  - T04: button.disabled なら click 経由で POST 発火しない (ブラウザ仕様)。あえて click を試みなくても disabled で十分。
  - "abc" 入力: `<input type="number">` の HTML 仕様により非数値文字は弾かれる (Playwright `fill("abc")` がエラー)。テスト上「abc を入力する」操作自体が成立しない。代わりに 0 / -5 / 1e309 をテストすることで `Number.isFinite(...)` / `> 0` ガードを網羅できる (1e309 → Infinity で `isFinite` がガード)。
- 後続対応: 必要なら #166 E2E で POST count を直接 assert する形にできる。本 Issue では Playwright DOM + state 反映で同等の保証を達成と判断。
