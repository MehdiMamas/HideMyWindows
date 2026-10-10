//! Capture actual desktop pixels while protected x64/WOW64 windows are shown.
#![cfg(all(windows, target_arch = "x86_64"))]
use hmw_core::model::HideAction;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Gdi::*;

const FIXTURE: &str = r#"
Add-Type -ReferencedAssemblies System.dll,System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;
public static class FrameTarget {
 delegate IntPtr Proc(IntPtr h, uint m, IntPtr w, IntPtr l);
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Class {
  public uint style; public Proc proc; public int a,b; public IntPtr instance,icon,cursor,brush;
  public string menu,name;
 }
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern ushort RegisterClassW(ref Class c);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr CreateWindowExW(uint ex,string cls,string title,uint style,int x,int y,int w,int h,IntPtr p,IntPtr menu,IntPtr instance,IntPtr data);
 [DllImport("user32.dll", CharSet=CharSet.Ansi)] static extern IntPtr CreateWindowExA(uint ex,string cls,string title,uint style,int x,int y,int w,int h,IntPtr p,IntPtr menu,IntPtr instance,IntPtr data);
 [DllImport("user32.dll")] static extern IntPtr DefWindowProcW(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] static extern bool GetWindowDisplayAffinity(IntPtr h,out uint a);
 [DllImport("user32.dll")] static extern bool SetWindowDisplayAffinity(IntPtr h,uint a);
 [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h,int c);
 [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr h,int c);
 [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int hh,uint flags);
 [DllImport("user32.dll")] static extern IntPtr BeginDeferWindowPos(int n);
 [DllImport("user32.dll")] static extern IntPtr DeferWindowPos(IntPtr batch,IntPtr h,IntPtr after,int x,int y,int w,int hh,uint flags);
 [DllImport("user32.dll")] static extern bool EndDeferWindowPos(IntPtr batch);
 [DllImport("user32.dll")] static extern bool DestroyWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool UpdateWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
 [DllImport("gdi32.dll")] static extern IntPtr CreateSolidBrush(uint color);
 static bool expectHidden, bad;
 static Proc callback = (h,m,w,l) => {
  if(m==0x18 && w!=IntPtr.Zero && expectHidden) {
   uint a; if(!GetWindowDisplayAffinity(h,out a) || a!=0x11) bad=true;
  }
  return DefWindowProcW(h,m,w,l);
 };
 static IntPtr Make(int mode) {
  uint visible = (mode==0 || mode==1) ? 0x10000000u : 0;
  var h = mode==1 ? CreateWindowExA(0,"HmwFrameFixture","ANSI custom heading",0x80000000u|visible,100,100,320,200,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero)
   : CreateWindowExW(0,"HmwFrameFixture","Private capture test",0x80000000u|visible,100,100,320,200,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);
  if(h==IntPtr.Zero) throw new Exception("CreateWindow failed");
  if(mode==2) ShowWindow(h,4);
  if(mode==3) ShowWindowAsync(h,4);
  if(mode==4) SetWindowPos(h,IntPtr.Zero,0,0,0,0,0x53);
  if(mode==5) EndDeferWindowPos(DeferWindowPos(BeginDeferWindowPos(1),h,IntPtr.Zero,0,0,0,0,0x53));
  UpdateWindow(h); return h;
 }
 public static void Run() {
  var c=new Class { proc=callback,brush=CreateSolidBrush(0x00ff00ff),name="HmwFrameFixture" };
  if(RegisterClassW(ref c)==0) throw new Exception("RegisterClass failed");
  var commands=new ConcurrentQueue<string>();
  Task.Run(()=> { string s; while((s=Console.ReadLine())!=null) commands.Enqueue(s); });
  var form=new Form { Text="Frame fixture host",Left=700,Top=550,Width=120,Height=80,StartPosition=FormStartPosition.Manual };
  var timer=new System.Windows.Forms.Timer { Interval=10 };
  IntPtr displayed=IntPtr.Zero;
  timer.Tick+=(s,e)=> { string cmd; while(commands.TryDequeue(out cmd)) {
   if(cmd=="visible") { expectHidden=false; displayed=Make(2); }
   if(cmd=="blocked") {
    expectHidden=false; displayed=Make(2);
    Console.WriteLine(IsWindowVisible(displayed) ? "HMW_EXPOSED" : "HMW_BLOCKED"); Console.Out.Flush();
   }
   if(cmd=="destroy") { DestroyWindow(displayed); displayed=IntPtr.Zero; }
   if(cmd=="burst") {
    expectHidden=true; bad=false;
    for(int i=0;i<90;i++) {
     var h=Make(i%6);
     // An app attempting to reset affinity must not expose its content.
     SetWindowDisplayAffinity(h,0);
     var until=DateTime.UtcNow.AddMilliseconds(45);
     while(DateTime.UtcNow<until) { Application.DoEvents(); Thread.Sleep(1); }
     uint a; if(!GetWindowDisplayAffinity(h,out a) || a!=0x11) bad=true;
     DestroyWindow(h);
    }
    if(bad) throw new Exception("A window reached visibility without exclusion");
   }
   Console.WriteLine("HMW_ACK"); Console.Out.Flush();
  }};
  form.Shown+=(s,e)=> { timer.Start(); Console.WriteLine("HMW_READY"); Console.Out.Flush(); };
  Application.Run(form);
 }
}
'@
[FrameTarget]::Run()
"#;
struct Target {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
}
impl Target {
    fn start(x86: bool) -> Self {
        let path = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join(if x86 {
            "SysWOW64/WindowsPowerShell/v1.0/powershell.exe"
        } else {
            "System32/WindowsPowerShell/v1.0/powershell.exe"
        });
        let mut child = Command::new(path)
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
                if line.starts_with("HMW_") && tx.send(line).is_err() {
                    break;
                }
            }
        });
        let target = Self {
            child,
            input,
            output,
        };
        assert_eq!(
            target.output.recv_timeout(Duration::from_secs(30)).unwrap(),
            "HMW_READY"
        );
        target
    }
    fn send(&mut self, cmd: &str) {
        writeln!(self.input, "{cmd}").unwrap();
        self.input.flush().unwrap();
    }
    fn ack(&self) {
        assert_eq!(
            self.output.recv_timeout(Duration::from_secs(30)).unwrap(),
            "HMW_ACK"
        );
    }
    fn command(&mut self, cmd: &str) {
        self.send(cmd);
        self.ack();
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn payload_loaded(pid: u32) -> bool {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    unsafe {
        let snapshot = hmw_core::process::SafeHandle(
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid).unwrap(),
        );
        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        Module32FirstW(snapshot.0, &mut entry).unwrap();
        loop {
            let length = entry
                .szModule
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szModule.len());
            if String::from_utf16_lossy(&entry.szModule[..length])
                .to_lowercase()
                .starts_with("hmw_payload")
            {
                return true;
            }
            if Module32NextW(snapshot.0, &mut entry).is_err() {
                return false;
            }
        }
    }
}

