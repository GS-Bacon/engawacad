<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R01: 採用 → plan の「...」節を修正 -->
<!-- - R02: 棄却 → Non-Goals に記載済みの蒸し返し -->
<!-- - R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue に委譲 -->

## Round 1

### 採用 (2 件)
- AM01 (medium): Conic AABB/サンプリング手法未定義 → plan.md 「実装対象 > after」の Conic case コメントに主軸変換 + 固定サンプリング (base_segments × 2) を明記
- AM02 (medium): T03b assert 条件曖昧 → plan.md テスト計画表で T03/T03b の期待結果に具体的 assert 条件 (点数 64、conic 評価誤差許容、x>0 等) を記載

### 棄却 (0 件)

## Round 2

### 採用 (2 件)
- SC01 (medium): Out-of-Scope に過去 Issue を入れるのは混乱 → 「Circle / Arc — Issue #273 (closed)」行を削除し、parabola の通常 tessellation 除外行に置き換え
- NU01 (medium): Conic 固有値の数値安定性が未定義 → plan.md 設計方針 §数値モデル に「数値安定性 (Conic 主軸変換)」段落を追加 (Jacobi 法/closed form、cancellation は base_segments × 2 で mask)

### 棄却 (1 件)
- IN01 (low): plan.md は math.rs に EPS_AXIS_RATIO/EPS_DISCRIMINANT 追加と記載するが ADR-017 §2 は tolerances.rs を指定 → **棄却**。理由: 実コードでは `LENGTH_TOLERANCE`/`ANGLE_TOLERANCE` は `crates/engawa-kernel/src/geometry/math.rs` に存在 (#273 でも math.rs)。`tolerance.rs` (単数形) は `Tolerance` 型用で定数置き場ではない。ADR-017 §2 の "tolerances.rs" は方向性表記で、実装上は既存配置 (math.rs) との整合を優先する
