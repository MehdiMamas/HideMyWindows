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
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.first().is_some_and(|arg| arg == "--normal-gate") {
            match hmw_core::wow64::run_gate(&args[1..]) {
                Ok(()) => std::process::exit(0),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
        }
        if args.first().is_some_and(|arg| arg == "--call-export") {
            match hmw_core::wow64::run_call(&args[1..]) {
                Ok(()) => std::process::exit(0),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
        }
        let code = hmw_core::cleanup::run(hmw_core::cleanup::options_from_args(None));
        std::process::exit(code);
    }
    #[cfg(not(windows))]
    {
        eprintln!("HideMyWindows only runs on Windows");
        std::process::exit(1);
    }
}
