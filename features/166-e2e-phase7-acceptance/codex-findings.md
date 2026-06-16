# Codex 非 blocking findings — #166

## R2 high (受容判断あり)

### F01 (high) — 実 API モック化が残る

- 指摘: Issue body は「実 API サーバを起動し、モックなしで動作確認 (#125 流儀)」を求めるが、本実装は `/api/v0/mesh` と POST `/api/v0/features` を route mock している。
- **状況**:
  - 実 API + `simple_box.engawa` の常設 mesh は RefPlane (20×20、原点中心) と完全重なる (Box 20×20×30、原点中心)。raycaster は形状面ピックを優先するため、Box が存在する状態では RefPlane (特に Right=YZ 平面 x=0、Box 中央を通る) を選択不能。
  - 各 test を実 API 経由で順次走らせると、POST `/api/v0/features` がサーバ side で永続化され、次の test で `duplicate feature_id "sketch_0"` 衝突が発生する (kernel 側に test 間 reset API なし)。
- **妥協**: `/api/v0/mesh` を空配列 mock、POST を mock で受けて non-degenerate mesh を返すことで RefPlane 選択 + 既知 ID 衝突回避を実現。POST capture (`page.on("request")`) で create_sketch (plane_ref) → extrude/extrude_cut の **POST 順序・payload (plane_ref、sketch ref、depth、fuse_target/target)** を実 UI 経路から検証する。
- **未カバー部分**:
  - 実サーバ side での `create_sketch.plane_ref` → SketchPlane 解決
  - 実サーバ side での `extrude_cut.target` 検証 (= 存在しない feature_id でのエラー)
  - 実サーバが返す mesh の B-rep 健全性
- **後続対応**: engawa-api に test fixture リセット API (例 `DELETE /api/v0/document/state`) を追加してから、本 spec を真の実 API E2E に書き換える (= 別 Issue)。

### F02 (high) — countBodies/mesh 健全性の検証不足 (修正済)

- 指摘: countBodies が __meshData.length のみで空 mesh 1 body も pass する。
- **修正**: E系の現実装は `countBodies` を使わず、POST capture (`posts.length >= 2`) と payload assert (`plane_ref`/`sketch`/`depth`/`fuse_target`) で「Extrude 経路が実際に発火し、UI から正しいペイロードが POST される」ことを検証する。non-degenerate mesh (positions/indices > 0) を mock response で返すため、`handle.updateBodies` 経路の整合性も間接的に通る。
- **残存**: 「実 API サーバが返す本物の mesh の B-rep 健全性」は F01 と同根 (実 API state 累積問題により実 API ベース化が困難) で、本 Issue では検証不能。
