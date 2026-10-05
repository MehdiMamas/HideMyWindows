//! Evaluates window rules against the live set of windows and applies the
//! matching hide/unhide actions. The scheduling (how often to run) is owned by
//! the caller; this module provides one stateless pass.

use crate::hider::{apply_to_process, apply_to_window};
use crate::model::{HideAction, RuleTarget, WindowRule};
use crate::process::process_name;
use crate::window::list_top_windows;
use std::collections::{HashMap, HashSet};

fn is_process_level(action: HideAction) -> bool {
    matches!(
        action,
        HideAction::HideProcessWindows | HideAction::UnhideProcessWindows
    )
}

/// Run a single rule-matching pass over all visible top-level windows.
///
/// * `persistent_only` restricts the pass to rules flagged persistent (used by
///   the frequent re-apply loop); the slower discovery pass runs all rules.
///
/// Returns a list of human-readable error strings for actions that failed, so
/// the caller can surface or log them without the pass aborting.
pub fn apply_rules_once(
    rules: &[WindowRule],
    payload_path: &str,
    persistent_only: bool,
) -> Vec<String> {
    let mut errors = Vec::new();

    let windows = match list_top_windows(true) {
        Ok(w) => w,
        Err(e) => return vec![e.0],
    };

    // Cache process names per pid for this pass (each lookup opens the process).
    let mut name_cache: HashMap<u32, String> = HashMap::new();
    // Avoid applying the same process-level action to the same pid twice.
    let mut applied: HashSet<(u32, &'static str)> = HashSet::new();

    let active: Vec<&WindowRule> = rules
        .iter()
        .filter(|r| r.enabled && !r.value.is_empty() && (!persistent_only || r.persistent))
        .collect();

    if active.is_empty() {
        return errors;
    }

    for w in &windows {
        for rule in &active {
            let value: String = match rule.target {
                RuleTarget::WindowTitle => w.title.clone(),
                RuleTarget::WindowClass => w.class.clone(),
                RuleTarget::ProcessId => w.pid.to_string(),
                RuleTarget::ProcessName => name_cache
                    .entry(w.pid)
                    .or_insert_with(|| process_name(w.pid).unwrap_or_default())
                    .clone(),
            };

            if value.is_empty() || !rule.matches(&value) {
                continue;
            }

            let result = if is_process_level(rule.action) {
                let key = (w.pid, action_key(rule.action));
                if applied.contains(&key) {
                    continue;
                }
                applied.insert(key);
                apply_to_process(w.pid, rule.action, payload_path)
            } else {
                apply_to_window(w.hwnd, rule.action, payload_path)
            };

            if let Err(e) = result {
                errors.push(format!("Rule '{}': {}", rule.value, e.0));
            }
        }
    }

    errors
}

fn action_key(action: HideAction) -> &'static str {
    match action {
        HideAction::HideProcessWindows => "hide-all",
        HideAction::UnhideProcessWindows => "unhide-all",
        HideAction::HideWindow => "hide-win",
        HideAction::UnhideWindow => "unhide-win",
        HideAction::HideTrayIcon => "hide-tray",
        HideAction::UnhideTrayIcon => "unhide-tray",
    }
}
