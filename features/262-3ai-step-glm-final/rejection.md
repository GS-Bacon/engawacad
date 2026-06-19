<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round Codex r4

- R01 (A-F01 / C-F01 / M-F01, 3 persona 一致, high): 「`isValidReviewYaml()` が正規表現ベースのため `issues: null` / 非配列 / 壊れた item を transport 成功扱いにする。root を実際に YAML parse すべき」を棄却 — Non-Goals に「other dispatch script (`dispatch-glm.ts` / `dispatch-codex.ts`) の同様改修（別 Issue で扱う）」を明記済みで、`dispatch-codex.ts` も同じ正規表現ベースの parseVerdict を使っており dispatch-glm-review.ts だけ yaml parser を採用すると 2 つの parser が project 内に共存する。yaml パッケージ追加には root `package.json` 新設が必要で本 Issue (#262 GLM final review vacuous pass 解消) のスコープから大きく逸脱。r2/r3 の 4 層検証 (structure / contract / quoted-string filter / fail-closed) で max-turns / claude-error / contract 違反は既に捕捉済み。フォローアップ: yaml-parser ベースの review verdict 検証は別 Issue として起票候補。
