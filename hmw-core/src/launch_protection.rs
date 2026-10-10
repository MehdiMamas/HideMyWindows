//! Own the global gates as one unit: pausing drops both, resuming creates both.
use crate::{Config, Result};
use serde::Serialize;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