// Unlike a final affinity query, this samples the actual desktop image during
// creation/showing. The visible control proves this capture detects the secret.
fn captured_secret() -> bool {
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, 200, 120);
        let previous = SelectObject(dc, bitmap);
        let copied = BitBlt(dc, 0, 0, 200, 120, screen, 140, 140, SRCCOPY).is_ok();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 200,
                biHeight: -120,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; 200 * 120 * 4];
        SelectObject(dc, previous);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            120,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        assert!(copied && lines == 120, "desktop capture failed");
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 245 && p[1] < 10 && p[2] > 245)
            .count()
            > 1000
    }
}
fn resources() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("src-tauri/resources")
}

#[test]
fn ordinary_launches_are_gated_without_a_watcher_or_quick_launch() {
    use hmw_core::{Config, RuleComparator, RuleTarget, WindowRule};
    let resources = resources();
    hmw_core::wow64::configure(
        resources.join("hmw-release-x86.exe"),
        resources.join("hmw_payload_x86.dll"),
    );
    let payload = resources
        .join("hmw_payload.dll")
        .to_string_lossy()
        .into_owned();
    let rule = WindowRule {
        id: "normal-gate".into(),
        target: RuleTarget::WindowClass,
        comparator: RuleComparator::Equals,
        value: "HmwFrameFixture".into(),
        action: HideAction::HideWindow,
        enabled: true,
        persistent: false,
    };
    let config_path =
        std::env::temp_dir().join(format!("hmw-normal-gate-{}.json", std::process::id()));
    let mut config = Config::default();
    config.save(&config_path).unwrap();
    let mut gate =
        hmw_core::normal_gate::WindowGate::start(&payload, &[], std::process::id()).unwrap();
    let x86_gate = hmw_core::wow64::start_gate(&config_path).unwrap();
    // Establish a visible control before introducing a rule.
    {
        let mut target = Target::start(false);
        target.command("visible");
        let until = Instant::now() + Duration::from_secs(3);
        while !captured_secret() {
            assert!(
                Instant::now() < until,
                "unmatched normal launch was not visible in capture"
            );
            std::thread::sleep(Duration::from_millis(15));
        }
        target.command("destroy");
    }
    config.window_rules = vec![rule];
    config.save(&config_path).unwrap();
    gate.update(&config.window_rules).unwrap();
    // The helper acknowledges startup; subsequent on-disk updates poll at 200 ms.
    std::thread::sleep(Duration::from_millis(450));
    for x86 in [false, true] {
        // An ordinary CreateProcess launch: no injection call and no watcher.
        let mut target = Target::start(x86);
        let stop = Arc::new(AtomicBool::new(false));
        let leaked = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(AtomicUsize::new(0));
        let (s, l, f) = (stop.clone(), leaked.clone(), frames.clone());
        let recorder = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                if captured_secret() {
                    l.store(true, Ordering::SeqCst);
                }
                f.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(8));
            }
        });
        target.command("burst");
        stop.store(true, Ordering::SeqCst);
        recorder.join().unwrap();
        assert!(frames.load(Ordering::SeqCst) > 50);
        assert!(
            !leaked.load(Ordering::SeqCst),
            "ordinary launch leaked (x86={x86})"
        );
    }
    // Simulate a corrupted/incomplete policy: a failed decision must keep the
    // window locally invisible, and it must appear once the policy is repaired.
    let mut target = Target::start(false);
    unsafe {
        use windows::core::{PCSTR, PCWSTR};
        use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
        let path: Vec<u16> = payload.encode_utf16().chain(Some(0)).collect();
        let module = LoadLibraryW(PCWSTR(path.as_ptr())).unwrap();
        let update = GetProcAddress(module, PCSTR(c"HmwGateUpdate".as_ptr().cast())).unwrap();
        let update: unsafe extern "system" fn(*const u8, u32) -> u32 = std::mem::transmute(update);
        assert_eq!(update(b"[".as_ptr(), 1), 0);
    }
    target.send("blocked");
    assert_eq!(
        target.output.recv_timeout(Duration::from_secs(10)).unwrap(),
        "HMW_BLOCKED"
    );
    target.ack();
    assert!(
        !hmw_core::normal_gate::blocked_windows().is_empty(),
        "blocked window failure was not reported"
    );
    gate.update(&[]).unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while !captured_secret() {
        assert!(
            Instant::now() < until,
            "repaired policy did not replay the blocked show"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
    target.command("destroy");
    // Stopping the controller releases normal-gate affinity in a surviving app.
    gate.update(&config.window_rules).unwrap();
    target.command("visible");
    assert!(!captured_secret());
    drop(gate);
    drop(x86_gate);
    let until = Instant::now() + Duration::from_secs(3);
    while !captured_secret() {
        assert!(
            Instant::now() < until,
            "stopping the gate did not restore recording"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
    target.command("destroy");
    // The same lifecycle used by the watcher must stop actual global DLL
    // loading, not merely publish an empty rule list, on BOTH architectures.
    let mut controller = hmw_core::launch_protection::GateController::new();
    let start = || {
        let native = hmw_core::normal_gate::WindowGate::start(
            &payload,
            &config.window_rules,
            std::process::id(),
        )?;
        let x86 = hmw_core::wow64::start_gate(&config_path)?;
        Ok((native, x86))
    };
    controller.reconcile(true, start);
    controller.reconcile(false, || unreachable!());
    for x86 in [false, true] {
        let mut fresh = Target::start(x86);
        fresh.command("visible");
        assert!(
            !payload_loaded(fresh.child.id()),
            "paused gate injected a DLL (x86={x86})"
        );
        let until = Instant::now() + Duration::from_secs(3);
        while !captured_secret() {
            assert!(
                Instant::now() < until,
                "paused launch was still blocked (x86={x86})"
            );
            std::thread::sleep(Duration::from_millis(15));
        }
        fresh.command("destroy");
    }
    controller.reconcile(true, start);
    for x86 in [false, true] {
        let mut fresh = Target::start(x86);
        fresh.command("visible");
        assert!(
            payload_loaded(fresh.child.id()),
            "resumed gate did not load (x86={x86})"
        );
        let window = hmw_core::window::list_top_windows(true)
            .unwrap()
            .into_iter()
            .find(|window| window.pid == fresh.child.id() && window.class == "HmwFrameFixture")
            .expect("resumed matching window must be locally visible");
        assert_eq!(
            hmw_core::window::capture_snapshot()
                .unwrap()
                .windows
                .get(&window.hwnd),
            Some(&hmw_core::model::CaptureStatus::Hidden)
        );
        assert!(
            !captured_secret(),
            "resumed window exposed capture content (x86={x86})"
        );
        fresh.command("destroy");
    }
    drop(controller);
    let _ = std::fs::remove_file(config_path);
}

#[test]
fn captured_frames_never_contain_new_protected_x64_or_x86_windows() {
    let resources = resources();
    hmw_core::wow64::configure(
        resources.join("hmw-release-x86.exe"),
        resources.join("hmw_payload_x86.dll"),
    );
    let payload = resources
        .join("hmw_payload.dll")
        .to_string_lossy()
        .into_owned();
    for x86 in [false, true] {
        let mut target = Target::start(x86);
        target.command("visible");
        let deadline = Instant::now() + Duration::from_secs(3);
        while !captured_secret() {
            assert!(
                Instant::now() < deadline,
                "visible control was not captured"
            );
            std::thread::sleep(Duration::from_millis(15));
        }
        target.command("destroy");
        hmw_core::hider::apply_to_process(
            target.child.id(),
            HideAction::HideProcessWindows,
            &payload,
        )
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let leaked = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(AtomicUsize::new(0));
        let (s, l, f) = (stop.clone(), leaked.clone(), frames.clone());
        let recorder = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                if captured_secret() {
                    l.store(true, Ordering::SeqCst);
                }
                f.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(8));
            }
        });
        target.send("burst");
        target.ack();
        stop.store(true, Ordering::SeqCst);
        recorder.join().unwrap();
        assert!(
            frames.load(Ordering::SeqCst) > 50,
            "too few captured frames"
        );
        assert!(
            !leaked.load(Ordering::SeqCst),
            "secret appeared in a captured frame (x86={x86})"
        );
        hmw_core::hider::apply_to_process(
            target.child.id(),
            HideAction::UnhideProcessWindows,
            &payload,
        )
        .unwrap();
        target.command("visible");
        let deadline = Instant::now() + Duration::from_secs(3);
        while !captured_secret() {
            assert!(Instant::now() < deadline, "unhide did not restore capture");
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

struct Launched(u32);
impl Drop for Launched {
    fn drop(&mut self) {
        unsafe {
            use windows::Win32::System::Threading::*;
            if let Ok(handle) = OpenProcess(
                PROCESS_TERMINATE | windows::Win32::System::Threading::PROCESS_SYNCHRONIZE,
                false,
                self.0,
            ) {
                let handle = hmw_core::process::SafeHandle(handle);
                let _ = TerminateProcess(handle.0, 0);
                WaitForSingleObject(handle.0, 5000);
            }
        }
    }
}

struct RestoreDll(PathBuf, PathBuf);
impl Drop for RestoreDll {
    fn drop(&mut self) {
        let _ = std::fs::rename(&self.1, &self.0);
    }
}

#[test]
fn quick_launch_arms_x64_and_x86_before_first_show_and_stops_on_failure() {
    let resources = resources();
    hmw_core::wow64::configure(
        resources.join("hmw-release-x86.exe"),
        resources.join("hmw_payload_x86.dll"),
    );
    let dir = std::env::temp_dir().join(format!("hmw-launch-frame-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("firstshow.cs");
    std::fs::write(&source,r#"
using System; using System.IO; using System.Drawing; using System.Runtime.InteropServices; using System.Windows.Forms;
class FirstShow {
 [DllImport("user32.dll")] static extern bool GetWindowDisplayAffinity(IntPtr h,out uint a);
 [STAThread] static void Main(string[] args) {
  var form=new Form {BackColor=Color.Magenta,FormBorderStyle=FormBorderStyle.None,
    StartPosition=FormStartPosition.Manual,Left=100,Top=100,Width=320,Height=200};
  form.Shown+=(s,e)=> { uint a;File.WriteAllText(args[0],GetWindowDisplayAffinity(form.Handle,out a) && a==17 ? "protected" : "unprotected"); };
  Application.Run(form);
 }
}
"#).unwrap();
    let compiler = PathBuf::from(std::env::var_os("WINDIR").unwrap())
        .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
    let payload = resources
        .join("hmw_payload.dll")
        .to_string_lossy()
        .into_owned();
    for arch in ["x64", "x86"] {
        let exe = dir.join(format!("firstshow-{arch}.exe"));
        let compiled = Command::new(&compiler)
            .arg("/nologo")
            .arg("/target:winexe")
            .arg(format!("/platform:{arch}"))
            .arg("/reference:System.Windows.Forms.dll")
            .arg("/reference:System.Drawing.dll")
            .arg(format!("/out:{}", exe.display()))
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stdout)
        );
        for attempt in 0..3 {
            let marker = dir.join(format!("{arch}-{attempt}.txt"));
            let arguments = format!("\"{}\"", marker.display());
            let stop = Arc::new(AtomicBool::new(false));
            let leaked = Arc::new(AtomicBool::new(false));
            let (s, l) = (stop.clone(), leaked.clone());
            let recorder = std::thread::spawn(move || {
                while !s.load(Ordering::SeqCst) {
                    if captured_secret() {
                        l.store(true, Ordering::SeqCst);
                    }
                    std::thread::sleep(Duration::from_millis(8));
                }
            });
            let launch =
                hmw_core::launch::launch_hidden(&exe.to_string_lossy(), &arguments, &payload);
            let pid = launch.expect("protected launch");
            let _process = Launched(pid);
            let deadline = Instant::now() + Duration::from_secs(20);
            while !marker.is_file() {
                assert!(Instant::now() < deadline, "first-show marker missing");
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(std::fs::read_to_string(marker).unwrap(), "protected");
            std::thread::sleep(Duration::from_millis(150));
            stop.store(true, Ordering::SeqCst);
            recorder.join().unwrap();
            assert!(
                !leaked.load(Ordering::SeqCst),
                "Quick Launch exposed a captured frame ({arch})"
            );
        }
        let marker = dir.join(format!("failed-{arch}.txt"));
        // WOW64 dispatch selects its bundled payload rather than the x64
        // argument, so temporarily make that test resource unavailable too.
        let restore = if arch == "x86" {
            let original = resources.join("hmw_payload_x86.dll");
            let backup = resources.join("hmw_payload_x86.disabled.dll");
            std::fs::rename(&original, &backup).unwrap();
            Some(RestoreDll(original, backup))
        } else {
            None
        };
        let failed = hmw_core::launch::launch_hidden(
            &exe.to_string_lossy(),
            &format!("\"{}\"", marker.display()),
            &dir.join("missing-payload.dll").to_string_lossy(),
        );
        drop(restore);
        assert!(failed.is_err(), "missing protection must stop launch");
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            !marker.exists(),
            "the unprotected primary thread was resumed"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}
