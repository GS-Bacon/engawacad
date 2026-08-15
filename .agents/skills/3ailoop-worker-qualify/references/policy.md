# Free worker pool policy

## Pool

- Target: up to three qualified fixed models.
- Selection: evaluation pass count, then fixed-suite wall time, then identity.
- One qualified model is enough to start. Replenish lazily instead of delaying work to find a theoretical best model.
- A failed model stays probation or retired; never relax the threshold to fill the pool.

## Qualification suite v2

1. bounded Rust deterministic-ID bug fix;
2. multi-crate API feature;
3. failure diagnosis and regression test;
4. scope, local commit, test, and receipt compliance;
5. `needs_supervisor` receipt and same OpenCode session resume.

Any forbidden operation or scope drift fails qualification. At least two of the three implementation tasks and both harness tasks must pass.

Run every candidate first through the same common harness. Derive a compact task profile from the
fixed-suite evidence; do not add model-specific edit sequences. Selection matches a slice's scope,
touchpoints, task class, ambiguity, and negative-test requirement against that profile.

## Availability

- `model`: only the affected fixed model is temporarily unavailable.
- `provider`: every model using the provider is unavailable until reset.
- `unknown`: try the next already-qualified model once. If it receives the same limit response, record provider scope and stop.

Do not discover on a timer. Trigger replenishment only for initial setup, pool size below three, expiry, auth failure, rate limit, persistent harness failure, or quality degradation.

## Evaluation timeout classification

- Run evaluation through a loopback-only, password-protected `opencode serve` instance and consume `/event` SSE. Do not infer provider state only from `opencode run` stdout/stderr.
- `first-output`: no model event before the initial deadline. Treat as infrastructure unavailable and retry once with a fresh fixture.
- `idle`: output started but no further event arrived before the idle deadline. Preserve edit/test/commit/receipt progress in the result. If the isolated grader test passes, mark the attempt inconclusive rather than a capability failure.
- `hard`: absolute safety ceiling independent of activity. Record the reached progress before classification.
- 429, upstream 502/503/504, and connection failures are infrastructure evidence, not model-quality evidence.
- A 429 / `FreeUsageLimitError` / `Free usage exceeded` ends the attempt immediately and is never retried immediately. After the same limit appears on two distinct OpenCode models, record provider cooldown and skip the remaining candidates in that run.
- Retry first-output, connection, or upstream 502/503/504 at most once in screening, always with a new fixture. Never retry an infrastructure failure inside a partially used qualification fixture.
- Only `passed` candidates proceed automatically. `inconclusive` candidates remain eligible for a later event-triggered re-evaluation but are never promoted by lowering the qualification threshold.

## Fixture integrity

- Compare the baseline against committed, staged, unstaged, and untracked files.
- Generate and commit deterministic build artifacts required by the grader, including Rust `Cargo.lock`, before taking the baseline.
- Require `receipt.changed_files` to exactly equal the complete diff; both omissions and overclaims fail.
- Consultation must leave no code diff of any kind before supervisor resume.
- Infrastructure-inconclusive qualification never replaces a previous qualified evaluation or resets production observations. A real capability rejection may demote to probation.

## Records

- Registry: `features/.loop/model-registry.json`
- Screen: `features/.loop/model-eval/screen-v1/results.json`
- Qualification: `features/.loop/model-eval/qualification-v1/results.json`
- Retirement: `features/.loop/model-retirements.jsonl`
- Durable migration log: `docs/development/3ailoop-codex-migration.md`
