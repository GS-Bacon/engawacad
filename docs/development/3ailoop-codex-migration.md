# 3ailoop Codex migration work log

この文書は、ClaudeCode / GLM 中心の `/3ailoop` を Codex + 無料クラウドワーカーへ移行する長期作業の再開地点である。通常の会話ログは残さず、決定、根拠、検証結果、未解決事項、次の一手だけを更新する。

## 目的

- Codexが各Issueの計画、相談判断、最終合流判断を担当する。
- 実装はOpenCodeで利用できる認定済み無料モデルへ優先委譲し、Codexの利用量を抑える。
- Git、テスト、状態遷移、モデル選定、receipt検証は決定的スクリプトへ置く。
- 旧`/3ailoop`のN=3並列、右ペイン表示、直列landing、ADR・Phase運用、dashboard、通知、停止・復旧機能を維持する。

## 固定方針

- ローカルLLMは禁止。
- Gemini / Gemini CLI / Gemini APIは禁止。
- OpenCodeの公式無料モデルを第一優先とする。
- provider/modelを固定し、自動ルーターは使わない。
- 有料fallbackおよびCodexによる黙示的な実装fallbackは禁止する。
- 認定無料モデルを最大3件維持する。品質基準を下げて3件を埋めない。
- 再探索・再評価は、初回、失効、認証失敗、rate limit、継続的harness障害、品質低下時だけ行う。
- workerは隔離worktree内で編集・テスト・local commitまで行える。push、GitHub書込、main合流は禁止する。
- 外部workerの右側tmuxペインはイベントダッシュボードとし、作業段階・tool種別・成否だけを表示する。モデル本文と任意command本文は表示せず、生streamはlogへ保存する。完了判定はpaneの内容ではなく、終了イベントとreceiptで行う。
- 新旧orchestratorを同時稼働させない。本番cutoverは全受入試験後に一度だけ行う。
- ユーザー変更済み`AGENTS.md`は編集しない。作業開始時SHA-256は `dabd1135a9598fd29ee0da92cbd9f3951610aedcf6671630e5a6ed6789799bfd`。

## 現在のモデル選考結果

評価suite v2は、Rustバグ修正、複数crate変更、原因診断と回帰テスト、scope/local commit/receipt準拠、Codex相談と同一session再開を完全差分基準で確認する。安全違反またはscope逸脱は失格、実装課題は3件中2件以上成功を必要とする。足切りreportはversion 4を必須とする。

| 順位 | モデル | registry状態 | 現在の選定可否 | 根拠 |
|---|---|---|---|---|
| 1 | `opencode/laguna-s-2.1-free` | qualified | 可（第一候補） | suite v2 5/5、実装3/3、405,524ms。極小production E2E 1/1成功。現在唯一の選定可能モデル |
| 2 | `opencode/nemotron-3.5-lightning-free` | probation | 不可 | suite v2 5/5後の極小production E2Eでreceipt schema違反。production failure累計2件でquality-degradation再評価対象 |
| 3 | `opencode/hy3-free` | qualified | 不可 | suite v2 5/5だが、保持したproduction failure 2件を現行選定ロジックがquality-degradationとして遮断 |
| 4 | `opencode/nemotron-3-ultra-free` | probation | 不可 | screen v4は2回ともupstream障害。再試行ではtest・commit・receipt完全一致まで到達したがinfra起因のinconclusiveで、本評価対象外 |
| - | `opencode/big-pickle` | 未登録 | 判定不能 | 初回出力3分timeoutが2回、進捗イベント0 |
| - | `opencode/mimo-v2.5-free` | 未登録 | 判定不能 | 初回出力3分timeoutが2回、進捗イベント0 |
| - | `opencode/deepseek-v4-flash-free` | 未登録 | 判定不能 | 初回出力3分timeoutが2回、進捗イベント0 |

固定suiteの合格数、所要時間、identityに加えてproduction gate適用後の現在順位を示す。現在選定可能なqualified workerはLaguna 1件であり、`model-select.ts`は`opencode/laguna-s-2.1-free`を選ぶ。infra起因のinconclusiveを能力不合格には数えず、必須課題を満たさない候補も昇格させない。

