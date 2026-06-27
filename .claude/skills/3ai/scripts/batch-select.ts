#!/usr/bin/env bun
// batch-select.ts — /3ai バッチモード対象 Issue の選択・グルーピング・実行プラン生成
//
// 使い方:
//   bun batch-select.ts [--batch fixes|phase] [--dry-run] [--roadmap ROADMAP.md]
//
// 出力:
//   --dry-run: stdout のみ（ディスク書き込みなし）
//   通常時: stdout + features/.batch/plan.json

import { readFileSync, mkdirSync, writeFileSync, existsSync } from "fs";
import type { Deliverable, BatchPlan, BatchGroup, BatchIssue } from "./types.ts";

// --- 定数 ---

/** グループ間の実行優先度 (小さい順に処理) */
const GROUP_ORDER: Record<string, number> = {
  "batch:kernel": 0,
  "batch:data":   1,
  "batch:viewer": 2,
  "batch:skill":  3,
  "batch:phase":  4, // type:feature Phase issue の synthetic fallback
};

const AMBIGUITY_MARKERS = /要検討|TBD|どちらか/;

/** 3ailoop モードで selected から除外するラベル群 (gate:* prefix は別判定) */
const LOOP_EXCLUDE_LABELS = new Set([
  "gate:human-feel",
  "gate:adr-review",
  "needs-triage",
  "needs-phase",
  "needs-human",
  "needs-intent-review",
  "needs-review",
  "blocked-by-split",
  // #283: ADR retire の伝播で子 Issue に付く。親 ADR が needs-human から
  // 戻されると clearRetiredIfHumanReleased がこれを外して再 actionable 化する。
  "blocked-by-adr-retired",
]);

function isLoopExcludedLabels(labels: string[]): boolean {
  for (const l of labels) {
    if (l.startsWith("gate:")) return true;
    if (LOOP_EXCLUDE_LABELS.has(l)) return true;
  }
  return false;
}

function isParentBlockedBySplitChild(labels: string[]): boolean {
  return labels.some(l => /^parent-blocked-by-split:\d+$/.test(l));
}

/** parent-adr:N ラベルから親 ADR Issue 番号を抽出 */
export function extractParentAdrNumbers(labels: string[]): number[] {
  const nums: number[] = [];
  for (const l of labels) {
    const m = l.match(/^parent-adr:(\d+)$/);
    if (m) nums.push(parseInt(m[1], 10));
  }
  return nums;
}

/**
 * 親 ADR の active 状態を判定するために inactive とみなすラベル群。
 * 子 Issue がこれら状態の親を持つ場合、ADR の方針が未確定なため
 * split-batch tier / その他 tier から除外する (歪み #1 修正)。
 *
 * gate:* prefix も inactive (gate:adr-review = ADR レビュー未通過) として扱う。
 */
const ADR_PARENT_INACTIVE_LABELS = new Set([
  "needs-human",
  "needs-intent-review",
  "blocked-by-adr-retired",
]);

/**
 * 親 ADR Issue の labels から inactive かどうか判定。
 * Issue が allIssues (open のみ) に無ければ closed = accepted/retired 確定済みとみなして false。
 */
export function isParentAdrInactive(
  labels: string[],
  parentLabelsLookup: (n: number) => string[] | null,
): boolean {
  for (const n of extractParentAdrNumbers(labels)) {
    const parentLabels = parentLabelsLookup(n);
    if (parentLabels === null) continue;
    for (const pl of parentLabels) {
      if (ADR_PARENT_INACTIVE_LABELS.has(pl)) return true;
      if (pl.startsWith("gate:")) return true;
    }
  }
  return false;
}

// --- 引数解析 ---

type BatchArg = "fixes" | "phase" | "foundation";
let batchArg: BatchArg | null = null;
let dryRun = false;
let loopMode = false;
let roadmapFile = "ROADMAP.md";

{
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--batch": {
        const val = args[++i];
        if (val !== "fixes" && val !== "phase" && val !== "foundation") {
          console.error(`--batch の値は 'fixes' / 'phase' / 'foundation' を指定してください (got: ${val})`);
          process.exit(1);
        }
        batchArg = val as BatchArg;
        break;
      }
      case "--dry-run": dryRun = true; break;
      case "--loop": loopMode = true; break;
      case "--roadmap": roadmapFile = args[++i]; break;
      default:
        console.error(`Unknown arg: ${args[i]}`);
        process.exit(1);
    }
  }
}

// --- ユーティリティ ---

