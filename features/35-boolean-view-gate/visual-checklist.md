# Boolean 目視確認チェックリスト (Issue #35)

## 確認観点

各 example で以下の 4 点を確認する:

- ① 形状が期待通り (穴の位置・サイズ、結合形状、切断結果)
- ② 法線の向きが正しい (外側が明るく、内側が暗い / 表裏の塗り分けが正しい)
- ③ 退化三角形・メッシュの抜けが見えない (穴や黒いパッチが出ない)
- ④ 交線が滑らかに閉じている (曲面 Boolean ケース)

## ビューアの起動方法

```bash
# 別ターミナルで cargo build -p mycad-cli を済ませてから:
cargo run -p mycad-cli -- view examples/<ファイル名>.mycad
# → http://127.0.0.1:7878 をブラウザで開く
# Ctrl-C で終了し次の example へ
```

---

## 平面 Boolean (必須 3 本)

### 1. boolean_box_fuse — box ∪ box

```bash
cargo run -p mycad-cli -- view examples/boolean_box_fuse.mycad
```

期待: 2×2×2 の同位置 box が合体 → 外形は 2×2×2 の box (面が消えて滑らか)

- [ ] ① 2×2×2 の直方体が表示される
- [ ] ② 法線が正しい (全面外向き、均一な明暗)
- [ ] ③ 退化三角形・抜けなし
- 結果: ___

### 2. boolean_box_cut — box − box

```bash
cargo run -p mycad-cli -- view examples/boolean_box_cut.mycad
```

期待: 2×2×2 から 1×1×1 を切り取った L 字/段付き形状

- [ ] ① 段付き直方体が表示される (大きい box から小さい box が除かれている)
- [ ] ② 法線が正しい (切断面も外向き)
- [ ] ③ 退化三角形・抜けなし
- 結果: ___

### 3. boolean_box_intersect — box ∩ box

```bash
cargo run -p mycad-cli -- view examples/boolean_box_intersect.mycad
```

期待: 2×2×2 同位置 box の共通部分 → 2×2×2 の box (同一 = 元のまま)

- [ ] ① 2×2×2 の直方体が表示される
- [ ] ② 法線が正しい
- [ ] ③ 退化三角形・抜けなし
- 結果: ___

---

## 平面 Boolean (bonus 1 本)

### 4. boolean_box_void — 内部 void shell

```bash
cargo run -p mycad-cli -- view examples/boolean_box_void.mycad
```

期待: 4×4×4 の外殻から 2×2×2 の内部を切り取った中空直方体

- [ ] ① 外殻が表示される (中空だが外から見ると普通の直方体)
- [ ] ② 法線が正しい (外面が外向き)
- [ ] ③ 退化三角形・抜けなし
- 結果: ___

---

## 曲面 Boolean (必須 2 本)

### 5. boolean_cut_cylinder_hole — box − cylinder = 丸穴

```bash
cargo run -p mycad-cli -- view examples/boolean_cut_cylinder_hole.mycad
```

期待: 10×10×10 の box に r=2, h=6 の円柱の穴が開いている

- [ ] ① 丸穴が中央に開いた直方体が表示される
- [ ] ② 法線が正しい (穴の内面も適切な向き)
- [ ] ③ 退化三角形・抜けなし
- [ ] ④ 円柱穴の断面円が滑らかに閉じている
- 結果: ___

### 6. boolean_intersect_cyl_sphere — cylinder ∩ sphere

```bash
cargo run -p mycad-cli -- view examples/boolean_intersect_cyl_sphere.mycad
```

期待: r=3, h=20 (z=-10 origin) の円柱と r=5 の球の共通部分 → レンズ/球帯状の形状

- [ ] ① レンズ/球帯状の形状が表示される (円柱と球の重なり部分)
- [ ] ② 法線が正しい (曲面の表裏が正しい)
- [ ] ③ 退化三角形・抜けなし
- [ ] ④ 球面と円柱面の交線が滑らかに閉じている
- 結果: ___

---

## 曲面 Boolean (bonus 2 本)

### 7. boolean_fuse_box_cyl — box ∪ cylinder

```bash
cargo run -p mycad-cli -- view examples/boolean_fuse_box_cyl.mycad
```

期待: 10×10×10 の box と r=2, h=15 の円柱が合体した形状 (円柱が box を貫通)

- [ ] ① box から円柱が突き出た形状が表示される
- [ ] ② 法線が正しい
- [ ] ③ 退化三角形・抜けなし
- [ ] ④ box と円柱の交線が滑らかに閉じている
- 結果: ___

### 8. boolean_intersect_box_cyl — box ∩ cylinder

```bash
cargo run -p mycad-cli -- view examples/boolean_intersect_box_cyl.mycad
```

期待: 10×10×10 の box と r=2, h=15 の円柱の共通部分 → box 内に収まった円柱部分

- [ ] ① box 内に収まった円柱形状が表示される
- [ ] ② 法線が正しい
- [ ] ③ 退化三角形・抜けなし
- [ ] ④ 切断面の円が滑らかに閉じている
- 結果: ___

---

## スキップ (known issue)

### boolean_cut_sphere_dimple — box − sphere = 球面の窪み

**`mycad export` で `TrimmedFaceUnsupported` エラー。 Issue #50 で修正予定。**
本 example は #35 の必須 5 本に含まれないため、#35 close の条件外。

---

## 最終判定

- [ ] 必須 5 本 (1〜3, 5〜6) が全て ✅
- [ ] bonus 3 本 (4, 7〜8) が ✅ または許容できる軽微な問題
- [ ] 視覚問題があれば Issue 起票・解消済み

判定日: ___
確認者: ___
