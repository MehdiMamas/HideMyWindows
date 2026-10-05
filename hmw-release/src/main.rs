//! Standalone cleanup for hides that outlive HideMyWindows.
//!
//! The installer runs `hidemywindows.exe --release-all`, which does this work
//! in-process and launches this binary only for the other architecture. Run it
//! directly after an uninstall that left windows hidden:
//!
//! ```text
//! hmw-release.exe --release-all
//! ```

fn main() {
    #[cfg(windows)]
    {
        let code = hmw_core::cleanup::run(hmw_core::cleanup::options_from_args(None));
        std::process::exit(code);
    }
    #[cfg(not(windows))]
    {
        eprintln!("HideMyWindows only runs on Windows");
        std::process::exit(1);
    }
}