function getHeadSha(): string {
  const proc = Bun.spawnSync(["git", "rev-parse", "HEAD"], { stdout: "pipe", stderr: "pipe" });
  return new TextDecoder().decode(proc.stdout).trim();
}

/**
 * #210: foundation-batch tier の milestone gating
 *   milestone なし / 非 `Phase N` title → false (除外しない)
 *   milestone `Phase N` (N <= currentPhase) → false (除外しない、過去 Phase の振り返り作業)
 *   milestone `Phase N` (N > currentPhase) → true (除外、将来 Phase の起点 Issue)
 */
export function isFutureMilestoneTitle(
  milestoneTitle: string | null,
  currentPhase: number | null,
): boolean {
  if (milestoneTitle === null || currentPhase === null) return false;
  const m = milestoneTitle.match(/^Phase\s+(\d+)/);
  if (!m) return false;
  return parseInt(m[1], 10) > currentPhase;
}

/** ROADMAP.md から現在の (非✅) Phase 番号を返す */
function detectCurrentPhase(roadmapPath: string): number | null {
  if (!existsSync(roadmapPath)) return null;
  for (const line of readFileSync(roadmapPath, "utf-8").split("\n")) {
    // "## Phase N:" または "## ✅ Phase N:" にマッチし、✅ を含む行 (完了済み) はスキップ
    // コロン必須: "## Phase 9 以降の総括" のような解説見出しを誤検出しないため
    const m = line.match(/^##\s+(?:✅\s+)?Phase\s+(\d+):/);
    if (m && !line.includes("✅")) {
      return parseInt(m[1]);
    }
  }
  return null;
}

/** Issue title から短い slug を生成する (B-2 でユーザーが確認・修正可能) */
function deriveSlug(title: string, issueNum: number): string {
  // "type(scope): rest" または "type: rest" の prefix を除去し、scope を取得
  const scopeMatch = title.match(/^\w+\(([^)]+)\):\s*/);
  const scope = scopeMatch?.[1] ?? "";
  let rest = scopeMatch ? title.slice(scopeMatch[0].length) : title.replace(/^\w+:\s*/, "");

  // ASCII 単語を抽出 (小文字化)
  const asciiWords = (rest.match(/[a-zA-Z][a-zA-Z0-9]*/g) ?? []).map(w => w.toLowerCase());

  const parts: string[] = [];
  if (scope) parts.push(scope.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/-+$/, ""));
  parts.push(...asciiWords.slice(0, 3));

  if (parts.length === 0 || (parts.length === 1 && parts[0].length < 3)) {
    return `issue-${issueNum}`;
  }
  return parts
    .join("-")
    .replace(/-{2,}/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 40);
}

/**
 * ラベル一覧から deliverable を判定する
 * detect-deliverable.ts のラベル fallback ロジックと同等 (余分な gh 呼び出しを避けるため inline)
 */
function detectDeliverableFromLabels(labels: string[]): Deliverable {
  const s = labels.join(",");
  if (/\b(kernel|format|cli|viewer)\b/.test(s)) return "code";
  if (/\bdocs\b/.test(s)) return "docs";
  return "code";
}

/** Issue body から #N 参照を抽出 (重複除去) */
function parseIssueRefs(body: string): number[] {
  const seen = new Set<number>();
  const re = /#(\d+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(body)) !== null) {
    seen.add(parseInt(m[1]));
  }
  return [...seen];
}

/**
 * Kahn 法によるトポロジカルソート
 * @param depsMap n → n が依存するノード一覧 (先に処理されるべきもの)
 */
function kahnSort(
  nums: number[],
  depsMap: Map<number, number[]>,
): { order: number[]; hasCycle: boolean } {
  const inDeg = new Map<number, number>(nums.map(n => [n, 0]));
  const successors = new Map<number, number[]>(nums.map(n => [n, []]));

  for (const n of nums) {
    for (const dep of depsMap.get(n) ?? []) {
      // n が dep に依存する → dep → n のエッジ
      successors.get(dep)!.push(n);
      inDeg.set(n, inDeg.get(n)! + 1);
    }
  }

  const queue = nums.filter(n => inDeg.get(n) === 0).sort((a, b) => a - b);
  const order: number[] = [];

  while (queue.length > 0) {
    queue.sort((a, b) => a - b);
    const n = queue.shift()!;
    order.push(n);
    for (const succ of successors.get(n) ?? []) {
      inDeg.set(succ, inDeg.get(succ)! - 1);
      if (inDeg.get(succ) === 0) queue.push(succ);
    }
  }

  if (order.length < nums.length) {
    // 閉路検出 → Issue 番号昇順にフォールバック
    return { order: [...nums].sort((a, b) => a - b), hasCycle: true };
  }
  return { order, hasCycle: false };
}

