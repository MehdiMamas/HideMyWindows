//! A stable, observed result of window-rule processing.
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleStatus {
    pub matched_windows: usize,
    pub hidden_windows: usize,
    pub unknown_windows: usize,
    pub errors: Vec<String>,
}

impl RuleStatus {
    /// Hash-set traversal must not make unchanged errors look like new issues.
    pub fn with_errors(mut self, mut errors: Vec<String>) -> Self {
        errors.sort();
        errors.dedup();
        self.errors = errors;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_targets_and_reordered_errors_produce_one_stable_result() {
        let first = RuleStatus::default().with_errors(vec!["B".into(), "A".into(), "B".into()]);
        let second = RuleStatus::default().with_errors(vec!["A".into(), "B".into()]);
        assert_eq!(first, second);
        assert_eq!(first.errors.len(), 2);
        assert_ne!(first, RuleStatus::default());
    }
}
