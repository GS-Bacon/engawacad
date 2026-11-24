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

## Phase 11 設計契約 — 拘束付きスケッチの保存と再生成

この節は #196 で実装する拘束付きスケッチの契約を定める。現行の拘束を持たない `CreateSketch.profile` では、作者が指定した座標をそのまま入力として扱う。

| 情報 | 所有者 | 保存と再生成 |
| --- | --- | --- |
| スケッチ平面、要素と端点の安定 ID、固定座標、駆動寸法の値・式と変数、幾何拘束とその参照先 | 文書の Feature 履歴（engawa-format） | 入力として保存する。拘束の参照は ID と端点の役割で表す。式を保存した寸法は履歴再生時にそのスコープで評価する。 |
| 複数の解のうち採用する枝 | 同じ Feature 履歴 | 作者が選んだ幾何関係を保存する。例えば、順序付きの基準点 A, B に対して点 P が左側か右側かを記録する。この順序と側を再生にも使用する。 |
| 解いた点の座標、従動寸法の測定値、拘束の充足状態 | ソルバと engawa-build の再生結果 | 入力の変更・再読込のたびに計算する。派生結果は文書に書き戻さない。 |
| 描画用ポリライン、メッシュ | テッセレータとビューア | 表示精度に応じて生成・破棄できる。表示点を拘束や次の Feature の参照先に使わない。 |

engawa-build は履歴順に参照と寸法式を解決し、平面内の解析要素、固定値、数値化した駆動寸法、拘束、枝の指定をソルバへ渡す。ソルバは拘束の残差と枝の条件を検証し、解いた座標を元の要素 ID に対応づける。後続の編集や押出が kernel に渡す入力は、この座標から組み立てた平面と解析プロファイルである。同じ文書は空のキャッシュから再生しても同じ枝を選ぶ。参照の消失、解の消失、枝が区別できなくなる退化時は対象の拘束と参照を示す診断を返し、その結果に依存する Feature の生成を止める。

### 二解を持つスケッチでの受入例

固定点 A=(0,0), B=(6,0) と可動点 P を作る。距離 AP と BP を同じ正の駆動寸法 r に拘束する。候補は P=(3, ±√(r²−9))。スケッチのローカル XY 座標で `cross(B−A, P−A) > 0` を「A から B を見て左」とする。

1. r=5 で上側の解を選び、点 ID、A と B の固定座標、二つの距離拘束と r、順序 (A,B) に対する P の「左」を保存する。
2. 保存直後も、画面を閉じて再読込した直後も、左を満たす P=(3,4) を得る。kernel に渡す要素はこの点を使う。
3. r を √73 に変えると、同じ枝の P=(3,8) を得る。r だけを保存し直し、再読込しても同じ座標を再生成する。
4. r=3 では左右の解が接して側を定義できず、r<3 では実数解がない。それぞれ枝の退化と拘束不成立を診断する。再び r=5 に戻せば、保存した枝から P=(3,4) に戻る。
