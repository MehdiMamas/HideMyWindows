import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { get } from "svelte/store";

let updateRuleStatus, ruleStatus, toasts;
beforeEach(async () => {
  vi.useFakeTimers();
  vi.resetModules();
  ({ updateRuleStatus, ruleStatus } = await import("./ruleStatus.js"));
  ({ toasts } = await import("./stores.js"));
});
afterEach(() => vi.useRealTimers());

const report = (errors, hiddenWindows = 1) => ({ matchedWindows: 2, hiddenWindows, unknownWindows: 0, errors });

it("combines multiple target errors and never repeats an unchanged failure", () => {
  updateRuleStatus(report(["Hide process 2: denied", "Hide process 1: denied"]));
  updateRuleStatus(report(["Hide process 1: denied", "Hide process 2: denied", "Hide process 1: denied"]));
  expect(get(toasts)).toHaveLength(1);
  expect(get(toasts)[0].message).toContain("2 issues");
  vi.advanceTimersByTime(6000);
  updateRuleStatus(report(["Hide process 2: denied", "Hide process 1: denied"], 2));
  expect(get(toasts)).toHaveLength(0);
  expect(get(ruleStatus).hiddenWindows).toBe(2);
});

it("replaces the rule notification when issues change and clears it on recovery", () => {
  updateRuleStatus(report(["First failure"]));
  updateRuleStatus(report(["Different failure"]));
  expect(get(toasts)).toHaveLength(1);
  expect(get(ruleStatus).errors).toEqual(["Different failure"]);
  updateRuleStatus(report([], 2));
  expect(get(toasts)).toHaveLength(0);
  expect(get(ruleStatus).errors).toEqual([]);
});
