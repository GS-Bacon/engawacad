# ADR-011: MyCAD → EngawaCAD への命名変更

**Date**: 2026-06-13
**Status**: Accepted
**Related**: ADR-002 (ロードマップ管理), ADR-006 (issue decomposition), ADR-001〜010 (歴史記録として "MyCAD" 表記をそのまま保持)

---

## Context

Phase 6 (対話編集の背骨) 完了 (2026-06-07) 後、Phase 7 (スケッチ描画) 着手を直前に控えたタイミングで、プロジェクト名 **MyCAD** の見直しが必要になった。

**問題**:

- 「MyCAD」は既存 CAD 製品 (複数) と命名衝突しており、検索性・ブランド識別性が低い
- Phase 7 から `web/` ビューアのフロントエンド資産・Phase 9 (STEP export) の対外文書が増えると、リネームコストが時間とともに増大する
- Phase 6 完了直後・Phase 7 未着手の現在は新規 commit がなく **rename diff のレビュー粒度が最も小さい**

**前提**:

- 実ユーザーは個人 (= 自分) のみ。`cargo install` ベースの公開バイナリ名互換は破壊しても実害ゼロ
- Phase 1〜6 までの ADR は決定時点のスナップショットとして既に確定済み
- リネーム作業は Issue #157 (`chore: rename MyCAD → EngawaCAD`) で実施する

---

## Decision 1: 新名称は **EngawaCAD**

### 決定

- プロジェクト名 (表記): **EngawaCAD**
- コード内 lowercase 識別子: `engawa`
- 由来: えんがわ (寿司ネタ、ヒラメ/カレイの縁側の身)
- 命名方針: Unagi (キックボードメーカー) の路線で、CAD ドメインから意図的に切り離した食べ物由来の命名

### 採用しなかった案

| 候補 | 棄却理由 |
|---|---|
| **MyCAD** (現名) | 既存 CAD 製品 (複数) と衝突、検索性が低い |
| **KegakiCAD** (罫書き) | CAD/製図ドメインに意味的に近すぎ、Unagi 方針 (ドメイン外名) と逆 |
| **Mochi** (餅) | 「ブロック状」連想がわずかに残る。Engawa より食べ物意味が前面で薄い |
| **Anko** (餡子) | 「中身が詰まっている」=ソリッド連想、意味付きすぎ |
| **Tofu** (豆腐) | 「ブロック=幾何プリミティブ」連想、意味付きすぎ |
| **Dango** (団子) | 「連結=構造」連想、わずかに意味付き |

### 理由

1. **ドメイン外名の明確性**: Unagi がキックボードメーカーであるように、CAD と無関係の食べ物名は「ブランドとして固有」「製品ジャンルを暗示しない」点で強い。検索衝突も起きにくい。
2. **EngawaCAD の語感**: 5 文字、発音しやすい、寿司ネタとして世界的に認知あり、意味的に CAD と全く接続しない。
3. **棄却候補との比較**: Mochi/Anko/Tofu/Dango はいずれも形状・構造の連想がわずかに残る。Engawa は完全にニュートラル。
4. **KegakiCAD は方針と逆**: 罫書き = 製図技術 で意味が乗りすぎる。Unagi 方針 (ドメイン外名) を採用した瞬間に棄却される。

---

## Decision 2: 派生識別子の置換ルール

### 決定

以下の対応表で機械置換する。ファイル種別を問わず適用 (ただし Decision 3 の不可触範囲を除く)。

| 旧表記 | 新表記 | 用途 |
|---|---|---|
| `MyCAD` | `EngawaCAD` | プロジェクト名 (ブランド表記、README, ROADMAP, 一般文章) |
| `MyCad` | `Engawa` | Rust 識別子の CamelCase (型名・モジュール名等、現状ほぼ未使用だが念のため) |
| `mycad` | `engawa` | crate prefix, バイナリ名, lowercase 識別子, ファイル拡張子, リポジトリ名 |
| `mycad_` | `engawa_` | Rust crate import (`use mycad_kernel` → `use engawa_kernel`) |
| `mycad-` | `engawa-` | crate ディレクトリ・パッケージ名 (`mycad-kernel` → `engawa-kernel`) |
| `.mycad` | `.engawa` | ファイル拡張子 |
| `github.com/GS-Bacon/mycad` | `github.com/GS-Bacon/engawacad` | リポジトリ URL |