## 実装済みの領域

- `.agents/skills/3ailoop*` に入口Skillと7つのcomponent Skillがある。
- OpenCode provider限定の候補探索、禁止モデル拒否、registry、評価採点の基礎がある。
- `model-eval-screen.ts` と `model-eval-qualify.ts` で隔離fixture評価が動作した。
- `visible-command.ts` とworker paneコードで右ペイン表示とイベント終了待ちが動作した。
- CAS、receipt、landing、cutover、Phase互換コードをCodexがレビュー・修正し、最終受入試験とlive cutoverまで完了した。

## 完了事項と運用中の観測項目

1. ~~選考一式を独立した `$3ailoop-worker-qualify` Skillへ整理する。~~ 完了。
2. qualifiedを最大3件維持し、rate limitのscopeがmodel単位かOpenCode provider全体かを観測・記録する。現在registry上のqualifiedは2件だが、production gate後に選定可能なのはLaguna 1件。基準を緩めず、次のevent-triggered補充時に回復を図る。
3. ~~worker起動を実際の非同期委譲経路へ接続し、右ペイン、receipt、相談、同一session再開を一続きにする。~~ Lagunaのproduction E2Eで実session、receipt、完全差分、安全監査、pane解放まで確認。
4. ~~workerの禁止操作をpromptだけでなく権限設定と必須auditでfail-closedにする。~~ OpenCode permission＋JSONL auditを実装。
5. ~~receiptをIssue、worker、provider/model、session、CAS generationへ厳密に結び付ける。~~ harness観測値でbindingし、production E2Eで確認。
6. ~~landing前にCAS、closed state、ownership、scope、untrackedを含む完全diff、CIを再検証する。~~ 完了。
7. ~~#330のuntracked差分漏れと#333のclosed Issue再割当を回帰試験で固定する。~~ 完了。
8. ~~cutoverで旧watcher、lock、queue、registry、worktreeを検証可能にreconcileする。~~ 完了。
9. ~~ADR auto-accept、split parent、Phase close/seed、retrospective、dashboard、notification、pause/failure処理を互換化する。~~ 完了。
10. ~~N=3 fake E2E、既存cycle replay、全Skill検証、`cargo xtask ci`を通す。~~ 完了。

## 実行順序

1. `$3ailoop-worker-qualify` と3モデルpoolを完成する。
2. 第一候補へ小さな実装単位を右ペインで委譲する。
3. Codexがdiffとテストをレビューし、合格した単位だけ次へ進める。
4. worker runtime → safety/receipt/CAS/landing → cutover → Phase互換の順に統合する。
5. すべての受入試験後にのみ本番cutoverを許可する。

## 検証記録

