//! Rule decisions made inside a newly opened app, before its window is shown.
use crate::model::{HideAction, RuleTarget, WindowRule};

pub fn excludes_window(
    rules: &[WindowRule],
    pid: u32,
    process: &str,
    title: &str,
    class: &str,
) -> bool {
    rules.iter().any(|rule| {
        if !rule.enabled
            || rule.value.is_empty()
            || !matches!(
                rule.action,
                HideAction::HideProcessWindows | HideAction::HideWindow
            )
        {
            return false;
        }
        let pid_text = pid.to_string();
        let value = match rule.target {
            RuleTarget::ProcessName => process,
            RuleTarget::ProcessId => &pid_text,
            RuleTarget::WindowTitle => title,
            RuleTarget::WindowClass => class,
        };
        !value.is_empty() && rule.matches(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RuleComparator;

    fn rule(target: RuleTarget, value: &str, action: HideAction) -> WindowRule {
        WindowRule {
            id: "gate".into(),
            target,
            comparator: RuleComparator::Equals,
            value: value.into(),
            action,
            enabled: true,
            persistent: false,
        }
    }

    #[test]
    fn ordinary_launches_use_nonpersistent_process_rules_and_check_each_window() {
        let rules = [rule(
            RuleTarget::ProcessName,
            "private.exe",
            HideAction::HideProcessWindows,
        )];
        assert!(excludes_window(&rules, 5, "private.exe", "", ""));
        assert!(!excludes_window(&rules, 5, "safe.exe", "", ""));
    }

    #[test]
    fn hide_wins_over_conflicting_unhide_and_taskbar_actions_do_not_exclude_capture() {
        let mut rules = vec![
            rule(RuleTarget::ProcessId, "5", HideAction::UnhideProcessWindows),
            rule(RuleTarget::WindowClass, "Private", HideAction::HideTrayIcon),
        ];
        assert!(!excludes_window(&rules, 5, "app.exe", "", "Private"));
        rules.push(rule(RuleTarget::ProcessId, "5", HideAction::HideWindow));
        assert!(excludes_window(&rules, 5, "app.exe", "", "Private"));
    }

    #[test]
    fn title_rules_are_rechecked_when_a_title_becomes_available() {
        let mut rules = vec![rule(
            RuleTarget::WindowTitle,
            "Secret",
            HideAction::HideWindow,
        )];
        assert!(!excludes_window(&rules, 1, "app.exe", "", ""));
        assert!(excludes_window(&rules, 1, "app.exe", "Secret", ""));
        rules[0].enabled = false;
        assert!(!excludes_window(&rules, 1, "app.exe", "Secret", ""));
    }
}
