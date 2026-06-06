# ADR-007 抜粋（#73 関連: §3 stdlib・参照解決機構）

## §1 transform を B-rep 本体に焼き込む（案A）— 背景
transform は幾何型に直接適用する方針。ただし #73 のスコープは「参照解決とツリー展開」であり、
transform の幾何適用は #74 に委譲する（#73 は transform を読まない）。

## §3 stdlib の物理形式と解決機構（#73 の主スコープ）
- `stdlib://X` → `<stdlib_root>/X.mycad` を読み込んでビルド。
- `stdlib_root` の決定順: env `MYCAD_STDLIB_PATH` → リポジトリ内 `stdlib/`（Cargo.toml 起点）。
- ファイル参照（`ComponentRef::File`）の解決機構を同じコードパスで実装し共通化する。
- 再帰ロード: `ComponentRef` を持つ Component は、参照先 `Document` をロードし
  その `root_component` を自身の子として展開してビルドする。
- **循環参照検出**: ロード中の Document パス集合を visit-set として持ち、同じパスが再度現れたら
  `KernelError::CircularReference` で即時エラー。
- **深さ上限**: デフォルト 16 段。超過時は `KernelError::MaxDepthExceeded` でエラー。

## §ASSEMBLY ペルソナ（Phase 5 標準レビュー観点）
① transform 合成順 ② 循環・深さガードの全パス有効性 ③ EntityID 決定性
④ pcurve・曲面パラメータの transform 後整合 ⑤ stdlib_root 未設定時の fallback 明記。
（#73 は transform 適用を含まないため ①④ は非該当。②③⑤ が主審査対象。）
