//! Own the global gates as one unit: pausing drops both, resuming creates both.
use crate::{Config, Result};
use serde::Serialize;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayProtectionAction {
    Toggle,
    Pause(u64),
    Resume,
}

impl TrayProtectionAction {
    pub fn from_menu_id(id: &str) -> Option<Self> {
        match id {
            "protection-toggle" => Some(Self::Toggle),
            "protection-pause-15" => Some(Self::Pause(15)),
            "protection-pause-30" => Some(Self::Pause(30)),
            "protection-pause-60" => Some(Self::Pause(60)),
            "protection-pause-120" => Some(Self::Pause(120)),
            "protection-resume" => Some(Self::Resume),
            _ => None,
        }
    }

    /// Change only launch protection; unrelated settings and rules are retained.
    /// A stale pause click after disabling must not re-enable protection.
    pub fn apply(self, config: &mut Config, now: u64) -> bool {
        match self {
            Self::Toggle => {
                config.normal_launch_protection = !config.normal_launch_protection;
                config.normal_launch_pause_until_ms = 0;
            }
            Self::Pause(minutes) if config.normal_launch_protection => {
                config.normal_launch_pause_until_ms =
                    now.saturating_add(minutes.saturating_mul(60_000));
            }
            Self::Pause(_) => return false,
            Self::Resume => {
                config.normal_launch_protection = true;
                config.normal_launch_pause_until_ms = 0;
            }
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayProtectionState {
    pub enabled: bool,
    pub resume_enabled: bool,
    pub phase: &'static str,
    pub remaining_minutes: u64,
}

impl TrayProtectionState {
    pub fn new(config: &Config, applied: &LaunchProtectionStatus, now: u64) -> Self {
        let remaining_minutes = applied.pause_until_ms.saturating_sub(now).div_ceil(60_000);
        Self {
            enabled: config.normal_launch_protection,
            resume_enabled: config.normal_launch_protection
                && config.normal_launch_pause_until_ms > now,
            phase: if applied.phase == "paused" && remaining_minutes == 0 {
                "starting"
            } else {
                applied.phase
            },
            remaining_minutes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProtectionStatus {
    pub phase: &'static str,
    pub pause_until_ms: u64,
    pub error: Option<String>,
}

impl Default for LaunchProtectionStatus {
    fn default() -> Self {
        Self {
            phase: if cfg!(windows) {
                "starting"
            } else {
                "unsupported"
            },
            pause_until_ms: 0,
            error: None,
        }
    }
}

pub struct GateController<T> {
    gate: Option<Result<T>>,
}

impl<T> Default for GateController<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GateController<T> {
    pub fn new() -> Self {
        Self { gate: None }
    }

    /// A failed start is retained until the next off/on transition rather than
    /// repeatedly attempting installation on every watcher tick.
    pub fn reconcile(&mut self, active: bool, start: impl FnOnce() -> Result<T>) {
        if !active {
            self.gate = None;
        } else if self.gate.is_none() {
            self.gate = Some(start());
        }
    }

    pub fn gate_mut(&mut self) -> Option<&mut Result<T>> {
        self.gate.as_mut()
    }

    pub fn status(
        &self,
        config: &Config,
        now: u64,
        error: Option<String>,
    ) -> LaunchProtectionStatus {
        let phase = if !config.normal_launch_protection {
            "off"
        } else if !config.normal_launch_active_at(now) {
            "paused"
        } else if error.is_some() {
            "error"
        } else if matches!(self.gate, Some(Ok(_))) {
            "active"
        } else {
            "starting"
        };
        LaunchProtectionStatus {
            phase,
            pause_until_ms: if phase == "paused" {
                config.normal_launch_pause_until_ms
            } else {
                0
            },
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Gate(Rc<Cell<usize>>);
    impl Drop for Gate {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn tray_actions_share_the_saved_pause_and_enable_policy() {
        let mut cfg = Config {
            process_poll_interval_ms: 321,
            ..Config::default()
        };
        let now = 1_000;
        for minutes in [15, 30, 60, 120] {
            let action =
                TrayProtectionAction::from_menu_id(&format!("protection-pause-{minutes}")).unwrap();
            assert!(action.apply(&mut cfg, now));
            assert_eq!(cfg.normal_launch_pause_until_ms, now + minutes * 60_000);
            assert!(!cfg.normal_launch_active_at(now));
            assert_eq!(cfg.process_poll_interval_ms, 321);
        }
        TrayProtectionAction::Toggle.apply(&mut cfg, now);
        assert!(!cfg.normal_launch_protection);
        assert_eq!(cfg.normal_launch_pause_until_ms, 0);
        assert!(!TrayProtectionAction::Pause(30).apply(&mut cfg, now));
        TrayProtectionAction::Toggle.apply(&mut cfg, now);
        assert!(cfg.normal_launch_active_at(now));
        TrayProtectionAction::Pause(30).apply(&mut cfg, now);
        TrayProtectionAction::Resume.apply(&mut cfg, now);
        assert!(cfg.normal_launch_active_at(now));
        assert_eq!(cfg.normal_launch_pause_until_ms, 0);
        assert!(TrayProtectionAction::from_menu_id("quit").is_none());
        assert!(TrayProtectionAction::from_menu_id("protection-pause-0").is_none());
    }

    #[test]
    fn tray_reports_applied_state_and_waits_for_confirmed_resume() {
        let cfg = Config {
            normal_launch_pause_until_ms: 120_000,
            ..Config::default()
        };
        let mut applied = LaunchProtectionStatus {
            phase: "active",
            pause_until_ms: 0,
            error: None,
        };
        assert_eq!(TrayProtectionState::new(&cfg, &applied, 0).phase, "active");
        applied.phase = "paused";
        applied.pause_until_ms = 120_000;
        let state = TrayProtectionState::new(&cfg, &applied, 59_999);
        assert_eq!(state.remaining_minutes, 2);
        assert!(state.resume_enabled);
        let expired = TrayProtectionState::new(&cfg, &applied, 120_000);
        assert_eq!(expired.phase, "starting");
        assert!(!expired.resume_enabled);
        applied.phase = "active";
        assert_eq!(
            TrayProtectionState::new(&cfg, &applied, 120_000).phase,
            "active"
        );
    }

    #[test]
    fn pause_drops_both_gates_and_expiry_reinstalls_them_once() {
        let native = Rc::new(Cell::new(0));
        let x86 = Rc::new(Cell::new(0));
        let starts = Cell::new(0);
        let start = || {
            starts.set(starts.get() + 1);
            Ok((Gate(native.clone()), Gate(x86.clone())))
        };
        let mut controller = GateController::new();
        let mut cfg = Config::default();
        controller.reconcile(cfg.normal_launch_active_at(10), start);
        controller.reconcile(true, start);
        assert_eq!(starts.get(), 1);
        cfg.normal_launch_pause_until_ms = 100;
        controller.reconcile(cfg.normal_launch_active_at(99), start);
        assert_eq!((native.get(), x86.get()), (1, 1));
        assert_eq!(controller.status(&cfg, 99, None).phase, "paused");
        controller.reconcile(cfg.normal_launch_active_at(100), start);
        assert_eq!(starts.get(), 2);
        assert_eq!(controller.status(&cfg, 100, None).phase, "active");
        cfg.normal_launch_protection = false;
        controller.reconcile(cfg.normal_launch_active_at(101), start);
        assert_eq!((native.get(), x86.get()), (2, 2));
        assert_eq!(controller.status(&cfg, 101, None).phase, "off");
    }

    #[test]
    fn starting_paused_never_installs_a_gate_and_resume_clears_a_failed_start() {
        let mut controller = GateController::<()>::new();
        controller.reconcile(false, || panic!("paused startup must not install hooks"));
        controller.reconcile(true, || Err(crate::Error("blocked".into())));
        controller.reconcile(true, || panic!("failed starts must not spin"));
        assert!(controller.gate_mut().unwrap().is_err());
        controller.reconcile(false, || unreachable!());
        controller.reconcile(true, || Ok(()));
        assert!(controller.gate_mut().unwrap().is_ok());
    }
}
