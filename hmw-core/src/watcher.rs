//! Evaluates window rules against the live set of windows and applies the
//! matching hide/unhide actions. The scheduling (how often to run) is owned by
//! the caller.
//!
//! [`RuleSession`] remembers which hides the rules have applied. A full pass
//! unhides anything a current rule no longer asks to hide. A persistent-only
//! pass records new persistent hides and does not release, because that pass
//! cannot see non-persistent rules. Process hides the payload is already
//! keeping are not sent again; its in-process worker covers windows created
//! later, and repeating `HmwHideAll` would rewrite capture exclusion.

use crate::hider::{apply_to_process, apply_to_window};
use crate::model::{HideAction, RuleTarget, WindowRule};
use crate::process::{list_processes, process_is_alive, process_name};
use crate::window::{list_top_windows, window_is_alive, window_pid, TopWindow};
use std::collections::{HashMap, HashSet};

/// Hides currently owned by window rules.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct HideSet {
    processes: HashSet<u32>,
    windows: HashSet<isize>,
    trays: HashSet<isize>,
}

/// An explicit unhide rule match. These still run when no hide rule owns the
/// target, so a rule can undo a manual dashboard hide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnhideOp {
    Process(u32),
    Window(isize),
    Tray(isize),
}

/// Remembers hides applied by window rules and releases them when the rules
/// no longer ask for those hides.
#[derive(Debug, Default)]
pub struct RuleSession {
    applied: HideSet,
}

impl RuleSession {
    /// Read capture affinity for the current hide matches; never infer success
    /// merely from an injection completing or a rule being enabled.
    pub fn status(&self, rules: &[WindowRule]) -> crate::rule_status::RuleStatus {
        let mut status = crate::rule_status::RuleStatus::default();
        let windows = match list_top_windows(true) {
            Ok(windows) => windows,
            Err(error) => return status.with_errors(vec![error.0]),
        };
        let (desired, _) = evaluate(rules, &windows, false, &mut HashMap::new());
        for window in windows {
            if desired.processes.contains(&window.pid) || desired.windows.contains(&window.hwnd) {
                status.matched_windows += 1;
                match crate::window::is_capture_hidden(window.hwnd) {
                    Ok(true) => status.hidden_windows += 1,
                    Ok(false) => {}
                    Err(_) => status.unknown_windows += 1,
                }
            }
        }
        status
    }

    pub fn new() -> Self {
        Self {
            applied: HideSet::default(),
        }
    }

    /// One matching pass.
    ///
    /// * `persistent_only` restricts the pass to rules flagged persistent. That
    ///   pass records new hides and does not release. Process hides already
    ///   recorded are left to the payload worker.
    /// * A full pass releases hides that no longer match, then applies the
    ///   hides current rules ask for.
    ///
    /// Returns human-readable errors for actions that failed. A gone process
    /// or window is dropped quietly.
    pub fn apply(
        &mut self,
        rules: &[WindowRule],
        payload_path: &str,
        persistent_only: bool,
    ) -> Vec<String> {
        let windows = match list_top_windows(true) {
            Ok(w) => w,
            Err(e) => return vec![e.0],
        };

        let mut name_cache: HashMap<u32, String> = HashMap::new();
        let (desired, unhides) = evaluate(rules, &windows, persistent_only, &mut name_cache);
        let mut errors = Vec::new();

        if persistent_only {
            let outcome = apply_hides(&desired, &self.applied, payload_path);
            errors.extend(outcome.errors);
            errors.extend(apply_unhides(&unhides, payload_path, &desired));
            union_into(&mut self.applied, &outcome.kept);
            return errors;
        }

        // Release before applying, so unhiding a process cannot wipe a hide
        // this same pass is about to put back.
        let live_pids: HashSet<u32> = windows.iter().map(|w| w.pid).collect();
        errors.extend(release_stale(
            &self.applied,
            &desired,
            &live_pids,
            payload_path,
        ));
        errors.extend(apply_unhides(&unhides, payload_path, &desired));
        let outcome = apply_hides(&desired, &self.applied, payload_path);
        errors.extend(outcome.errors);
        self.applied = outcome.kept;
        for &pid in &self.applied.processes {
            if let Err(error) = crate::hider::check_protection(pid, payload_path) {
                if process_is_alive(pid) {
                    errors.push(format!("Process {pid}: {}", error.0));
                }
            }
        }
        errors
    }
}

fn union_into(applied: &mut HideSet, extra: &HideSet) {
    applied.processes.extend(&extra.processes);
    applied.windows.extend(&extra.windows);
    applied.trays.extend(&extra.trays);
}

