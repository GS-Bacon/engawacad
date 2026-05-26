# ADR-004: 自由曲面・自由曲線を確定要件として扱う

## Status

Accepted

## Context

現在の幾何カーネルは解析曲面・解析曲線のみを持つ:

- `Surface` (`crates/mycad-kernel/src/geometry/surface.rs`): `Plane` / `Cylinder` / `Sphere` / `Cone`
- `Curve` (`crates/mycad-kernel/src/geometry/curve.rs`): `Line` / `Circle`

「解析曲面だけで進められないか」を検討したが、ロードマップ上の操作を踏むと自由曲面・自由曲線は構造的に避けられないと結論した:

- **Boolean 演算 (Phase 4)**: 曲面同士の交線は一般に円・直線にならない。例として円筒 ∩ 球の交わりは 4 次の空間曲線であり、`Curve::Line` / `Curve::Circle` では表せない。→ 自由**曲線** (交線・スプライン) が必須。
- **フィレット・面取り (ADR-001 が動機として挙げた操作)**: 一般のフィレット面や掃引面は解析式に乗らない。→ 自由**曲面** (NURBS 等) が必須。

一方、次の理由から「今すぐ実装する必要はない」:

- `Surface` / `Curve` は Rust の enum であり、variant 追加は加算的。`evaluate` / `normal_at` / tessellation などの `match` はコンパイラが網羅性を強制するため、追加漏れは検出される。
- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。

## Decision

自由曲面・自由曲線を「いつか検討する」ものではなく **確定要件** として扱う。ただし NURBS 等の実装は今は行わず (現在 Phase 1)、Boolean が要求する範囲から漸進的に導入する。これに伴い、以降の設計で次を守る:

1. **`Surface` / `Curve` enum を唯一の幾何拡張点**とする。`Nurbs` / スプライン variant の追加が加算的であり続けるよう、アルゴリズムは variant 集合を仮定しない。
2. **アルゴリズムは曲面・曲線の型に非依存**であること。平面・直線前提をアルゴリズムへ埋め込まない (例: 面法線は面ごとに 1 回でなく、点ごとに `Surface::normal_at_point` で評価する)。
3. **数値モデル (トレラント方式 vs 厳密方式) は本 ADR では保留**し、曲面 Boolean を実装する Phase 4 着手時に決定する。自由曲面の交線は数値的近似になるため、業界カーネル (Parasolid / ACIS) はエンティティ毎に公差を持つトレラントモデルを採る。これを後から全エンティティ・全比較へ導入するのは大改修であり、Phase 4 で意思決定する論点として明示しておく。なお `CLAUDE.md` の **決定性** 原則とは両立可能 (アルゴリズム固定で再現できる) だが、数値ロバスト性の確保が決定性維持の難所になる点に留意する。

## 単位系・グローバル公差 (暫定)

### 内部標準単位

| 次元 | 単位 | 備考 |
|------|------|------|
| 長さ | mm | カーネル内部幾何の標準 |
| 角度 | rad | カーネル内部幾何の標準 |
| 比率 | 無次元 | スケール比例の相対比較 |

### deg/rad 境界

`.mycad` フォーマットの `Transform.rotation` は既存どおり**度数法 (オイラー角, deg)** (`component.rs`・`docs/file-format.md` 参照)。カーネル内部幾何は rad。**deg↔rad 変換は format/build 層の責務**であり、カーネルは rad のみを扱う。

### 公開公差定数

`mycad-kernel` は以下の 3 定数を公開 (`geometry::math` モジュール / crate root から再エクスポート):

| 定数 | 値 | 用途 |
|------|-----|------|
| `LENGTH_TOLERANCE` | `1e-9` (mm) | 距離・座標の絶対比較 |
| `ANGLE_TOLERANCE` | `1e-9` (rad) | 角度・パラメータの絶対比較 |
| `RELATIVE_TOLERANCE` | `1e-9` (無次元) | スケール比例の相対比較 |

これらは暫定の単一スカラ値。Phase 4 でトレラントモデルへ移行する際、次元付き公差として差し替える。

### 比較 helper 関数

上記定数に基づく共通比較関数を `geometry::math` に集約:

- `length_near(a, b) -> bool`
- `angle_near(a, b) -> bool`
- `point_near(a, b) -> bool`
- `point_near_scaled(a, b, scale) -> bool`

### 内部実装詳細

- **Cone apex singularity guard**: `surface.rs` 内の `pub(crate) const APEX_TOLERANCE: f64 = 1e-12` は cone 頂点の特異点ガード。構造的退化検出のため `LENGTH_TOLERANCE` より厳しい。公開契約・crate root 再エクスポートには含めない。
- **面積退化閾値 `AREA_EPS`**: tessellation 内の private 定数。退化三角形除去の heuristic 閾値 (次元付き公差ではない)。`push_triangle` 内の cross-product norm 計算式に既知の誤り (`.sqrt()` の適用位置) があり、[#29](https://github.com/GS-Bacon/mycad/issues/29) として別 Issue 化済み。Phase 4 着手前に修正する。

### Document 単位フィールド

`Document` に単位フィールドは追加しない。理由: (1) byte-exact golden YAML・TS/JsonSchema を壊さない、(2) 単位の消費者が不在 = YAGNI。将来のインポート/エクスポート機能で必要になれば追加する。

### 拡張点

質量 (g)・密度等の物理単位は本 Issue では扱わず、将来の質量特性/材料 feature 着手時に決定する。公差の次元別分離が `MASS_TOLERANCE` 等を加算的に足す継ぎ目になる。

Phase 4 でのトレラントモデル移行については Decision 3 を参照のこと。

## Rationale

- **要件確定だけ先に行う理由**: 後付けが致命的に痛い横断的決定 (上記 2・3) を「事故的に」決めてしまわないため。実装そのもの (NURBS 評価・Boolean アルゴリズム) は enum + Feature 再生成により安全に後回しでき、先回り実装はむしろ過剰設計になる。
- **専用 Phase を立てない理由**: 自由曲面を独立フェーズにすると、どの操作がそれを必要とするかと切り離されて陳腐化する (ADR-002 の「空想 Issue は作らない」と同趣旨)。Boolean / フィレットが要求した時点で必要分を入れる。

## Implementation Details

- 本 ADR は ADR-001 (B-rep 採用) を拡張する位置づけ。番号 003 は ADR-002 で Viewer スタック用に予約済みのため 004 を使用。
- **未決の横断的論点 (Phase 4 で再検討)**:
  - **pcurve (パラメータ空間曲線)**: トリムされた周期・自由曲面を tessellation / Boolean で安定に扱うには、half-edge ごとに面のパラメータ空間上の 2D 曲線を持たせる構造が事実上必要になりうる (OpenCASCADE 等が保持)。現在の `Edge` は 3D `curve + t_range` のみで pcurve を持たない。
  - **数値モデル (トレラント vs 厳密)**: 上記 Decision 3。
- 本 ADR 採択に伴う即時対応: `tessellate_solid` が面法線を `normal_at(0.0, 0.0)` で 1 回だけ評価していた平面前提を解消し、`Surface::normal_at_point` による頂点毎評価へ変更した (平面では従来と同一の結果)。
