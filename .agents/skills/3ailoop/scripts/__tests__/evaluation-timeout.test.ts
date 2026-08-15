import { describe, expect, test } from "bun:test";
import {
  EvaluationDeadline,
  isInfrastructureFailure,
  type EvaluationTimeoutDeps,
  type EvaluationTimeoutKind,
} from "../lib/evaluation-timeout.ts";

class FakeTimers {
  now = 0;
  nextId = 1;
  timers = new Map<number, { at: number; fn: () => void }>();

  deps(): EvaluationTimeoutDeps {
    return {
      setTimer: (fn, ms) => {
        const id = this.nextId++;
        this.timers.set(id, { at: this.now + ms, fn });
        return id as unknown as ReturnType<typeof setTimeout>;
      },
      clearTimer: timer => this.timers.delete(timer as unknown as number),
    };
  }

  advance(ms: number): void {
    const target = this.now + ms;
    while (true) {
      const due = [...this.timers.entries()]
        .filter(([, timer]) => timer.at <= target)
        .sort((a, b) => a[1].at - b[1].at || a[0] - b[0])[0];
      if (!due) break;
      this.now = due[1].at;
      this.timers.delete(due[0]);
      due[1].fn();
    }
    this.now = target;
  }
}

function deadline(fake: FakeTimers, fired: EvaluationTimeoutKind[]): EvaluationDeadline {
  return new EvaluationDeadline(
    { firstOutputMs: 90, idleMs: 180, hardMs: 1_200 },
    kind => fired.push(kind),
    fake.deps(),
  );
}

describe("EvaluationDeadline", () => {
  test("classifies an endpoint with no output separately", () => {
    const fake = new FakeTimers();
    const fired: EvaluationTimeoutKind[] = [];
    deadline(fake, fired).start();
    fake.advance(90);
    expect(fired).toEqual(["first-output"]);
  });

  test("output cancels first-output deadline and resets idle deadline", () => {
    const fake = new FakeTimers();
    const fired: EvaluationTimeoutKind[] = [];
    const d = deadline(fake, fired);
    d.start();
    fake.advance(80);
    d.activity();
    fake.advance(170);
    d.activity();
    fake.advance(179);
    expect(fired).toEqual([]);
    fake.advance(1);
    expect(fired).toEqual(["idle"]);
  });

  test("hard deadline is not extended by activity", () => {
    const fake = new FakeTimers();
    const fired: EvaluationTimeoutKind[] = [];
    const d = deadline(fake, fired);
    d.start();
    fake.advance(80);
    d.activity();
    for (let i = 0; i < 11; i++) {
      fake.advance(100);
      d.activity();
    }
    fake.advance(20);
    expect(fired).toEqual(["hard"]);
  });

  test("stop cancels every deadline", () => {
    const fake = new FakeTimers();
    const fired: EvaluationTimeoutKind[] = [];
    const d = deadline(fake, fired);
    d.start();
    d.activity();
    d.stop();
    fake.advance(2_000);
    expect(fired).toEqual([]);
  });
});

test("infrastructure errors are recognized without treating ordinary failures as transport", () => {
  expect(isInfrastructureFailure("upstream timeout: 504 Gateway Timeout")).toBe(true);
  expect(isInfrastructureFailure("429 rate limit exceeded")).toBe(true);
  expect(isInfrastructureFailure("cargo test failed: assertion mismatch")).toBe(false);
});
