# ADR-007: アセンブリと部品参照の設計方針

**Date**: 2026-06-06  
**Status**: Accepted  
**Supersedes**: ADR-005 §「参照スコープ」の Phase 5 前方拡張予告 (跨ぎBooleanをPhase 6+へ延期)  
**Related**: ADR-002 (ロードマップ管理), ADR-005 (トポロジカル命名), ADR-006 (Issue分解規約)

---

## 背景

Phase 4 (Boolean) 完了後、Phase 5 ではアセンブリと部品参照を実装する。完了条件は:

1. `stdlib://` 参照解決
2. Component 階層の transform 適用
3. `examples/assembly.mycad` が動作 (3部品が正しい位置で export/view できる)

フォーマット層 (`mycad-format`) は既に完成しており、`Component` / `Transform` / `ComponentRef` の
型定義・バリデーション・ラウンドトリップはすべてテスト済み。未実装は「ビルド・export 層から先」のみ。

---

## 決定事項

### 1. transform を B-rep 本体に焼き込む (案A)

transform (位置・回転) を、ソリッドを構成する幾何型 (`Point`/`Plane`/`Cylinder`/`Sphere`/
`Cone`/`Line`/`Circle` および `pcurve`) に直接適用する。テッセレーション後のメッシュだけを変換する
案 (案B) は採用しない。

**理由**:
- 「厳密B-repが真実の源」原則 (ADR-004) に忠実。B-repが世界座標を持つことで、将来の
  アセンブリ間Boolean・STEP配置がそのまま乗る。
- 案Bはビューア表示は動くが B-rep は原点に留まり、将来の機能で必ず作り直しになる。

### 2. 平行移動を先に実装し、回転は独立 Issue で実装する

**理由 (リスク分析)**:
- 回転リスクは 3 点: ①Euler角 → sin/cos の浮動小数 (90°が厳密に揃わない) ②Cylinder/Cone の
  パラメータ基準方向 (`orthonormal_basis(axis)` 由来) のズレ ③Sphere シームと pcurve の不整合。
- `examples/assembly.mycad` の回転は全て `[0,0,0]` (平行移動のみ)。
  → 平行移動だけで完了条件を満たせる。回転の実装を急ぐ理由がない。
- 平行移動は「点・原点・中心を一律ずらす」だけで pcurve・軸・半径は無傷。

**回転の数値モデル (回転Issue実装時に準拠すること)**:
- オイラー角 (度) → ラジアン → ZYX 順序の回転行列 (3×3) を合成。
- 90°/180°/270° の特別扱い: `(n × 90°).to_radians().sin()` が完全に 0.0 または 1.0/-1.0 に
  ならない場合は丸め込みを行い幾何的な整合を保証する。しきい値は `1e-15`。
- `Point` / `Curve` / `Surface` の各変換: 点は「回転+平行移動」、法線・軸ベクトルは「回転のみ」
  (平行移動しない)。この使い分けを型レベルで保証するユーティリティを `geometry/transform.rs` に置く。
- Cylinder: 変換後の `orthonormal_basis(new_axis)` が元の基準を回転したものと一致することを
  数値テスト (精度 `1e-12`) で確認する。
- 決定性テスト: 変換後のソリッドをシリアライズして 100 回同一出力を確認。

### 3. stdlib の物理形式と解決機構

- `stdlib://X` → `<stdlib_root>/X.mycad` を読み込んでビルド。
- `stdlib_root` の決定順: env `MYCAD_STDLIB_PATH` → リポジトリ内 `stdlib/` (Cargo.toml 起点)。
- ファイル参照 (`ComponentRef::File`) の解決機構を同じコードパスで実装し共通化する。
- 再帰ロード: `ComponentRef` を持つ Component は、参照先 `Document` をロードし
  その `root_component` を自身の子として展開してビルドする。
- **循環参照検出**: ロード中の Document パス集合を visit-set として持ち、同じパスが再度現れたら
  `KernelError::CircularReference` で即時エラー。
- **深さ上限**: デフォルト 16 段。超過時は `KernelError::MaxDepthExceeded` でエラー。

### 4. 簡略 M5x20 ボルト

`stdlib/fasteners/jis_b1176/M5x20.mycad` を既存プリミティブで構成:
- 軸: `CreateCylinder` (radius=2.5mm, height=20mm)
- 頭: `CreateCylinder` (radius=4.5mm, height=3mm, origin=[0,20,0])  
  ※ネジ山・六角形状は作らない。Phase 5 の完了条件は「ボルトが正しい位置に見える」まで。

stdlib のフォルダ規約: `stdlib/<category>/<standard>/<part_number>.mycad`

### 5. 部品跨ぎ Boolean とネーミング跨ぎ拡張は Phase 6+ へ延期

ADR-005 は「Phase 5 で `(component/occurrence path, feature_id, kind, role)` に前方拡張する」と
予告していたが、これを **Phase 6+** へ延期する。理由:

- `assembly.mycad` は部品を並べるだけで完了条件に跨ぎBooleanは含まれない。
- ADR-006 の教訓: 「ADR決定と実装の混在」「Issue肥大化」が Phase 4 遅延の主因。
  跨ぎBooleanは 1 Issue に収まらない大規模変更になる。
- 案A (B-rep世界座標化) により、各ソリッドは最初から世界座標に配置される。
  跨ぎBooleanは将来「同じ座標系のソリッド同士を合成するだけ」になり、土台の作り直しは不要。

**跨ぎ参照の記法予約 (前方互換)**: 将来の拡張時には `"ComponentName/feature_id"` の
スラッシュ区切りパス表記を使う。この記法を ADR-005 と整合させ、既存の単一 Component 内参照
 (スラッシュ無し) と衝突しない。現 Phase では実装・バリデーションともに対象外。

---

## ASSEMBLY ペルソナ (GLM設計レビュー / ADR-006 §4 準拠)

Phase 5 の GLM 設計レビューに追加するペルソナ:

| ペルソナ | 観点 |
|---|---|
| ASSEMBLY | ① transform 合成の順序 (親→子の適用順) が正しいか ② 参照解決の循環・深さガードが全パスで有効か ③ 平行移動・回転で EntityID が変わっていないか (決定性) ④ pcurve と曲面パラメータの整合が transform 後も保たれるか ⑤ stdlib_root が未設定の場合の fallback が明記されているか |

Common 3 (SCOPE / INVARIANT / AMBIG) + ASSEMBLY = Phase 5 の標準ペルソナ構成。  
NUMERIC (tolerance/ε) は回転 Issue (#6) のみ Heavy モードで追加する。

---

## 影響を受けるファイル

| ファイル | 変更内容 |
|---|---|
| `crates/mycad-kernel/src/geometry/transform.rs` | 新規: 平行移動・回転ユーティリティ |
| `crates/mycad-kernel/src/geometry/surface.rs` | `translate` / `rotate` メソッド追加 |
| `crates/mycad-kernel/src/geometry/curve.rs` | `translate` / `rotate` メソッド追加 |
| `crates/mycad-kernel/src/brep/topology.rs` | Solid への translate 適用 |
| `crates/mycad-build/src/lib.rs` | `build_assembly` 関数追加 (ツリー走査 + 参照解決) |
| `crates/mycad-cli/src/main.rs:74` | assembly 拒否ガードを外す |
| `crates/mycad-api/src/handler.rs:23` | assembly 拒否ガードを外す |
| `stdlib/fasteners/jis_b1176/M5x20.mycad` | 新規: 簡略ボルト定義 |