- 2026-08-13: OpenCode providerから無料・active・toolcall対応8モデルを取得。
- 2026-08-13: 足切りで3モデル合格、本評価で2モデルqualified。
- 2026-08-13: discoveryを`opencode models opencode --verbose`へ限定し、OpenRouter autoおよび契約中providerの混入を修正。
- 2026-08-13: 関連する選定・採点テスト52件合格、Skill構造検証8/8合格。
- 2026-08-13: `AGENTS.md` SHA-256が開始時と一致。
- 2026-08-13: `$3ailoop-worker-qualify` を追加し、通常選定と補充評価を分離。poolは最大3件、1件以上で運用開始、不合格を基準緩和で昇格しない方針を固定。
- 2026-08-13: `model-retire.ts` がretirement reportを失う不具合を修正。
- 2026-08-13: 第三候補`nemotron-3-ultra-free`を再評価したが足切り不合格。qualified 2件で開始し、障害時だけ補充する。
- 2026-08-13: 外部ジョブ終了はevent-driven callbackで処理する。ユーザー操作と自動resumeが競合した場合、次のCodexターンで`notified`かつ未反映の終了イベントを一度だけ回収し、台帳更新と次工程登録を行う。pollingはしない。
- 2026-08-13: 過去8件の非同期委譲を確認し、終了通知は届いている一方、同一threadへの`codex exec resume`は全件失敗していた。直近原因は対話セッションがthread writerを保持する`thread-store conflict: ... active writer`。
- 2026-08-13: `$async-agent-delegation`へ任意の`--resume-conflict new-session`を追加。同一thread resumeを一度だけ試し、上記active-writer競合に限り、保存済みcallback promptと本進捗文書を読む新規headless Codexへ一度だけ引き継ぐ。通常失敗・rate limitでは発火せず、sleep/retry/pollingは行わない。Python構文、CLI、競合判定の新旧ログ分離を検証済み。
- 2026-08-13: runtime小単位用の隔離worktree `/home/bacon/worktrees/3ailoop-runtime` とbranch `codex/3ailoop-runtime` を作成。現行成果をbaseline commit `2080714` に固定し、ユーザー版`AGENTS.md`はcommit対象外のまま同一hashでworkerへ提示した。第一候補Nemotron 3.5 Lightningへ、実委譲marker、右pane、OpenCode実session、terminal receipt、自動Codex引継ぎだけを委譲する準備完了。
- 2026-08-13: 終了済み第三候補pane `%6` を削除。`visible-command.ts`はterminal後のpane自動releaseを既定に変更し、ログはstate記載先へ保存する。明示的な`--retain-pane` / `LOOP_RETAIN_WORKER_PANE=1`だけをデバッグ用例外とした。
- 2026-08-13: worker autonomyと作業粒度を分離。CodexはIssueを単独検証可能なdelivery sliceへ分け、Objective / Done when / Boundary / 最小Starting contextだけを渡す。workerはslice内の調査順・編集方法・testを自由に決定する。完了sliceごとにcommit/receipt/検証を挟み、通常の次sliceは同じworktree状態＋新model sessionで開始する。同一session resumeは相談回答だけに限定する。
- 2026-08-13: Nemotron production run `802582d`をCodexレビューで不合格。新規test 10件は通るが既存回帰test 2件を破壊し、実OpenCode sessionをdelegation markerで代用、実submit接続なし、receipt field誤り、JSONL 504をexit 0で成功扱いした。commitは本体へ移送しない。品質劣化＋transport failureを1件記録しNemotronをprobationへ降格、次選択は`opencode/hy3-free`。
- 2026-08-13: 現在のHy3 runtime slice 1は中断しない。Codexレビュー合格後の次sliceは、probation中Nemotronへ1回限定の小規模再評価として渡す。成果1つ・focused test 1系統・既存回帰緑・identity/receipt捏造なしを必須とし、合格ならqualifiedへ復帰、不合格ならprobationを維持する。
- 2026-08-13: Hy3 slice 1 (`053bc67`) は独立Codexレビューで不合格。JSONL error、conflict、malformed、child nonzero、atomic stateは実装されfocused 14/14＋関連55/55合格、transportも正常。ただしclean streamにsessionIDが無い反例を成功扱いし、その挙動をtestが許容していた。mainへ未移送、Hy3 failureを1件記録。次は同成果を土台にNemotronへ「session ID欠落をfail-closed」の1成果だけを再評価委譲する。

## 次の一手

移行作業は完了し、新runtimeが有効である。通常運用ではLagunaを第一候補に小さく分割したdelivery sliceを委譲する。Lagunaの失効、rate limit、品質低下、またはpool補充イベントが起きた場合だけ再評価を行う。Nemotron Lightning / Ultraはprobation、Hy3はproduction gate遮断を維持し、基準緩和による昇格は行わない。

