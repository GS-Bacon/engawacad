# 3ailoop runtime reference (new runtime, Codex-orchestrated)

This is the operational reference linked from every `3ailoop*` component Skill. It holds the
**schemas**, **safety rules**, and **deterministic invariants** the Skills rely on. Keep it the
single source of truth; component Skills stay thin and only sequence scripts + this reference.

## Roles and authority

- **Codex** is the orchestrator. Codex is required for every Issue's plan, every worker
  consultation decision, and every delivery/landing decision.
- **Workers** are qualified **free cloud models** (OpenCode first). A worker may inspect, edit,
  test, and create a local commit **only inside its assigned isolated worktree**. It may NOT:
  push, mutate GitHub, land into the default branch, edit another worker's worktree, or use a
  shared stash.
- Codex constrains the **delivery slice**, not the worker's route through it. A slice fixes one
  objective, observable acceptance, non-goals, and authority boundary. Inside it the worker chooses
  its own investigation order, files, implementation, and focused tests. Exact command/edit steps
  are not prescribed unless an operation is fragile enough to require deterministic handling.
- **Never** use Codex as the implementation worker. **Never** use a paid fallback. If no qualified
  free worker exists, **stop and notify** the user.

## State, receipts, Git, and landing

### CAS state file: `features/.loop/assign-state.json`

Locked compare-and-set transitions with a monotonically checked `generation` per Issue. Every
mutating call passes the `expectedGeneration` it read when deciding; a mismatch is a
`stale-generation` failure and the caller must re-read and re-decide (this is the #333 defense:
a stale plan cannot reassign an Issue).

Per-Issue fields: `generation`, `status` (`unassigned | assigned | verifying | landing | closed`),
`worker_id`, `assigned_at`, `closed`, `closed_at`, `last_outcome`.

