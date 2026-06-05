## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `docs/decisions/005-topological-naming.md` §7「canonical local frame の必須化」段落直後に position パラメータの扱いを 1 段落追記 | コード変更 (feature.rs / kernel / build) — impl Issue #48 |
| 追記文に「position 省略時は canonical 原点(0,0,0)デフォルト・既存 example YAML 不変で有効」を明記 | 回転 (rotation/axis) パラメータの ADR 決定 — 別 Issue |
| `cargo xtask ci` が green (docs 変更のみ) | Component.transform の扱い — Phase 5 |

## Non-Goals
- コード変更 (feature.rs の CreateCylinder/CreateSphere へのフィールド追加): impl Issue #48 で実施
- 回転 (rotation) パラメータの canonical frame への組み込み方針: 別 Issue で決定
- Component.transform の扱い: Phase 5 マター、別 Issue
- position の数値表現・tolerance 議論: 数値モデルは ADR-004/Phase 4 実装側 (#48) のスコープ

## 実装対象
<!-- Issue: #47 -->
<!-- 影響ファイル: docs/decisions/005-topological-naming.md (docs のみ。crates 変更なし) -->

`docs/decisions/005-topological-naming.md` の §7 内、`canonical local frame の必須化` 段落
(現 151–154 行: 「…具体 frame と role 表は各 primitive の role 付与実装 issue で確定する。」で終わる段落)
の**直後**、`**face role の例**:` (現 156 行) の**前**に、以下の段落を新規挿入する。

### before (挿入位置)
```markdown
...具体 frame と role 表は各 primitive の role 付与実装 issue で確定する。

**face role の例**:
```

### after
```markdown
...具体 frame と role 表は各 primitive の role 付与実装 issue で確定する。

**位置パラメータの扱い**: canonical local frame を導く feature パラメータには、形状パラメータ
(radius/height 等) に加え位置パラメータ (`CreateCylinder` の origin、`CreateSphere` の center 等) を含む。
position は frame の**原点を決めるだけ**で、role 名 (`lateral`/`cap_top`/`seam`/`surface` 等) や
内部 canonical name grammar `<feature_id>;<kind>:<role>` には影響しない。position を省略した場合は
canonical 原点 (0,0,0) にデフォルトし、既存の example YAML は不変のまま有効である。
回転 (rotation) の扱いは別 Issue で決定する。

**face role の例**:
```

## 設計方針
- **コード変更なし**: 本 Issue は ADR ドキュメントへの 1 段落追記のみ。決定性・B-rep トポロジー・derive 規約・エラーハンドリング・workspace.dependencies は **N/A**。
- **§7 不変条件との整合**: §7 は「role は feature パラメータから導く canonical frame に従い、内部リファクタで別名化させない」と規定する。position は frame **原点**を動かすが role grammar には現れないため、この不変条件と矛盾しない。本追記はその範囲を position まで明示するもの。
- **後方互換**: 現状 `CreateCylinder { id, radius, height }` / `CreateSphere { id, radius }` は position フィールドを持たない (feature.rs:276-284 で確認)。追記文に「省略時は canonical 原点(0,0,0)デフォルト」を明記することで、#48 で origin/center を追加しても既存 example YAML が壊れない設計根拠を ADR に残す。
- **prospective 記述**: origin/center は #48 で追加予定の forward-looking なパラメータ。ADR は設計の真実の源なので先に方針を確定し、#48 がそれを実装する。

### 数値モデル
N/A — docs-only。tolerance/ε は impl Issue #48 (ADR-004/Phase 4 数値モデル) のスコープ。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | ビルド | `cargo xtask ci` が green | docs 変更のみでコンパイル・既存テストに影響なし |
| T02 | ドキュメント | §7 に「位置パラメータの扱い」段落が存在し、原点デフォルト・role 不変・rotation 別 Issue の 3 点を含む | 目視 / grep で文言確認 |

<!-- docs-only Issue のため自動テスト追加なし。acceptance skeleton (STEP 5.5) も N/A。 -->

## 幾何的不変条件チェックリスト
- [ ] N/A — 本 Issue は docs 追記のみで Boolean/Partition/Assemble に該当しない
- [ ] N/A
- [ ] N/A
- [ ] N/A
