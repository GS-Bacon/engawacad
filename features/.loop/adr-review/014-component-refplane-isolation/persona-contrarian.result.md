- High: `採用理由 2/5` は、`coordinate frame` の独立性と `ref_plane` 名解決スコープを混同しています。child が自前の transform / local sketch 座標を持つことと、親の datum を参照できることは両立可能です。Option B はその両立可能性を切り捨て、assembly 全体で共有したい datum を child ごとに複製させる設計負債を導入しています。
- High: `採用理由 1` の parser-level 不変は根拠として弱いです。root の canonical 自動補完は「親を持たない document root を自己完結させる」ための処理であり、親を持つ child の意味論を拘束しません。root と child が非対称なのは自然で、「非対称をなくしたい」だけで Option B を選ぶのは論理が飛んでいます。
- Medium: `採用理由 4` は循環参照です。現行実装と T15/T16 が Option B を enforce している事実は、「すでにそう実装した」ことの証拠であって、「その意味論が正しい」ことの証拠ではありません。ADR が既存コードを追認するだけなら、r2/r4 の設計論争を解決したことになりません。
- Medium: Option A の唯一の具体的欠点として挙げている「親 custom-only だと child の `Front` が壊れる」は、継承モデル自体の欠陥ではなく canonical three を effective scope に常在させていない表現上の問題です。これは Option C、あるいは A を少し正規化した派生案で解消可能で、B のように親 datum 参照そのものを禁止する必要はありません。

Option B は「実装済みだから正しい」「root で canonical 補完しているから child も独立であるべき」という弱い根拠に依存しており、assembly モデルで最も自然な要件である「親で定義した共有 datum を複数 child から参照する」能力を過小評価しています。child の local coordinate frame を保つことと、親 datum の参照可能性を断つことは別問題であり、前者は Option A/C でも維持できます。しかも B の安全性根拠である「親 custom-only で `Front` が壊れる」は canonical を effective scope に含める設計へ補正すれば済む話で、継承自体を否定する理由にはなっていません。将来の `face_ref` へ逃がす説明も、feature 作成前の datum 共有という要求を代替しておらず投機的です。ADR としては Option B が実装都合に引っ張られすぎており、少なくとも Option A か Option C を本命比較として再評価すべきです。

verdict: refuted