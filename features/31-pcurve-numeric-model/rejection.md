# Codex 設計レビュー 棄却 log — Issue #31

## Round 3

- **R02 (medium)** —「`Curve2D` の variant payload private 化は `pub enum` では成立しない。外部から `Curve2D::Line2D { direction: (0.0, 0.0) }` 直接構築できる」を **棄却**。

  **理由**:
  1. **scope 外の workspace 規約論**: pub enum + 公開 struct-like variant フィールドの組み合わせで直接構築できる問題は、Rust の構文上の制約で、本リポジトリの公開型 (`Curve` / `Surface` / `EntityRef` / `Feature` 等) すべてが同様の構造を持つ。#31 が `Curve2D` だけ opaque enum (`Line2D(Line2DRepr)` 形式) にすると workspace 内で一貫性が崩れる。opaque enum を全体規約として導入するなら ADR/別 Issue で議論すべきで、#31 単独で先行決定すべきでない。
  2. **実質的な不正侵入経路は既に塞がれている**:
     - YAML/JSON 境界: Round 2 R01 で custom Deserialize → `try_*` ルーティング済み
     - 公開 API: `Curve2D::try_line` / `try_circle` のみが推奨構築経路 (rustdoc で明示)
     - 唯一残る穴は kernel 内部 Rust コードでの literal 構築だが、これは `tessellation` / `booleans` / `topology.rs` の自分が書く実装側で律する範囲 — 実装方針として「`Curve2D` の literal 構築は禁止、必ず `try_*` 経由」を CLAUDE.md に追加する程度で十分。
  3. **取らない場合のコスト < 取る場合のコスト**: opaque 化すると pattern match での read が `match curve_2d { Curve2D::Line2D(repr) => repr.origin(), ... }` 形式になり、tessellation 内のサンプリング処理が読みづらくなる。kernel 内コードのコスト増 > literal 構築バグの実害 (kernel 内部で `(0.0, 0.0)` direction を literal で書く理由がそもそもない)。

  **代替措置**:
  - rustdoc `#[doc = "..."]` で `Curve2D` の variant 構築は「`try_line` / `try_circle` を必ず使え」と明記
  - kernel CLAUDE.md (`crates/mycad-kernel/CLAUDE.md`) に「`Curve2D` の literal 構築禁止」を追加 (実装フェーズで GLM が編集する)
  - 将来 opaque enum 規約を入れるなら別 Issue として ADR で扱う