fn evaluate(
    rules: &[WindowRule],
    windows: &[TopWindow],
    persistent_only: bool,
    name_cache: &mut HashMap<u32, String>,
) -> (HideSet, Vec<UnhideOp>) {
    let mut desired = HideSet::default();
    let mut unhides = Vec::new();

    let active: Vec<&WindowRule> = rules
        .iter()
        .filter(|r| r.enabled && !r.value.is_empty() && (!persistent_only || r.persistent))
        .collect();

    // Process rules do not need to wait for a first visible window. This also
    // retains protection while an app has temporarily closed all its windows.
    if let Ok(processes) = list_processes() {
        for process in processes {
            if !crate::process::in_current_session(process.pid) {
                continue;
            }
            for rule in &active {
                let value = match rule.target {
                    RuleTarget::ProcessName => process.name.clone(),
                    RuleTarget::ProcessId => process.pid.to_string(),
                    _ => continue,
                };
                if !rule.matches(&value) {
                    continue;
                }
                match rule.action {
                    HideAction::HideProcessWindows => {
                        desired.processes.insert(process.pid);
                    }
                    HideAction::UnhideProcessWindows => {
                        unhides.push(UnhideOp::Process(process.pid))
                    }
                    _ => {}
                }
            }
        }
    }

    for w in windows {
        for rule in &active {
            let value = match rule.target {
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

            match rule.action {
                HideAction::HideProcessWindows => {
                    desired.processes.insert(w.pid);
                }
                HideAction::HideWindow => {
                    desired.windows.insert(w.hwnd);
                }
                HideAction::HideTrayIcon => {
                    desired.trays.insert(w.hwnd);
                }
                HideAction::UnhideProcessWindows => unhides.push(UnhideOp::Process(w.pid)),
                HideAction::UnhideWindow => unhides.push(UnhideOp::Window(w.hwnd)),
                HideAction::UnhideTrayIcon => unhides.push(UnhideOp::Tray(w.hwnd)),
            }
        }
    }

    (desired, unhides)
}

/// Process hides `desired` still needs that `already` has not applied.
///
/// The payload worker keeps those processes hidden, including windows they
/// create later, so the host must not call `HmwHideAll` again.
fn pids_to_hide(desired: &HideSet, already: &HideSet) -> Vec<u32> {
    desired
        .processes
        .difference(&already.processes)
        .copied()
        .collect()
}

/// Pids a previous pass hid that the current rules do not.
fn pids_to_release(applied: &HideSet, desired: &HideSet) -> Vec<u32> {
    applied
        .processes
        .difference(&desired.processes)
        .copied()
        .collect()
}

/// Hwnds present in `applied` but not in `desired`.
fn hwnds_to_release(applied: &HashSet<isize>, desired: &HashSet<isize>) -> Vec<isize> {
    applied.difference(desired).copied().collect()
}

/// A window hide stays while its process is still covered by a process-level
/// hide. Releasing it here would show the window before the process hide is
/// re-applied.
fn should_unhide_window(hwnd: isize, pid: u32, desired: &HideSet) -> bool {
    !desired.windows.contains(&hwnd) && !desired.processes.contains(&pid)
}

fn process_still_running(pid: u32, live_pids: &HashSet<u32>) -> bool {
    live_pids.contains(&pid) || process_is_alive(pid)
}

fn release_stale(
    applied: &HideSet,
    desired: &HideSet,
    live_pids: &HashSet<u32>,
    payload_path: &str,
) -> Vec<String> {
    let mut errors = Vec::new();

    for pid in pids_to_release(applied, desired) {
        if !process_still_running(pid, live_pids) {
            continue;
        }
        if let Err(e) = apply_to_process(pid, HideAction::UnhideProcessWindows, payload_path) {
            if process_still_running(pid, live_pids) {
                errors.push(format!("Unhide process {pid}: {}", e.0));
            }
        }
    }

    for hwnd in hwnds_to_release(&applied.windows, &desired.windows) {
        if !window_is_alive(hwnd) {
            continue;
        }
        let pid = window_pid(hwnd);
        if !should_unhide_window(hwnd, pid, desired) {
            continue;
        }
        if let Err(e) = apply_to_window(hwnd, HideAction::UnhideWindow, payload_path) {
            if window_is_alive(hwnd) {
                errors.push(format!("Unhide window {hwnd}: {}", e.0));
            }
        }
    }

    for hwnd in hwnds_to_release(&applied.trays, &desired.trays) {
        if !window_is_alive(hwnd) {
            continue;
        }
        if let Err(e) = apply_to_window(hwnd, HideAction::UnhideTrayIcon, payload_path) {
            if window_is_alive(hwnd) {
                errors.push(format!("Unhide tray {hwnd}: {}", e.0));
            }
        }
    }

    errors
}

struct HideApply {
    errors: Vec<String>,
    /// Hides that are in effect after this call. Failed process hides are
    /// omitted so the next pass retries them.
    kept: HideSet,
}

fn apply_hides(desired: &HideSet, already: &HideSet, payload_path: &str) -> HideApply {
    let mut errors = Vec::new();
    let mut kept = HideSet {
        processes: already
            .processes
            .intersection(&desired.processes)
            .copied()
            .collect(),
        windows: desired.windows.clone(),
        trays: desired.trays.clone(),
    };

    for pid in pids_to_hide(desired, already) {
        if let Err(e) = apply_to_process(pid, HideAction::HideProcessWindows, payload_path) {
            errors.push(format!("Hide process {pid}: {}", e.0));
        } else {
            kept.processes.insert(pid);
        }
    }

    for &hwnd in &desired.windows {
        if desired.processes.contains(&window_pid(hwnd)) {
            continue;
        }
        if let Err(e) = apply_to_window(hwnd, HideAction::HideWindow, payload_path) {
            errors.push(format!("Hide window {hwnd}: {}", e.0));
        }
    }

    for &hwnd in &desired.trays {
        if let Err(e) = apply_to_window(hwnd, HideAction::HideTrayIcon, payload_path) {
            errors.push(format!("Hide tray {hwnd}: {}", e.0));
        }
    }

    HideApply { errors, kept }
}

fn apply_unhides(ops: &[UnhideOp], payload_path: &str, desired: &HideSet) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen_proc = HashSet::new();
    let mut seen_win = HashSet::new();
    let mut seen_tray = HashSet::new();

    for op in ops {
        match *op {
            UnhideOp::Process(pid) => {
                if desired.processes.contains(&pid) || !seen_proc.insert(pid) {
                    continue;
                }
                if let Err(e) =
                    apply_to_process(pid, HideAction::UnhideProcessWindows, payload_path)
                {
                    errors.push(format!("Unhide process {pid}: {}", e.0));
                }
            }
            UnhideOp::Window(hwnd) => {
                let pid = window_pid(hwnd);
                if desired.windows.contains(&hwnd)
                    || desired.processes.contains(&pid)
                    || !seen_win.insert(hwnd)
                {
                    continue;
                }
                if let Err(e) = apply_to_window(hwnd, HideAction::UnhideWindow, payload_path) {
                    errors.push(format!("Unhide window {hwnd}: {}", e.0));
                }
            }
            UnhideOp::Tray(hwnd) => {
                if desired.trays.contains(&hwnd) || !seen_tray.insert(hwnd) {
                    continue;
                }
                if let Err(e) = apply_to_window(hwnd, HideAction::UnhideTrayIcon, payload_path) {
                    errors.push(format!("Unhide tray {hwnd}: {}", e.0));
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_with_process(pid: u32) -> HideSet {
        let mut set = HideSet::default();
        set.processes.insert(pid);
        set
    }

    #[test]
    fn removed_process_hide_is_released() {
        let mut applied = set_with_process(10);
        applied.processes.insert(20);
        let desired = set_with_process(20);
        let mut stale = pids_to_release(&applied, &desired);
        stale.sort_unstable();
        assert_eq!(stale, vec![10]);
    }

    #[test]
    fn already_applied_process_hide_is_not_sent_again() {
        let already = set_with_process(20);
        let mut desired = set_with_process(20);
        desired.processes.insert(30);
        let mut fresh = pids_to_hide(&desired, &already);
        fresh.sort_unstable();
        assert_eq!(fresh, vec![30]);
    }

    #[test]
    fn kept_process_hide_is_not_released() {
        let applied = set_with_process(20);
        let desired = set_with_process(20);
        assert!(pids_to_release(&applied, &desired).is_empty());
    }

    #[test]
    fn window_hide_covered_by_process_rule_is_not_released() {
        let mut desired = HideSet::default();
        desired.processes.insert(7);
        assert!(!should_unhide_window(5, 7, &desired));
    }

    #[test]
    fn window_hide_is_released_when_nothing_covers_it() {
        let desired = HideSet::default();
        assert!(should_unhide_window(5, 7, &desired));
    }

    #[test]
    fn window_still_requested_is_not_released() {
        let mut applied = HideSet::default();
        applied.windows.insert(5);
        let mut desired = HideSet::default();
        desired.windows.insert(5);
        assert!(hwnds_to_release(&applied.windows, &desired.windows).is_empty());
    }
}
