issues:
  - id: R01
    severity: medium
    section: "実装対象 > examples/two_bodies.mycad / テスト計画 > T12 / 検証手順"
    finding: "`two_bodies.mycad` を『XY 位置をずらした 2 つの extrude』に更新すると書いている一方で、T12 とブラウザ確認は引き続き `feature_id == [\"box_1\", \"cyl_1\"]`・`box + cylinder` を前提にしており、受け入れ条件が相互に矛盾している。これだと example の実体に応じて API テストか手動確認のどちらかが必ずずれ、実装者が何を正とすべきか判断できない。"
    suggestion: "`two_bodies.mycad` の中身を 1 つに確定し、その実ファイルに合わせて T12 の期待 `feature_id` と手動確認文言を揃えること。非重なり 2 体を維持するなら、`box + cylinder` 記述はやめて extrude 実体に一致する説明へ更新すること。"
  - id: R02
    severity: medium
    section: "複数ボディ出力 > cli export"
    finding: "CLI export の設計は `doc.root_component.features` をそのまま build するだけで、API 側で維持すると明記している `root.reference` / `root.children` の拒否が入っていない。これだと scope 外の assembly/reference 文書、特に root に features と children が混在する文書で、CLI が子ボディを黙って無視した部分 STL を出力し得る。"
    suggestion: "`run_export` でも API と同じ単一 component 前提チェックを先に行い、assembly/reference 文書は dedicated error で fail closed にすること。合わせて CLI 側にもその拒否を固定するテストを追加すること。"

verdict: pass