# MyCad Architecture

## Overview

MyCad は Feature-based parametric CAD システム。ユーザーの操作履歴（Feature 列）を真実の源とし、B-rep 形状はそこから再生成される。

## Layer Structure

```
┌─────────────────────┐
│     mycad-cli       │  CLI / UI 層
├─────────────────────┤
│    mycad-viewer     │  可視化層（テッセレーション結果を描画）
├─────────────────────┤
│    mycad-format     │  永続化層（YAML <-> Document/Feature/Component）
├─────────────────────┤
│    mycad-kernel     │  コア層（B-rep, geometry, primitives, tessellation）
└─────────────────────┘
```

## Key Design Decisions

### B-rep (Boundary Representation)

形状を「境界面の集合」として表現する。CSG (Constructive Solid Geometry) より複雑だが、エッジ・フェイスへの直接アクセスが可能で、フィレット・面取り等の操作に適している。

### Index-based Topology

`Solid` 内のトポロジーエンティティ（Vertex, Edge, Face 等）はフラット配列に格納し、ポインタではなくインデックスで相互参照する。

**理由:**
- シリアライズが容易
- 決定性の保証が簡単
- メモリレイアウトがキャッシュフレンドリー
- 所有権問題（循環参照）を回避

### Feature History as Source of Truth

`.mycad` ファイルに保存される Feature 列が設計の唯一の源。B-rep 形状は Feature 列から毎回再生成可能。これにより:
- パラメータ変更時の再生成が可能
- undo/redo が自然に実装できる
- ファイルサイズが小さい（形状データではなく操作手順のみ）

### YAML Format

設計ファイルに YAML を採用。

**理由:**
- 人間が読み書きできる
- git diff でレビューできる
- スキーマバリデーション可能（schemars で JSON Schema 生成）
- 拡張子: `.mycad`

### Kernel の Rendering 非依存

`mycad-kernel` はレンダリングライブラリに一切依存しない。テッセレーション結果（`TriangleMesh`）を生成するのみ。ビューアがそれを消費する。

## Data Flow

```
.mycad file → Document → Feature列 → Kernel (B-rep Solid) → Tessellation → TriangleMesh → Viewer
```