### 採用しなかった案

- **A. 拡張子 `.mycad` を維持** (互換): サンプル 32 ファイルを触らずに済むが、ブランドと食い違って混乱の源。ソロ開発で実ユーザーは自分のみのため互換コストはゼロに近い。
- **B. 拡張子 `.kcad` / `.eng` 等の短縮**: `.kcad` は KiCad と衝突気味。`.eng` は意味不明。フル名 `.engawa` が固有性と明確性で最良。
- **C. crate prefix を捨てる** (`kernel`/`format` 直): ワークスペース内なら衝突しないが、crates.io publish 時に再考が必要になり手戻りリスク。prefix `engawa-*` 維持が保守的。

### 理由

1. **対応表の明示でツール置換を機械的に**: `sed` / `rg --replace` で表に従って置換すれば、人間の判断を介さず確実に切り替えられる。
2. **大文字小文字の独立処理**: 一括 `-i` 置換は危険 (意図しない箇所マッチ)。表で 5 種類に分けて個別に処理する。

---

## Decision 3: ADR-001〜010 本文 + `features/**/*.md` は **書き換えない**

### 決定

- `docs/decisions/001-*.md` 〜 `010-*.md` の本文中の "MyCAD" / "mycad" / "MyCad" 表記は **一切書き換えない**。
- `features/**` 配下の全ファイル (plan.md / test-spec.md / fix-plan.md / 設計レビュー記録 / final-review.yaml / glm-test-result.json / plan-snapshots/ 等) も同様に **書き換えない**。md / yaml / json を問わず決定時点のスナップショットとして ADR と同等の歴史記録扱いとする。
- 本 ADR-011 のみが「現時点で EngawaCAD」と書く。
- 機械置換時は `--glob '!docs/decisions/00[0-9]-*.md' --glob '!docs/decisions/010-*.md' --glob '!features/**'` で除外する。

### 採用しなかった案

- **A. ADR-001〜010 も機械置換**: 名前統一は綺麗だが、ADR は決定時点のコンテキストを保持する文書なので、後から名前を遡及書き換えると「いつ ADR を書いたか / その時の世界観」を歪める。
- **B. ADR-001〜010 に注記を追加** ("当時の呼称は MyCAD" 等): 11 ファイルに小さな注記を入れる手間に対し、ADR-011 の存在自体で文脈は十分復元可能なので不要。

### 理由

1. **ADR は決定時点のスナップショット**: ADR-002 で「フェーズ完了判定」が "MyCAD" 文脈で記述されている。それを EngawaCAD に書き換えると、ADR の "Status: Accepted" が示す確定の重みが揺らぐ。
2. **歴史的可読性**: 将来「ADR-002 を読むと MyCAD と書いてあるが今は EngawaCAD」という二段階の解釈は、ADR-011 の存在で 1 ステップ補完できる。むしろ歴史の連続性を示す情報になる。
3. **置換漏れリスクの大幅減**: 11 ファイルを除外することで、機械置換のレビューコストが減り、誤置換も起きにくい。

---

## Decision 4: ADR-011 を実装 Issue #157 と同梱する (ADR-006 §2 例外扱い)

### 決定

- 本 ADR-011 は実装 Issue #157 の **最初の commit** で同梱する (ADR-only Issue は別途立てない)。
- ADR-006 §2 (ADR-only Issue / impl Issue 分離原則) の例外として扱う。
- Issue #157 の本文に Codex intent-check 棄却記録と同梱根拠を明示している。

### 採用しなかった案