Invariants enforced by `lib/cas-state.ts`:
- assigning a **closed** Issue → `closed` failure (#333).
- assigning an Issue already owned by another worker → `already-assigned` failure (double-assign).
- a terminal outcome (`failed`/`rate_limited`) returns the Issue to `unassigned`; `completed`/
  `needs_supervisor` preserve ownership for landing/consultation.
- terminal worker cleanup: `recordTerminal` always frees the worker slot (#333 stuck-busy).

### Worker receipt (schema v1)

```
{
  "schema_version": 1,
  "issue": 295,
  "worker_id": "worker-1",
  "status": "completed" | "needs_supervisor" | "failed" | "rate_limited",
  "model": { "provider": "opencode", "model": "deepseek-v4-flash-free" },
  "session_id": "<opaque>",
  "summary": "...",
  "commit_sha": "<sha>",            // required when completed
  "changed_files": ["..."],         // required when completed; must cover the FULL diff
  "forbidden_operations": [],        // any non-empty entry fails verification immediately
  "test_commands": [{ "command": "...", "rc": 0, "output_summary": "..." }],
  "unverified_items": ["..."],
  "consultation": {                 // required when needs_supervisor
    "question": "...",
    "options": ["...", "..."],       // >= 2
    "recommendation": "..."
  },
  "emitted_at": "<ISO>"
}
```

Validation rules (`lib/receipt.ts`): required fields + types; `completed` ⇒ `commit_sha` +
`changed_files`; `needs_supervisor` ⇒ `consultation`; any `forbidden_operations` ⇒ fail.
`forbidden_operations` records operations actually executed, not the constraint list; use `[]`
when none occurred. The terminal harness binds identity and the observed emission time, then
validates the receipt before it can emit a completed terminal state.

### Complete diff (#330)

Verification considers the **complete** worktree delta: tracked, staged, committed, **and
untracked** files. A receipt that omits an untracked file the worker created → `completeness-fail`
(#330: a missing untracked file must fail completeness rather than produce a false clean result).

### Serialized landing

`lib/land-serial.ts` serializes via a file lock so only one land runs at a time. Before any
default-branch mutation it **revalidates**: receipt validity, CAS generation, Issue-closed state,
worker ownership, scope, complete diff, and tests. A conflict or CI/test failure returns a bounded,
explicit `outcome` (`rebase-conflict | ci-red | push-failed | scope-drift | stale-generation |
receipt-invalid | closed-issue | completeness-fail | internal-error`) — never destructive recovery.

## Free-model policy

1. Query the installed OpenCode CLI machine-readably (`opencode models opencode --verbose`). The
   explicit provider filter prevents configured paid/account providers and automatic routers from
   entering the OpenCode-free candidate set.
2. A candidate is **eligible** only when: `status == active`, input AND output cost are explicit
   zero, and `capabilities.toolcall == true`.
3. **Mechanically reject** any provider/model/runtime containing or representing local inference
   (Ollama, LM Studio, vLLM, llama.cpp, …), Gemini, or a nonzero/**unknown** price. **Unknown
   price is not free.**
4. Evaluate OpenCode candidates first. Consult an external free provider **only** when the OpenCode
   candidate set is empty, all fail evaluation, all expire/rate-limit, or OpenCode persistently fails.
5. External candidates must be cloud-hosted, non-Gemini, officially free, usable noninteractively
   through a resumable CLI/API, pinned to a specific provider+model, and configured so paid/
   automatic fallback is **impossible**. No automatic free-model router. If account creation,
   terms acceptance, billing info, or credentials are missing → **stop for the user**.
6. If no qualified free worker is available → **stop and notify**. Never Codex-as-worker. Never paid.
7. Re-evaluate initially and thereafter **only** on: expiry, auth failure, rate limiting, persistent
   harness failure, or observed quality degradation. **No recurring weekly discovery.**
8. On retirement, write a concise report (dates/evidence, eval + production outcomes, failures/rate
   limits, replacement reason) to `features/.loop/model-retirements.jsonl`.

## Evaluation and registry

- Ranking: fixed EngawaCAD task suite > comparable public benchmark under the same harness >
  reputation (supporting context only).
- Suite v3 exercises a bounded Rust bug fix, a multi-crate feature, diagnosis + regression test,
  and production-equivalent Skill/scope/local-commit/receipt compliance. Every implementation
  task has optional `$ask-codex`; consultation is neither required nor independently scored. It
  must not mutate the real default branch or GitHub.
- Qualification requires: zero forbidden external operations, zero scope drift, passing required
  tests, a valid receipt, and success on **≥2 of 3** implementation tasks. Any safety violation
  fails the model.
- Persist (`features/.loop/model-registry.json`): harness/provider/model, status
  (`qualified | probation | retired`), evaluation suite version/results, supported task class,
  free-tier evidence + check date, known/unknown expiry, recent success/failure/rate-limit.
- Keep safety, receipt, terminal-state and authority rules common to every model. Derive a compact
  `task_profile` from the fixed suite (maximum slice scope/touchpoints, passed task classes,
  ambiguity handling, negative-test capability, starting-context size). Persist consultation
  count/assisted completions as cost evidence after task success. Match the profile against the
  slice `requirements.json`; if nothing fits, split smaller or stop rather than weakening gates.

## Async delegation (no polling)

Long-running worker processes integrate with the repository's async, event-driven delegation
convention (AGENTS.md "External AI delegation"). The current turn **enqueues** a job and emits a
delegation marker, then ends the turn. The worker process is launched from Codex's turn-complete
by running the job descriptor's `launch.command` (noninteractive, model-pinned, in `launch.cwd` =
the assigned isolated worktree); on its terminal notification (success / rate-limit / abnormal),
the same Codex thread resumes and runs verify + land. **Do not** poll, sleep-loop, or repeatedly
check process/state files.

### Bounded slices and fresh context

- Split an Issue by independently verifiable outcome, not by arbitrary file count.
- Prefer one outcome, one focused test family, and roughly one to four primary touchpoints. Crossing
  that estimate is allowed when the atomic change genuinely spans more files.
- A completed slice ends with a local commit and receipt. Codex verifies the real delta before
  defining the next slice.
- Reuse the isolated worktree state, but start a fresh model session for the next normal slice.
  Pass only required files plus a concise receipt/diff/test handoff; do not replay the prior model
  conversation or raw tool trace.
- Same-session resume is reserved for answering a `needs_supervisor` consultation. Keep the
  harness-observed session ID, consultation record ID, and Codex decision as distinct values.
- Large context is a harness-quality signal. Around 40k active context or 15–20 model steps, the
  next slice should be made smaller; do not micromanage the current worker solely to hit a number.

`run-worker.ts` writes the job descriptor (with the executable `launch`, `receipt_path`, and
constraints) both to stdout (the marker) and to
`features/.loop/worker-queue/<marker>.json`, then submits the visible command directly to
`$async-agent-delegation`. Its returned `[agent-delegation:<uuid>]` is the turn-complete marker.
The `launch` command is the concrete noninteractive
invocation (e.g. `opencode run --model <provider/model> [--session <session-id>] "<prompt>"`) the harness
executes; it instructs the worker to write a receipt at `receipt_path` on a terminal outcome.
For OpenCode, `opencode-worker-terminal.ts` derives session/audit/terminal truth from JSONL and
binds the observed identity into the receipt; model self-report is not trusted for those fields.

## Worker tmux pane observability (operator-only)

contract §"Orchestration and authority" #5/#6 require each **active external worker** to be
given a dedicated **right-side** tmux pane. The pane is an event dashboard, not a raw transcript:
`pane-render.ts` reduces the ordinary CLI stream to lifecycle and tool summaries such as
**RUNNING / STEP / LOOK / EDIT / TEST / COMMIT / RETRY / ERROR / verdict**. Model text, hidden
reasoning, credentials, and arbitrary command bodies are never displayed. The exact unmodified
stream is still appended to the durable log named by visible-command state for later audit.

This is realized by `lib/worker-pane.ts` (pure-ish logic behind an injectable `WorkerPaneDeps`
tmux adapter, mirroring `LandDeps`) and the `scripts/worker-pane.ts` CLI (the real tmux adapter).
The job descriptor emitted by `run-worker.ts` carries a `pane` field with the allocation contract:

- `allocate` — split a right-side pane (`-h`) and stamp the labeled status line.
- `stream` — send the executable `launch.command` through `pane-render.ts`; only safe event
  summaries are visible in the pane (the launch is still driven event-driven; the pane is not polled).
- `release` — on the terminal notification, deterministically clean up (kill-pane). The ordinary
  output remains in the durable log named by visible-command state; a fresh pane is allocated for
  reuse. `--retain-pane` / `LOOP_RETAIN_WORKER_PANE=1` is an explicit debugging-only override.

**Pane visibility is an operator interface only (contract #6).** Scheduling, completion, failure
detection, consultation, and resume are driven by **durable CAS state, receipts, and event-driven
termination** — never by scraping panes or polling processes. Pane allocation/stream/release
**never mutate** CAS `assign-state.json` or the worker registry; a pane-layer failure or cleanup
cannot affect lifecycle correctness. The N=3 dry run (`scripts/dry-run.ts`) proves this with a
fake tmux adapter: each fake worker gets a labeled right-side pane, output stays observable
through terminal completion, and pane cleanup/reuse leaves CAS+registry correct.

If an evaluation dashboard loses its input while the evaluation itself is healthy, do not stop
or restart the evaluation. Move its existing execution pane to a detached tmux window, then run
`scripts/evaluation-pane-follow.ts` in a fresh right-side pane. The follower reads the screen and
qualification JSONL artifacts with `fs.watch`, renders only the same safe summaries, and exits
when the durable visible-command state becomes terminal. It does not poll or control the worker.
The evaluation view shows separate SCREEN and QUALIFY progress bars. Before the grader publishes
a verdict, the screen score is explicitly provisional and counts five observable gates: response,
edit, passing agent test, local commit, and receipt. Published suite results replace it with the
formal five-task qualification score. Tool-policy rejections and provider retries are separate
operational counters and are never presented as model-score deductions.
The live header must be unambiguous in tmux: show an increasing `RUNNING 00000s` counter,
an animated scan marker, and seconds since the last real event. The 500ms heartbeat redraws only
held display state; it must never read worker state or become a lifecycle poll.

## Cutover

One-shot, idempotent, fail-safe. First detect and stop/reconcile old watchers, locks, registries,
queues, and worktrees, then enable the new runtime. It must never leave both orchestrators active.
Do **not** start the autonomous loop or perform a live cutover as part of automated tests or
delivery verification.
