export type EvaluationTimeoutKind = "first-output" | "idle" | "hard";

export type EvaluationTimeoutConfig = {
  firstOutputMs: number;
  idleMs: number;
  hardMs: number;
};

type Timer = ReturnType<typeof setTimeout>;

export type EvaluationTimeoutDeps = {
  setTimer: (fn: () => void, ms: number) => Timer;
  clearTimer: (timer: Timer) => void;
};

const realTimers: EvaluationTimeoutDeps = {
  setTimer: (fn, ms) => setTimeout(fn, ms),
  clearTimer: timer => clearTimeout(timer),
};

export const DEFAULT_EVALUATION_TIMEOUTS: EvaluationTimeoutConfig = {
  firstOutputMs: 3 * 60_000,
  idleMs: 8 * 60_000,
  hardMs: 20 * 60_000,
};

/**
 * Event-driven deadline controller for model evaluations.
 *
 * It distinguishes an unavailable endpoint (no first output), a stalled session
 * (no output after activity), and a bounded absolute ceiling. `activity()` resets
 * only the idle deadline; the hard deadline always remains fixed.
 */
export class EvaluationDeadline {
  private firstTimer: Timer | null = null;
  private idleTimer: Timer | null = null;
  private hardTimer: Timer | null = null;
  private active = false;
  private sawOutput = false;

  constructor(
    private readonly config: EvaluationTimeoutConfig,
    private readonly onTimeout: (kind: EvaluationTimeoutKind) => void,
    private readonly deps: EvaluationTimeoutDeps = realTimers,
  ) {}

  start(): void {
    if (this.active) return;
    this.active = true;
    this.firstTimer = this.deps.setTimer(() => this.expire("first-output"), this.config.firstOutputMs);
    this.hardTimer = this.deps.setTimer(() => this.expire("hard"), this.config.hardMs);
  }

  activity(): void {
    if (!this.active) return;
    if (!this.sawOutput) {
      this.sawOutput = true;
      this.clear("first");
    }
    this.clear("idle");
    this.idleTimer = this.deps.setTimer(() => this.expire("idle"), this.config.idleMs);
  }

  stop(): void {
    if (!this.active) return;
    this.active = false;
    this.clear("first");
    this.clear("idle");
    this.clear("hard");
  }

  private expire(kind: EvaluationTimeoutKind): void {
    if (!this.active) return;
    this.stop();
    this.onTimeout(kind);
  }

  private clear(which: "first" | "idle" | "hard"): void {
    const timer = which === "first" ? this.firstTimer : which === "idle" ? this.idleTimer : this.hardTimer;
    if (timer !== null) this.deps.clearTimer(timer);
    if (which === "first") this.firstTimer = null;
    else if (which === "idle") this.idleTimer = null;
    else this.hardTimer = null;
  }
}

export function isInfrastructureFailure(text: string): boolean {
  return /(?:\b429\b|\b50[234]\b|rate.?limit|temporar(?:y|ily) unavailable|connection (?:reset|refused)|network error|upstream (?:error|timeout))/i.test(text);
}