- **A. ADR-only Issue + impl Issue の 2 Issue 構成** (ADR-006 §2 文面通り): ADR-only Issue を起票 → ADR-011 を 1 commit で merge → close → impl Issue 起票、というフロー。
- **B. ADR-011 を本 Issue の最後に書く**: 実装後に「結局こう決めた」と書く方式。事後的で意思決定としての価値が薄い。

### 理由 (ADR-006 §2 例外扱いの根拠)

1. **リネーム ADR は ADR ≒ 実行計画書**: 通常 ADR は「設計判断 → 実装は別チーム/別タイミング」だが、本 ADR の本文 (Decision 2 の置換ルール表) は機械置換の手順書そのもの。決定と実装の時間軸が一致する。
2. **分離すると状態整合性が悪化**: ADR-011 を先に merge すると「ADR には `.engawa` と書いてあるのに実コードはまだ `.mycad`」というファイル状態の不整合期間が生じ、レビュアー・将来読者に混乱を与える。
3. **§2 の本来目的 (設計議論と実装の混在禁止) は満たされている**: リネームの意思決定 (名前選定・スコープ・拡張子) は Issue 起票前のユーザー壁打ちで既に確定済み。Issue 内で設計議論を再開する余地はない。
4. **ソロ開発の事務オーバーヘッド**: 承認者 = 実装者 = 自分のため、ADR-only Issue の開閉サイクルは事務作業コストのみ生む。

---

## Out of scope (本 ADR では扱わない)

以下は本 ADR では決定しない:

- **`crates/xtask/` の rename**: xtask は cargo の業界慣習名 (xtask = build automation crate)。プロジェクト名と独立なため据え置き。
- **`Cargo.lock` の手動編集**: `cargo update` で自動再生成。手動編集は別のリスクを生む。
- **ビューア UI のブランディング画面要素** (ロゴ・タイトル等): Phase 7 内で個別に対応する別 Issue として扱う。
- **公開バイナリ名 breaking change の互換シム**: 旧 `mycad` バイナリは消去。実ユーザーは個人 (= 自分) のみなので互換不要。
- **Phase 7 の機能 Issue** (正準平面スケッチ・Extrude/ExtrudeCut UI 等): Issue #157 close 後に新名で別途起票する。

---

## Affects

| ADR | 影響 |
|---|---|
| **ADR-001** (B-rep representation) | 本文不変。"MyCAD" 表記をそのまま残す (Decision 3) |
| **ADR-002** (ロードマップ管理) | 本文不変。ROADMAP 内の repo URL は実装 Issue #157 Step 5 で別途更新 |
| **ADR-003** (viewer/app architecture) | 本文不変 |
| **ADR-004** (freeform geometry) | 本文不変 |
| **ADR-005** (topological naming) | 本文不変 |
| **ADR-006** (issue decomposition) | 本文不変。§2 (ADR-only/impl 分離) は本 ADR で例外扱い (Decision 4) |
| **ADR-007** (assembly / references) | 本文不変 |
| **ADR-008** (interactive editing) | 本文不変 |
| **ADR-009** (boolean intersection curve) | 本文不変 |
| **ADR-010** (sketch input model) | 本文不変。Phase 7 機能 Issue は本 ADR-011 の後に新名で起票 |

---

## 検証

実装 Issue #157 の完了条件と一致:

1. `cargo xtask ci` 通過 (fmt + clippy + test + build)
2. `cargo run --bin engawa -- export examples/simple_box.engawa -o /tmp/test.stl` で従来同等の STL 出力
3. `rg -i 'mycad' . --glob '!Cargo.lock' --glob '!docs/decisions/00[0-9]-*.md' --glob '!docs/decisions/010-*.md' --glob '!features/**'` の結果が空 (= 意図せぬ残存ゼロ)
4. GitHub repo が `GS-Bacon/engawacad` にリネーム済み
5. 旧 URL `github.com/GS-Bacon/mycad` が新 URL にリダイレクトされる