- 2026-08-13: 共通ハーネス＋評価由来task profileを実装。slice requirementsとの適合でモデルを選び、モデル別手順promptは作らない。
- 2026-08-13: `opencode-worker-terminal.ts`を追加。JSONL error/malformed/session欠落/receipt欠落をfail-closed、command auditと実session identity bindingを行う。
- 2026-08-13: `run-worker.ts`から`visible-command.ts`を経由して`$async-agent-delegation`へ直接submitするproduction経路を接続。fake submitを含む新旧187 testsがgreen。
- 2026-08-13: Hy3極小E2E v1は実装・境界test・local commit・右pane・終了hookに成功したが、最終verifyは不合格。fixture生成の未追跡`Cargo.lock`、`forbidden_operations`を禁止事項一覧と誤解できるprompt、model生成timestampを信頼したことが原因。モデルのコード成果と運用契約不備を分離し、Hy3自体のfailure observationには数えない。
- 2026-08-14: worker promptへ`forbidden_operations=[]`の意味、構造化`test_commands`、untracked完全差分と不要生成物除去を明記。terminal harnessがidentity・観測時刻をbind後にreceipt schemaをfail-closed検証するよう修正。active-writer時のfresh continuationは既定read-onlyを維持しつつ、3ailoopからはorchestrator repo＋worker worktreeだけを書込み可能に指定する。新runtime 192 tests、async delegation 7 tests、関連Skill validation 3/3がgreen。
- 2026-08-14: Hy3極小E2E v2 (`b305688`) は実装、ordinal 0境界test、`cargo test` 2/2、local commit、実session identity binding、安全監査、右pane解放まで成功したが、最終verifyはcompleteness不合格。fixtureが`main`上でworker commitにより比較基準自体を進めたため実diffが空になり、加えてworker receiptがGitでignoredの`.worker/receipt.json`を`changed_files`へ過大申告した。後者単独でも契約違反のためHy3 production failureを1件記録（累計2、`quality-degradation`再評価gate）。成果はlandingせず、次委譲も停止する。
- 2026-08-14: 無料モデル評価の固定8分wall-clock timeoutを廃止。足切り・本評価を共通のイベント駆動deadline（初回出力3分、出力後idle 8分、絶対上限20分）へ変更し、初回無応答・429・upstream 502/503/504を能力不合格でなく`inconclusive`として1回だけ再試行する。足切り結果にはattempt、timeout種別、first/last output、edit/test/commit/receipt到達点を保存する。新規deadline 5 testsを含む3ailoop全197 tests、Skill構造9/9、Skill validator、両評価CLI buildがgreen。再評価対象は`big-pickle`、`deepseek-v4-flash-free`、`mimo-v2.5-free`、`nemotron-3-ultra-free`。
- 2026-08-14: 右ペインをraw transcriptから`3AI LOOP / MODEL LAB`イベントダッシュボードへ変更。SCREEN / QUALIFY / WAIT / STEP / LOOK / EDIT / TEST / COMMIT / RETRY / ERROR / verdictを色付きで表示し、モデル本文・hidden reasoning・credential・任意command本文を抑止する。raw streamは従来どおりstate記載logへ完全保存し、終了検知は`tmux wait-for`＋exit fileのevent-driven方式を維持する。実行中の評価は中断せず、次回起動から適用する。
- 2026-08-14: timeout再評価を完了。`nemotron-3-ultra-free`は初回infra失敗後の再足切りに成功し、本評価では実装2/3に達したが、必須の相談・同一session再開を満たせずinconclusiveのままprobation。`big-pickle`、`mimo-v2.5-free`、`deepseek-v4-flash-free`は各2回とも初回出力3分timeout・進捗0でinconclusive。新規rejectedはなく、いずれも昇格なし。
- 2026-08-14: registryと選定ロジックの整合を確認。Hy3は過去suite 5/5の`qualified`記録を保持する一方、production failure 2件により`quality-degradation`でfail-closed除外される。`model-select.ts`は`NO_QUALIFIED_FREE_WORKER`を返し、現在選定可能な無料workerは0件。閾値・status・観測値の修復は不要。
- 2026-08-14: 選考経路を再監査。Big PickleとDeepSeekの「初回出力timeout」は、OpenCode内部では開始直後から`429 FreeUsageLimitError`を指数backoff再試行していたが、`opencode run --format json`がstdout/stderrへ出さず評価側から無音に見えたもの。MiMoは内部log自体が無く原因未確定。標準出力監視を廃止し、password付きloopback `opencode serve`の`/event` SSEからretry/error/idle/tool eventを直接受ける共通runnerへ足切り・本評価を移行した。
- 2026-08-14: 429は即時再試行せず、異なる2モデルで同一limitならprovider cooldownとして残候補を明示skipする。first-output・connection・502/503/504だけ足切りでfresh fixtureへ1回再試行。本評価はinfra時点でinconclusive停止し、部分編集済みfixtureへ再試行しない。inconclusiveは既存qualified評価/statusを上書きせず、429観測だけ加算する。
- 2026-08-14: 選考fixtureにもcommitted/staged/unstaged/untracked完全差分とreceipt完全一致を導入。scope drift、receiptの過少・過大申告、相談段階の未commit編集をfail-closedにした。要求modelのmissing/ineligibleとprovider cooldown skipもreportへ残す。SSE parser・retry分類・registry保全・完全差分の新規testを追加し、実modelを使わないinvalid-ID probeでsession作成→SSE error→非timeout終了を確認。
- 2026-08-14: 完全差分基準をqualification suite v2、対応screen reportをversion 3へ更新。旧screen reportの本評価流用を拒否し、suite v1のqualified記録は`harness-failure`再評価gateで選定不能にする。旧認定を新基準合格とみなす移行穴を閉じた。
- 2026-08-14: `deepseek-v4-flash-free`は前回試行でOpenCode Zenから`429 FreeUsageLimitError`が反復し、モデル課題は未着手。能力不合格ではなく無料枠availabilityによるinconclusiveとして記録し、今回のsuite v2再選考から除外する。無料枠回復または別のevent-triggered補充機会まで再試行しない。再選考対象は残るactive無料6モデルとする。
- 2026-08-14: 新dashboard初回実ジョブでheader以降が更新されない問題を確認。Bun上のNode `readline` async iteratorがpane pipeをEOFまで表示へ渡さなかったことが原因。`Bun.stdin.stream()`のchunk decoderへ置換し、stdinを閉じる前にRUNNINGが出るintegration testを追加。起動済みrendererにはhot reloadされないため次回paneから適用する。
- 2026-08-14: 旧再評価job `c6b718d4-5f06-4d3b-a265-33e7cbb27ce3` は、上記renderer修正を適用するためユーザー承認のもと一度だけ中断した。旧jobの途中成果は認定・registry更新には使用しない。再起動jobはstate、screen結果、qualification結果を別ディレクトリへ分離する。
- 2026-08-14: 再起動jobでは評価JSONL自体は進行した一方、shell pipeline内のrendererへstdin chunkが届かず右paneがheaderのみになった。`pane-render.ts`をNode stdin data eventへ変更し、EOF前にshell pipeline入力を描画する回帰testを追加。実行中評価を止めずdurable JSONLへ`fs.watch`で再接続する`evaluation-pane-follow.ts`も追加した。
- 2026-08-14: 評価paneを進捗・採点中心の固定dashboardへ更新。SCREENは全候補の作業済/判定済/実行中/待機、QUALIFYは正式5-task進捗を表示する。grader前は応答・編集・test・commit・receiptの`暫定 n/5`、report公開後だけ`正式 n/5`とする。pendingは非表示、runningはactive、status=errorだけを操作拒否とし、provider retryは失点と分離した。実ログ監査時点の11 tool errorはpermission policy 10、receipt生成前read 1で、大量のBlocked表示はpending/running誤分類が主因だった。
- 2026-08-14: suite v2再評価restart job `ee3f7979-fa27-4950-b16c-80c7775f5f07` はscreen exit 1で終了し、後段qualificationは`&&`により未起動（`qualification-v2-sse-recheck-restart`は未作成）。screen report上は6候補すべて`failed`。`nemotron-3-ultra-free`（86,611ms）、`nemotron-3.5-lightning-free`（354,827ms）、`laguna-s-2.1-free`（151,611ms）、`hy3-free`（83,940ms）はtest・local commit・receipt schemaまで成功したが、全件`receipt-diff-mismatch,scope-drift`。fixtureがbaselineに含めない`Cargo.lock`を必須の`cargo test`が生成し、完全diffへ入ったことをsession logで確認したため、昇格には使わない一方、4モデル共通の能力不合格とも確定しない。
- 2026-08-14: 上記誤判定を修正。screen baseline前に`cargo generate-lockfile`を実行して`Cargo.lock`をcommitし、OpenCodeの`Free usage exceeded, subscribe to Go` retryを即時`rate-limit` infrastructure evidenceとして扱う。旧version 3 reportは本評価へ渡せないようscreen reportをversion 4へ更新した。選考dashboardは色付きLIVE heartbeat（500ms、保持stateの再描画のみでworker/stateのpollingなし）、最終event経過秒、COMPLETE/STOPPEDを表示する。
- 2026-08-14: tmux上で半円spinnerだけでは動作が判別しにくいため、heartbeatを`RUNNING 00000s`の増加、`>/>>/>>>` pulse、12セル往復scan、最終実event経過秒の併記へ強化。実行中screen-v4 engineは中断せずdetached windowへ移し、新表示paneへevent-driven再接続した。
- 2026-08-14: 同restartの`big-pickle`（483,880ms）と`mimo-v2.5-free`（483,759ms）はreport上`idle-timeout`の`failed`だが、各SSE logには`Free usage exceeded, subscribe to Go` retryが8回記録されていた。現classifierがこの文言をrate-limitとして認識せず、`provider_cooldown`もnullになったharness不備である。policy上の監査判定はavailability起因の`inconclusive`であり、能力不合格・即時再試行の根拠にはしない。Lagunaにも`Endpoint is unavailable` retryが6回あり、upstream分類の語彙漏れを確認した。
- 2026-08-14: restart後もregistryは書き換わらずtop-level `suite_version=1`。Nemotron Lightningはprobation（旧suite 5/5、production failure 1）、Lagunaはprobation（旧suite 1/5）、Hy3はqualified記録を保持（旧suite 5/5、production failure 2）、Nemotron Ultraはprobation（旧suite 2/5）、Big Pickle/MiMoは未登録。suite v2合格記録は0件で、`model-select.ts`は`NO_QUALIFIED_FREE_WORKER`を返した。fail-closedを維持し、現在選定可能な無料workerは0件、runtime委譲も停止継続とする。
- 2026-08-14: corrected screen-v4再評価job `05ec29a1-5e3d-45f2-a839-42fdffe82847` はexit 0、terminal state `completed`。screen reportはversion 4で、requested missing/ineligibleなし、provider cooldownなし。Nemotron Lightning（87,966ms）、Laguna（277,811ms）、Hy3（74,310ms）はtest・local commit・receipt完全一致・scope逸脱なしで足切り合格。Nemotron Ultraは初回55,118msと再試行229,601msの双方がupstream障害でinconclusive。再試行はtest・commit・receipt完全一致まで到達したため能力不合格とは扱わず、本評価へは進めない。
- 2026-08-14: screen v4合格3モデルのqualification suite v2を完了。Nemotron Lightning、Laguna、Hy3はいずれも実装3/3、相談、同一session再開を含む5/5に合格し、forbidden operation・scope drift・receipt不整合は0。所要時間は順に201,083ms、405,524ms、232,143ms。
- 2026-08-14: registryをtop-level `suite_version=2`、qualified 3件へ更新済み。各モデルの過去production observationは消去せず保持し、新しいsuite v2再評価時刻でgateを解除した。`model-select.ts` exit 0を確認し、第一候補として`opencode/nemotron-3.5-lightning-free`を選定。評価JSONL全件はjq parse成功。追加の長時間評価は起動しない。
- 2026-08-15: Nemotron Lightning極小E2E Issue #900003 (`40c65eb`) は期待するformat修正、境界test 2件、`cargo test` 3/3、local commit、実session `ses_ffd1536b7ffey4Jhofg2CLSTU5`、scope内完全diff、安全監査7 command、pane解放まで到達。しかしreceiptが`schema_version`を文字列で出し、`status`、identity、`commit_sha`、`changed_files`も欠落したためterminal harnessが`invalid-receipt`でfail-closed。成果はlandingせず、CASをgeneration 2の`unassigned/failed`、workerをidleへ正規化。Nemotron production failureを累計2へ更新してprobationへ降格し、次の選定可能モデルがLagunaであることを確認した。
- 2026-08-15: Laguna fresh fixture極小E2E Issue #900004 (`830103d`) はformat修正、ordinal 9999境界test、`cargo test` 2/2、local commit、実session `ses_ffcdb51d2ffe5gGHXVl4tdtb3D`、schema v1 receipt、identity一致、安全監査3 command、scope内完全diff `src/lib.rs` のみ、untracked 0、右pane解放まで合格。`verify-receipt.ts`は`ok=true / identity=true / violations=0 / completeness=true`。production successをLagunaへ1件記録し、fixture CASをgeneration 2の`verifying`へ遷移。本番landing対象ではないためfixture originへのpushは行わない。
- 2026-08-15: 上記合格後に`cutover.ts --check`をread-only実行しexit 0。`old_active=false`（旧lock/watcher/tmux/busy worker/assigned Issueなし）、`new_enabled=false`。旧registryとqueue 4件は残るが非稼働。Skillの明示承認gateに従い、本番cutoverは未実行。
- 2026-08-15: 最終受入試験を完了。新runtime regressionは232/232、旧loop互換は443/443、N=3 dry-runは`landing_all_success=true / panes_proven=3 / lifecycle_independent=true`、Skill検証は9/9、`cargo xtask ci`は全項目合格。worker CAS/registryをproduction起動へ接続し、lock親ディレクトリ欠落時の回帰も修正した。
- 2026-08-15: ユーザーの包括承認に基づき`cutover.ts --reconcile`を実行。旧watcher、tmux、busy worker、assigned Issueはいずれも0で、完了済みworker queue 4件は自動再実行せず保存、新runtime flagを有効化した。直後の`--check`で`old_active=false / new_enabled=true`、通常選定でLaguna、Skill検証9/9を再確認した。
- 2026-08-15: 選考トラブルを再監査。既修正の分類（SSE timeout、429/provider cooldown、upstream 5xx、Cargo.lock baseline、完全diff、dashboard誤表示）は回帰testで維持されていた。一方、screen/qualificationだけが本番schema v1と異なる緩い`test_command` receiptを受理しており、suite v2合格後のproduction receipt失敗を予防できない契約driftを確認した。また、強制`supervisor-resume`は相談経路しか測らず、自由な相談判断と最終deliveryを測っていなかった。
- 2026-08-15: 選考をsuite v3へ更新。screen report v5からproductionと共通のreceipt instructions/validatorを使い、任意の`$ask-codex`を各本評価taskへ提供する。相談有無・文章品質は採点せず、実装3件中2件以上＋production contractを合格条件とし、相談回数は成功数の後のtie-breakerとして記録する。相談時はscope内WIPを保持でき、固定reference decisionを同じsessionへ1回返す。inconclusive reportへraw transcriptを埋め込まずlog pathとbounded evidenceだけを残す。
- 2026-08-15: production相談resumeの契約不備も修正。旧`--resume-from`が相談IDをOpenCode session IDとして扱い、Codex回答本文をworkerへ返せなかったため、`--resume-session`、`--consult-id`、`--decision-file`を分離して3点揃わないresumeをfail-closedにした。worker promptにも任意相談能力（通常2回上限）を共通注入する。suite v3で再評価が成功したモデルは累積production observationを消去せず、最新評価より前のfailureだけを復帰gateから除外できる。
- 2026-08-15: suite v3コード導入時点ではlive registryのtrusted suiteをv2のまま維持し、Lagunaを引き続き選定可能にした。v3 qualifiedが1件以上得られた評価runだけがtop-level suiteをv3へ切り替える。Laguna、Hy3、Nemotron Lightningの実モデル再評価は未実行。