// --- gh Issue 型 ---

interface GhLabel { name: string }
interface GhMilestone { title: string; number: number }
interface GhIssue {
  number: number;
  title: string;
  labels: GhLabel[];
  body: string;
  milestone: GhMilestone | null;
}

// --- メイン処理 ---

async function main() {
  // gh issue list でオープン Issue を一括取得 (単一 gh 呼び出し)
  const ghProc = Bun.spawn(
    ["gh", "issue", "list", "--state", "open", "--limit", "200",
     "--json", "number,title,labels,body,milestone"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const ghOut = await new Response(ghProc.stdout).text();
  await ghProc.exited;

  let allIssues: GhIssue[];
  try {
    allIssues = JSON.parse(ghOut);
  } catch {
    console.error("gh issue list の出力を JSON として解析できませんでした");
    process.exit(1);
  }

  const currentPhase = detectCurrentPhase(roadmapFile);
  const warnings: string[] = [];

  const labelNames = (issue: GhIssue) => issue.labels.map(l => l.name);
  const hasBatchLabel = (issue: GhIssue) => labelNames(issue).some(l => l.startsWith("batch:"));

  // 親 ADR 状態の lookup (allIssues は open Issue のみ含むため、closed = accepted/retired 確定済みとみなす)
  const issueByNumber = new Map<number, GhIssue>(allIssues.map(i => [i.number, i]));
  const parentLabelsLookup = (n: number): string[] | null => {
    const parent = issueByNumber.get(n);
    return parent ? labelNames(parent) : null;
  };
  const isLoopExcluded = (labels: string[]): boolean =>
    isLoopExcludedLabels(labels) || isParentAdrInactive(labels, parentLabelsLookup);
  const isFeatureTier = (labels: string[]) =>
    labels.some(l => l === "type:feature" || l === "type: feature");
  const isFoundationTier = (labels: string[]) =>
    labels.some(l => l === "type:foundation" || l === "type: foundation");
  // #254: refactor を独立 tier として認識する (ADR-002 type 軸の正規ラベル)
  const isRefactorTier = (labels: string[]) =>
    labels.some(l => l === "type:refactor" || l === "type: refactor");

  const isFutureMilestone = (issue: GhIssue): boolean =>
    isFutureMilestoneTitle(issue.milestone?.title ?? null, currentPhase);

  // --- 優先順位ラダーで対象 Issue を選択 ---
  let selected: GhIssue[] = [];
  let tier: BatchPlan["tier"] = "bug-batch";

  // 3ailoop モード: split-batch tier を最優先 (parent-blocked-by-split:<N> 子 Issue)
  if (loopMode && batchArg === null) {
    const splitChildrenRaw = allIssues.filter(i => isParentBlockedBySplitChild(labelNames(i)));
    const splitChildren = splitChildrenRaw.filter(i => !isLoopExcluded(labelNames(i)));
    const splitDowngraded = splitChildrenRaw.length - splitChildren.length;
    if (splitDowngraded > 0) {
      warnings.push(
        `split-batch tier: ${splitDowngraded} 件を降格 (gate/needs-* または親 ADR が inactive)`,
      );
    }
    if (splitChildren.length > 0) {
      selected = splitChildren;
      tier = "split-batch";
    }
  }

  if (selected.length > 0) {
    // split-batch で確定、ladder スキップ
  } else if (batchArg === "fixes") {
    // --batch fixes: bug + batch:* のみ
    selected = allIssues.filter(i => labelNames(i).includes("bug") && hasBatchLabel(i));
    tier = "bug-batch";
  } else if (batchArg === "foundation") {
    // --batch foundation: type:foundation + batch:* のみ
    // #210: 将来 Phase milestone は除外
    selected = allIssues.filter(
      i => isFoundationTier(labelNames(i)) && hasBatchLabel(i) && !isFutureMilestone(i),
    );
    tier = "foundation-batch";
  } else if (batchArg === "phase") {
    // --batch phase: 現 Phase milestone の type:feature のみ
    selected = allIssues.filter(
      i => i.milestone !== null &&
           currentPhase !== null &&
           i.milestone.title.startsWith(`Phase ${currentPhase}`) &&
           isFeatureTier(labelNames(i)),
    );
    tier = "phase-feature";
  } else {
    // 引数なし: 優先順位 1 → 2 → 3 → 4 → 5
    //   1: bug-batch         (bug + batch:*)
    //   2: enh-batch         (enhancement + batch:*) — ADR-002 正規外だが歴史互換
    //   3: foundation-batch  (type:foundation + batch:*)
    //   4: refactor-batch    (type:refactor + batch:*) — #254 で追加
    //   5: phase-feature     (現 Phase milestone の type:feature)
    //
    // #187: loop モードでは各 tier の filter 内で loop exclude も同時適用し、
    // exclude 後に 0 件なら次 tier に fall-through する。
    const applyLoopExclude = (xs: GhIssue[]) =>
      loopMode ? xs.filter(i => !isLoopExcluded(labelNames(i))) : xs;

    let excludedTotal = 0;
    const tryTier = (raw: GhIssue[]): GhIssue[] => {
      if (raw.length === 0) return [];
      const filtered = applyLoopExclude(raw);
      excludedTotal += raw.length - filtered.length;
      return filtered;
    };

    const bugBatch = tryTier(
      allIssues.filter(i => labelNames(i).includes("bug") && hasBatchLabel(i)),
    );
    if (bugBatch.length > 0) {
      selected = bugBatch;
      tier = "bug-batch";
    } else {
      const enhBatch = tryTier(
        allIssues.filter(i => labelNames(i).includes("enhancement") && hasBatchLabel(i)),
      );
      if (enhBatch.length > 0) {
        selected = enhBatch;
        tier = "enh-batch";
      } else {
        const foundationBatch = tryTier(
          // #210: 将来 Phase milestone (Phase N で N > currentPhase) は除外
          allIssues.filter(
            i => isFoundationTier(labelNames(i)) && hasBatchLabel(i) && !isFutureMilestone(i),
          ),
        );
        if (foundationBatch.length > 0) {
          selected = foundationBatch;
          tier = "foundation-batch";
        } else {
          // #254: refactor-batch tier を foundation の次に挿入
          //       (loop が永遠に pick できない問題の解消)
          const refactorBatch = tryTier(
            allIssues.filter(
              i => isRefactorTier(labelNames(i)) && hasBatchLabel(i) && !isFutureMilestone(i),
            ),
          );
          if (refactorBatch.length > 0) {
            selected = refactorBatch;
            tier = "refactor-batch";
          } else {
            selected = tryTier(
              allIssues.filter(
                i => i.milestone !== null &&
                     currentPhase !== null &&
                     i.milestone.title.startsWith(`Phase ${currentPhase}`) &&
                     isFeatureTier(labelNames(i)),
              ),
            );
            tier = "phase-feature";
          }
        }
      }
    }

    if (loopMode && excludedTotal > 0) {
      warnings.push(`loop モード: gate/needs-* で ${excludedTotal} 件除外 (残 ${selected.length} 件)`);
    }
  }

  // 3ailoop モード: 共通 exclude フィルタを適用 (gate:* / needs-* / blocked-by-split / 親 ADR inactive)
  // (引数なし起動時は上のラダー内で適用済み。--batch fixes/foundation/phase 経路では後段で適用)
  if (loopMode && batchArg !== null) {
    const beforeCount = selected.length;
    selected = selected.filter(i => !isLoopExcluded(labelNames(i)));
    const filtered = beforeCount - selected.length;
    if (filtered > 0) {
      warnings.push(`loop モード: gate/needs-*/親ADR inactive で ${filtered} 件除外 (残 ${selected.length} 件)`);
    }
  }

  // 空プランの場合
  if (selected.length === 0) {
    process.stderr.write("対象 Issue が見つかりませんでした。バッチ実行をスキップします。\n");
    const emptyPlan: BatchPlan = {
      generated_at: new Date().toISOString(),
      batch_start_sha: getHeadSha(),
      tier,
      batch_arg: batchArg,
      current_phase: currentPhase,
      groups: [],
      warnings: [],
    };
    const json = JSON.stringify(emptyPlan, null, 2);
    console.log(json);
    if (!dryRun) {
      mkdirSync("features/.batch", { recursive: true });
      writeFileSync("features/.batch/plan.json", json, "utf-8");
    }
    return;
  }

  const selectedNums = new Set(selected.map(i => i.number));

  // --- batch:* グループ割り当て ---
  interface IssueWithGroup extends GhIssue { group: string }
  const issuesWithGroup: IssueWithGroup[] = selected.map(issue => {
    const batchLabels = labelNames(issue).filter(l => l.startsWith("batch:"));
    let group = "batch:phase"; // synthetic fallback (type:feature issue with no batch:* label)
    if (batchLabels.length > 0) {
      group = batchLabels[0];
      if (batchLabels.length > 1) {
        warnings.push(
          `Issue #${issue.number}: 複数の batch:* ラベル [${batchLabels.join(", ")}]、先頭の ${group} を使用`,
        );
      }
    }
    return { ...issue, group };
  });

  // --- #N 依存参照の解析 ---
  const depsMap = new Map<number, number[]>();
  const rawRefsMap = new Map<number, number[]>();
  for (const issue of selected) {
    const allRefs = parseIssueRefs(issue.body ?? "");
    rawRefsMap.set(issue.number, allRefs);
    // 選択集合内の他 Issue を指す参照のみ依存として採用
    depsMap.set(
      issue.number,
      allRefs.filter(n => selectedNums.has(n) && n !== issue.number),
    );
  }

  // --- グループ別処理 ---
  const groupMap = new Map<string, IssueWithGroup[]>();
  for (const issue of issuesWithGroup) {
    if (!groupMap.has(issue.group)) groupMap.set(issue.group, []);
    groupMap.get(issue.group)!.push(issue);
  }

  // グループを固定優先度順にソート
  const sortedGroupNames = [...groupMap.keys()].sort(
    (a, b) => (GROUP_ORDER[a] ?? 99) - (GROUP_ORDER[b] ?? 99),
  );

  const groups: BatchGroup[] = [];

  for (let gIdx = 0; gIdx < sortedGroupNames.length; gIdx++) {
    const groupName = sortedGroupNames[gIdx];
    const groupIssues = groupMap.get(groupName)!;
    const groupNums = groupIssues.map(i => i.number);

    // グループ内の依存のみ (グループ間依存は v1 では無視)
    const intraGroupDeps = new Map<number, number[]>();
    for (const n of groupNums) {
      intraGroupDeps.set(n, (depsMap.get(n) ?? []).filter(d => groupNums.includes(d)));
    }
    const { order, hasCycle } = kahnSort(groupNums, intraGroupDeps);
    if (hasCycle) {
      warnings.push(
        `グループ ${groupName}: 依存グラフに閉路を検出、Issue 番号昇順にフォールバック`,
      );
    }

    const issueMap = new Map(groupIssues.map(i => [i.number, i]));
    const batchIssues: BatchIssue[] = order.map(num => {
      const issue = issueMap.get(num)!;
      const labels = labelNames(issue);
      const deliverable = detectDeliverableFromLabels(labels);

      // flow 分類: type:feature → full、それ以外 → light
      const flow: "light" | "full" = isFeatureTier(labels) ? "full" : "light";
      // kernel は幾何不変量リスクが高いため、light でも Codex 個別ゲート (STEP 7.5) を保持
      const keepCodexGate = groupName === "batch:kernel";

      const body = issue.body ?? "";
      const isAmbiguous = AMBIGUITY_MARKERS.test(body);

      const pauseReasons: string[] = [];
      if (labels.includes("needs-review")) pauseReasons.push("needs-review");
      if (isAmbiguous) pauseReasons.push("ambiguous");

      const gate: "auto" | "pause" = pauseReasons.length > 0 ? "pause" : "auto";
      // full Issue と ambiguous は intent-check 必須
      const intentCheckRequired = flow === "full" || isAmbiguous;

      return {
        number: issue.number,
        slug: deriveSlug(issue.title, issue.number),
        title: issue.title,
        labels,
        deliverable,
        flow,
        gate,
        pause_reasons: pauseReasons,
        ambiguous: isAmbiguous,
        intent_check_required: intentCheckRequired,
        keep_codex_gate: keepCodexGate,
        deps: (depsMap.get(num) ?? []).filter(d => groupNums.includes(d)),
        raw_refs: rawRefsMap.get(num) ?? [],
      };
    });

    groups.push({ group: groupName, order: gIdx, issues: batchIssues });
  }

  // --- プラン構築 ---
  const plan: BatchPlan = {
    generated_at: new Date().toISOString(),
    batch_start_sha: getHeadSha(),
    tier,
    batch_arg: batchArg,
    current_phase: currentPhase,
    groups,
    warnings,
  };

  const planJson = JSON.stringify(plan, null, 2);
  console.log(planJson);

  if (!dryRun) {
    mkdirSync("features/.batch", { recursive: true });
    writeFileSync("features/.batch/plan.json", planJson, "utf-8");
    process.stderr.write(
      `\n✅ 実行プランを features/.batch/plan.json に書き込みました\n` +
      `   対象 ${selected.length} 件 / グループ ${groups.length} 個 / tier: ${tier}\n`,
    );
  } else {
    process.stderr.write("\n（--dry-run: ディスク書き込みなし）\n");
  }
}

if (import.meta.main) {
  main().catch(e => { console.error(e); process.exit(1); });
}
