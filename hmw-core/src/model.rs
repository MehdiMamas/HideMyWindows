//! Shared, platform-independent data types (serializable for the UI and config).

use serde::{Deserialize, Serialize};

/// What a hide/unhide action does to a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HideAction {
    /// Hide every top-level window owned by the target process.
    HideProcessWindows,
    /// Hide a single window (by handle).
    HideWindow,
    /// Unhide every top-level window owned by the target process.
    UnhideProcessWindows,
    /// Unhide a single window (by handle).
    UnhideWindow,
    /// Remove the target's taskbar button.
    HideTrayIcon,
    /// Restore the target's taskbar button.
    UnhideTrayIcon,
}

/// What a window rule matches against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleTarget {
    WindowTitle,
    WindowClass,
    ProcessName,
    ProcessId,
}

/// How a window rule compares the target value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleComparator {
    Equals,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
}

/// An automatic window rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRule {
    #[serde(default = "crate::model::new_id")]
    pub id: String,
    pub target: RuleTarget,
    pub comparator: RuleComparator,
    pub value: String,
    pub action: HideAction,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub persistent: bool,
}

impl WindowRule {
    /// Does `value` satisfy this rule's comparator?
    pub fn matches(&self, value: &str) -> bool {
        match self.comparator {
            RuleComparator::Equals => value == self.value,
            RuleComparator::Contains => value.contains(&self.value),
            RuleComparator::StartsWith => value.starts_with(&self.value),
            RuleComparator::EndsWith => value.ends_with(&self.value),
            RuleComparator::Regex => regex::Regex::new(&self.value)
                .map(|re| re.is_match(value))
                .unwrap_or(false),
        }
    }
}

/// An entry that launches an app already hidden.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickLaunchEntry {
    #[serde(default = "crate::model::new_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub arguments: String,
}

/// Information about a running process (sent to the UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    #[serde(default)]
    pub path: String,
}

/// Generate a short unique id for list items (stable keys in the UI).
pub fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Mix in a per-process counter so ids created in the same nanosecond differ.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{c:x}")
}
