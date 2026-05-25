issues:
  - id: R01
    severity: medium
    section: "設計方針 > 決定性"
    finding: "`normals` 欠落時に `geometry.computeVertexNormals()` をクライアントで実行しており、『API レスポンスをそのまま BufferGeometry 化し、独自の幾何生成はしない』方針と矛盾している"
    suggestion: "法線は Rust/API 側で常に生成して返すか、欠落時は viewer で再計算せず明示的エラーにすること"
  - id: R02
    severity: medium
    section: "設計方針 > CI 統合"
    finding: "`xtask ci()` で lockfile 不在時に `npm install` へフォールバックすると依存解決が時点依存になり、`package-lock.json` による版固定と整合しない"
    suggestion: "`web/package-lock.json` を必須にし、欠落時は `npm ci` を失敗させて CI を赤にすること"
  - id: R03
    severity: medium
    section: "テスト計画"
    finding: "決定性反復テスト（T0x 系）が無く、同一 `TriangleMesh` 入力から同一 `BufferGeometry` 変換結果が得られることを自動検証していない"
    suggestion: "固定 mesh fixture に対する `TriangleMesh -> BufferGeometry` の反復比較テストを追加し、少なくとも attribute 長・index 内容・bbox を繰り返し検証すること"
  - id: R04
    severity: medium
    section: "テスト計画 > 退化/エッジ"
    finding: "退化・不正入力の扱いが `indices` 空と file エラーに偏っており、`normals` 長さ不一致、index 範囲外、NaN/inf 座標など malformed `TriangleMesh` の方針と自動検証が不足している"
    suggestion: "viewer 初期化前に mesh の長さ整合・index 範囲・有限値を検証し、異常系テストを追加すること"

verdict: pass