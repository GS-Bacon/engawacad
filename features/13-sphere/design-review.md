issues:
  - id: R01
    severity: critical
    section: "設計方針 > テッセレーション"
    finding: "`surface.evaluate(u, ±π/2)` をそのまま極点として使い、極の退化三角形を `push_triangle` の自動破棄に任せているが、f64 では `cos(π/2)` が 0 にならないため極行は単一点に潰れず微小リングになる。結果として極セルは退化せず、T07/T11 の 960 facet 前提が崩れ、極に穴のある非 watertight メッシュになり得る。"
    suggestion: "南北極は明示的に 1 頂点へスナップし、内部緯度帯だけを格子化して極は fan で閉じること。少なくとも `v==±π/2` は特別扱いし、boundary edge 数 0 / watertight を自動テストで検証すること。"
  - id: R02
    severity: high
    section: "設計方針 > テッセレーション"
    finding: "`Surface::Sphere` を一律 `UvSphere` に切り替える一方で、提案された `tessellate_face_sphere` は outer loop / seam / `t_range` を検証せず常に全周球を生成する。canonical な球プリミティブ以外の球面 face や壊れた loop でも、B-rep 境界を無視して full sphere を silent に出力してしまう。"
    suggestion: "`UvSphere` は canonical な 2-HE seam 球に限定し、outer loop が 2 HalfEdge・1 Edge・両端が極・半周 seam であることを検証できない場合は `TrimmedFaceUnsupported` か `UnsupportedSurface` を返すこと。"
  - id: R03
    severity: medium
    section: "設計方針 > トポロジー"
    finding: "seam `Curve::Circle` の `normal` と `t_range` が具体値で固定されていない。現行 `Curve::Circle` は `orthonormal_basis(normal)` に依存するため、ここを曖昧にすると『XZ 平面・+X 側・南極→北極』が -X 側や逆向きに化ける。"
    suggestion: "`normal = -Vec3::y()` と `t_range = [π, 2π]` のように実パラメータを設計で固定し、`t_start = 南極`、`t_mid = +X 赤道`、`t_end = 北極` を検証する幾何テストを追加すること。"
  - id: R04
    severity: medium
    section: "テスト計画 > T07/T11"
    finding: "期待 facet 数を『実装後に実測してピン留め』としており、実装結果をそのまま oracle 化する計画になっている。今回の極処理のようなバグがあると、誤った 1024 facet などを正解として固定してしまう。"
    suggestion: "facet 数はアルゴリズムから事前に式で決め、別途 watertight 性と極 fan の健全性を検証するテストを置くこと。実装由来の実測値をそのまま期待値にしないこと。"

verdict: fail