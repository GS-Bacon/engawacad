# ADR-001: B-rep (Boundary Representation) の採用

## Status

Accepted

## Context

3D CAD カーネルの形状表現方式を選択する必要がある。主な候補:

1. **CSG (Constructive Solid Geometry)**: ブーリアン演算のツリーとして形状を表現
2. **B-rep (Boundary Representation)**: 面・辺・頂点で境界を表現
3. **Voxel / Implicit**: 暗黙的な距離関数で表現

## Decision

B-rep を採用する。

## Rationale

- **エッジ・フェイスへの直接アクセス**: フィレット、面取り、面選択等の操作に必要
- **業界標準**: STEP, IGES 等の標準フォーマットが B-rep ベース
- **テッセレーションが直接的**: 面を直接メッシュ化できる
- **CSG との併用可能**: ブーリアン演算の結果を B-rep として保持

## Implementation Details

- トポロジーエンティティはフラット配列に格納（index-based）
- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
- 各エンティティは `EntityId` を持ち、Feature からの参照に使用
