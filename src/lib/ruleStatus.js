import { writable } from "svelte/store";
import { notify, dismiss } from "./stores.js";
import { tr } from "./i18n.js";

export const ruleStatus = writable(null);
let previousErrors = "";
let toastId;

export function updateRuleStatus(report) {
  if (!report) return;
  const errors = [...new Set(report.errors || [])].sort();
  ruleStatus.set({ ...report, errors });
  const fingerprint = JSON.stringify(errors);
  if (fingerprint === previousErrors) return;
  previousErrors = fingerprint;
  if (toastId !== undefined) dismiss(toastId);
  toastId = undefined;
  if (errors.length) {
    toastId = notify(tr("rules.issueNotice", { count: errors.length }), "warning", 6000);
  }
}
