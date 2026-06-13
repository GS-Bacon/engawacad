# .engawa ファイルフォーマット仕様

## Overview

`.engawa` ファイルは YAML 形式のテキストファイル。

## Structure

```yaml
version: "0.1.0"           # カーネルバージョン
root_component:             # ルートコンポーネント
  name: "Part Name"
  transform:                # (省略可) 変換
    position: [x, y, z]
    rotation: [rx, ry, rz]  # オイラー角（度）
  features: [...]           # (省略可) 操作履歴
  children: [...]           # (省略可) 子コンポーネント
  ref: "path"               # (省略可) 外部参照
```

## Feature Types

### create_box
```yaml
- type: create_box
  id: unique_id
  width: 10.0     # X寸法
  height: 20.0    # Y寸法
  depth: 30.0     # Z寸法
```

### create_cylinder
```yaml
- type: create_cylinder
  id: unique_id
  radius: 5.0
  height: 20.0
```

### create_sphere
```yaml
- type: create_sphere
  id: unique_id
  radius: 10.0
```

### extrude
```yaml
- type: extrude
  id: unique_id
  sketch: sketch_id
  depth: 15.0
```

### Boolean Operations (cut, fuse, intersect)
```yaml
- type: cut          # or fuse, intersect
  id: unique_id
  target: body_id
  tool: body_id
```

## External References

コンポーネントは外部ファイルまたは標準ライブラリを参照可能:

```yaml
ref: "./other_part.engawa"                    # ファイル参照
ref: "stdlib://fasteners/jis_b1176/M5x20"   # 標準ライブラリ参照
```
