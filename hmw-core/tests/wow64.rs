//! Exercise the released x86 helper and payload against a real WOW64 app.
#![cfg(all(windows, target_arch = "x86_64"))]

use hmw_core::model::{HideAction, RuleComparator, RuleTarget, WindowRule};
use hmw_core::window::{is_capture_hidden, list_top_windows};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const FIXTURE: &str = r#"
Add-Type -ReferencedAssemblies System.dll,System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Collections.Concurrent;
using System.Threading.Tasks;
using System.Windows.Forms;
public static class Wow64Target {
 public static void Run() {
  if(IntPtr.Size != 4) throw new Exception("Fixture must be 32-bit");
  var commands = new ConcurrentQueue<string>();
  var main = new Form { Text = "WOW64 rule fixture", Width = 600, Height = 240 };
  Task.Run(() => { string s; while((s = Console.ReadLine()) != null) commands.Enqueue(s); });
  var timer = new Timer { Interval = 30 };
  timer.Tick += (s,e) => { string cmd; while(commands.TryDequeue(out cmd)) {
    if(cmd == "new") { var extra = new Form { Text = "WOW64 later window" }; extra.Show();
      Console.WriteLine("HMW_WINDOW " + extra.Handle.ToInt64()); Console.Out.Flush(); }
  }};
  main.Shown += (s,e) => { timer.Start();
    Console.WriteLine("HMW_WINDOW " + main.Handle.ToInt64()); Console.Out.Flush(); };
  Application.Run(main);
 }
}
'@
[Wow64Target]::Run()
"#;

struct Target {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
    hwnd: isize,
}
impl Target {
    fn start() -> Self {
        let powershell = PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("SysWOW64/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = Command::new(powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", FIXTURE])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line.starts_with("HMW_WINDOW ") && tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut target = Self {
            child,
            input,
            output,
            hwnd: 0,
        };
        target.hwnd = target.next_window();
        target
    }
    fn next_window(&self) -> isize {
        self.output
            .recv_timeout(Duration::from_secs(30))
            .expect("WOW64 fixture ready")
            .strip_prefix("HMW_WINDOW ")
            .unwrap()
            .parse()
            .unwrap()
    }
    fn new_window(&mut self) -> isize {
        writeln!(self.input, "new").unwrap();
        self.input.flush().unwrap();
        self.next_window()
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn eventually(hwnd: isize, hidden: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while is_capture_hidden(hwnd).unwrap() != hidden {
        assert!(
            Instant::now() < deadline,
            "capture state did not become {hidden}"
        );
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn x64_controller_hides_and_restores_x86_windows_and_rules_cover_later_windows() {
    let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("src-tauri/resources");
    let helper = resources.join("hmw-release-x86.exe");
    let payload = resources.join("hmw_payload_x86.dll");
    assert!(
        helper.is_file() && payload.is_file(),
        "build the x86 helper and payload first"
    );
    // Same name/layout as the embedded resources extracted by the installed app.
    let dir = std::env::temp_dir().join(format!("hmw-wow64-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let runtime_payload = dir.join("hmw_payload.dll");
    std::fs::copy(payload, &runtime_payload).unwrap();
    hmw_core::wow64::configure(helper.clone(), runtime_payload.clone());
    let mut target = Target::start();
    let pid = target.child.id();
    let handle = unsafe {
        windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
        .unwrap()
    };
    let handle = hmw_core::process::SafeHandle(handle);
    assert!(!hmw_core::process::is_process_64bit(handle.0).unwrap());
    let own_payload = resources
        .join("hmw_payload.dll")
        .to_string_lossy()
        .into_owned();
    hmw_core::hider::apply_to_window(target.hwnd, HideAction::HideWindow, &own_payload).unwrap();
    eventually(target.hwnd, true);
    hmw_core::hider::apply_to_window(target.hwnd, HideAction::UnhideWindow, &own_payload).unwrap();
    eventually(target.hwnd, false);
    hmw_core::hider::apply_to_window(target.hwnd, HideAction::HideTrayIcon, &own_payload).unwrap();
    hmw_core::hider::apply_to_window(target.hwnd, HideAction::UnhideTrayIcon, &own_payload)
        .unwrap();

    let rule = WindowRule {
        id: "wow64".into(),
        target: RuleTarget::ProcessId,
        comparator: RuleComparator::Equals,
        value: pid.to_string(),
        action: HideAction::HideProcessWindows,
        enabled: true,
        persistent: true,
    };
    let mut session = hmw_core::watcher::RuleSession::new();
    assert!(session
        .apply(std::slice::from_ref(&rule), &own_payload, false)
        .is_empty());
    eventually(target.hwnd, true);
    let later = target.new_window();
    eventually(later, true);
    let status = session.status(&[rule]);
    assert_eq!(
        (
            status.matched_windows,
            status.hidden_windows,
            status.unknown_windows
        ),
        (2, 2, 0)
    );
    assert!(session.apply(&[], &own_payload, false).is_empty());
    eventually(target.hwnd, false);
    eventually(later, false);
    std::thread::sleep(Duration::from_millis(400));
    eventually(later, false); // The payload worker must not re-hide after removal.

    // Uninstall cleanup still recognizes and releases the x86 payload.
    hmw_core::hider::apply_to_process(pid, HideAction::HideProcessWindows, &own_payload).unwrap();
    let released = Command::new(helper)
        .arg("--release-all")
        .arg("--no-elevate")
        .output()
        .unwrap();
    assert!(
        matches!(released.status.code(), Some(0 | 2)),
        "{}",
        String::from_utf8_lossy(&released.stderr)
    );
    eventually(target.hwnd, false);
    assert!(list_top_windows(true)
        .unwrap()
        .iter()
        .any(|w| w.hwnd == target.hwnd));
    drop(target);
    let _ = std::fs::remove_dir_all(dir);
}
