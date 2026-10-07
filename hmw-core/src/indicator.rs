//! Native, click-through red dots on capture-excluded target title bars.
//! All overlay HWNDs belong to this controller process and are themselves
//! excluded from capture before being shown. No target titles are modified.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateEllipticRgn, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetDC,
    GetMonitorInfoW, GetStockObject, GetTextExtentPoint32W, MonitorFromWindow, ReleaseDC,
    SelectObject, SetDCBrushColor, SetWindowRgn, DC_BRUSH, DEFAULT_GUI_FONT, HBRUSH, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::indicator_position::{place, DotRect};
use crate::window::{is_capture_hidden, list_top_windows, set_capture_hidden};
use crate::{Error, Result};

pub const CLASS_NAME: &str = "HideMyWindows.CaptureIndicator";
const CLASS_WIDE: windows::core::PCWSTR = w!("HideMyWindows.CaptureIndicator");
static STARTED: AtomicBool = AtomicBool::new(false);
static REGISTERED: OnceLock<std::result::Result<(), String>> = OnceLock::new();

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut paint);
            SetDCBrushColor(dc, COLORREF(0x000000ff));
            FillRect(dc, &paint.rcPaint, HBRUSH(GetStockObject(DC_BRUSH).0));
            let _ = EndPaint(hwnd, &paint);
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn register_class() -> Result<()> {
    REGISTERED
        .get_or_init(|| unsafe {
            let brush = CreateSolidBrush(COLORREF(0x000000ff));
            if brush.is_invalid() {
                return Err("Could not create indicator brush".into());
            }
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: HINSTANCE(GetModuleHandleW(None).map_err(|e| e.to_string())?.0),
                hbrBackground: brush,
                lpszClassName: CLASS_WIDE,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                let _ = DeleteObject(brush);
                return Err(windows::core::Error::from_win32().to_string());
            }
            // The class and its single shared brush live for the controller's lifetime.
            Ok(())
        })
        .clone()
        .map_err(Error)
}

fn title_width(title: &str, dpi: u32) -> i32 {
    unsafe {
        let dc = GetDC(None);
        if dc.is_invalid() {
            return 0;
        }
        let old = SelectObject(dc, GetStockObject(DEFAULT_GUI_FONT));
        let mut size = SIZE::default();
        let text: Vec<u16> = title.encode_utf16().collect();
        let _ = GetTextExtentPoint32W(dc, &text, &mut size);
        let _ = SelectObject(dc, old);
        ReleaseDC(None, dc);
        size.cx * dpi.max(96) as i32 / 96
    }
}

fn target_rect(hwnd: HWND, title: &str) -> Option<DotRect> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, 4).ok()?;
        if cloaked != 0 || !is_capture_hidden(hwnd.0 as isize).ok()? {
            return None;
        }
        let mut frame = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame as *mut _ as _,
            std::mem::size_of::<RECT>() as u32,
        )
        .ok()?;
        let native = (GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0;
        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut caption = frame;
        if native {
            let mut info = TITLEBARINFO {
                cbSize: std::mem::size_of::<TITLEBARINFO>() as u32,
                ..Default::default()
            };
            GetTitleBarInfo(hwnd, &mut info).ok()?;
            caption = info.rcTitleBar;
        } else {
            // Custom-drawn title bars, like hardware-monitor windows. Do not
            // put a dot into borderless fullscreen content or toast popups.
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            )
            .as_bool()
                && frame.left <= monitor.rcMonitor.left
                && frame.top <= monitor.rcMonitor.top
                && frame.right >= monitor.rcMonitor.right
                && frame.bottom >= monitor.rcMonitor.bottom
            {
                return None;
            }
            caption.bottom = caption.top + (24 * dpi as i32 / 96).min(frame.bottom - frame.top);
        }
        place(
            (caption.left, caption.top, caption.right, caption.bottom),
            dpi,
            title_width(title, dpi),
            native,
        )
    }
}

struct Marker {
    hwnd: HWND,
    rect: Option<DotRect>,
}

impl Drop for Marker {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

impl Marker {
    fn new(owner: HWND) -> Result<Self> {
        register_class()?;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                CLASS_WIDE,
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                owner,
                None,
                HINSTANCE(GetModuleHandleW(None)?.0),
                None,
            )?
        };
        let marker = Self { hwnd, rect: None };
        unsafe {
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
        }
        set_capture_hidden(hwnd.0 as isize, true)?;
        if !is_capture_hidden(hwnd.0 as isize)? {
            return Err(Error("Indicator capture exclusion was not applied".into()));
        }
        Ok(marker)
    }

    fn position(&mut self, owner: HWND, rect: DotRect) -> Result<()> {
        unsafe {
            // Place directly above its owner, beneath any covering windows.
            let above = GetWindow(owner, GW_HWNDPREV).unwrap_or(HWND_TOP);
            let same_z = above == self.hwnd;
            if self.rect == Some(rect)
                && same_z
                && IsWindowVisible(self.hwnd).as_bool()
                && is_capture_hidden(self.hwnd.0 as isize).unwrap_or(false)
            {
                return Ok(());
            }
            if !is_capture_hidden(self.hwnd.0 as isize).unwrap_or(false) {
                set_capture_hidden(self.hwnd.0 as isize, true)?;
                if !is_capture_hidden(self.hwnd.0 as isize)? {
                    return Err(Error("Indicator capture exclusion was not applied".into()));
                }
            }
            if self.rect.map(|r| r.size) != Some(rect.size) {
                let region = CreateEllipticRgn(0, 0, rect.size, rect.size);
                if region.is_invalid() {
                    return Err(Error("Could not shape indicator".into()));
                }
                if SetWindowRgn(self.hwnd, region, true) == 0 {
                    let _ = DeleteObject(region);
                    return Err(Error("Could not shape indicator".into()));
                }
                // Windows owns the region after successful SetWindowRgn.
            }
            let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW;
            if same_z {
                flags |= SWP_NOZORDER;
            }
            SetWindowPos(
                self.hwnd, above, rect.x, rect.y, rect.size, rect.size, flags,
            )?;
            self.rect = Some(rect);
        }
        Ok(())
    }
}

#[derive(Default)]
struct Indicators {
    markers: HashMap<isize, Marker>,
}

impl Indicators {
    fn scan(&mut self) {
        let Ok(windows) = list_top_windows(true) else {
            self.markers.clear();
            return;
        };
        let mut eligible = HashSet::new();
        for window in windows {
            if window.title.is_empty()
                || window.class == CLASS_NAME
                || matches!(
                    window.class.as_str(),
                    "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
                )
                || (window.class == "Windows.UI.Core.CoreWindow"
                    && crate::process::process_name(window.pid)
                        .is_some_and(|name| name.eq_ignore_ascii_case("ShellExperienceHost.exe")))
            {
                continue;
            }
            let owner = HWND(window.hwnd as *mut _);
            let Some(rect) = target_rect(owner, &window.title) else {
                continue;
            };
            eligible.insert(window.hwnd);
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.markers.entry(window.hwnd)
            {
                if let Ok(marker) = Marker::new(owner) {
                    entry.insert(marker);
                }
            }
            if let Some(marker) = self.markers.get_mut(&window.hwnd) {
                if marker.position(owner, rect).is_err() {
                    self.markers.remove(&window.hwnd);
                }
            }
        }
        self.markers.retain(|hwnd, _| eligible.contains(hwnd));
    }

    fn follow(&mut self) {
        self.markers.retain(|owner, marker| {
            let hwnd = HWND(*owner as *mut _);
            let title = crate::window::window_title(hwnd);
            let Some(rect) = target_rect(hwnd, &title) else {
                return false;
            };
            marker.position(hwnd, rect).is_ok()
        });
    }
}

/// Start once per controller. Win32 destroys these controller-owned dots when
/// HideMyWindows exits; the existing payload's protection is left untouched.
pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let mut indicators = Indicators::default();
        let mut scan_at = Instant::now();
        loop {
            unsafe {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    if message.message == WM_QUIT {
                        return;
                    }
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if Instant::now() >= scan_at {
                indicators.scan();
                scan_at = Instant::now() + Duration::from_millis(250);
            }
            indicators.follow();
            std::thread::sleep(Duration::from_millis(16));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, GetRgnBox, GetWindowRgn};

    // A real, separately owned WinForms window. Its UI thread pumps messages
    // while a background reader queues commands; no foreign UI thread is blocked.
    const FIXTURE: &str = r#"
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Windows.Forms;
public static class HiddenTargetFixture {
 [DllImport("user32.dll")] static extern bool SetWindowDisplayAffinity(IntPtr h, uint a);
 public static void Run(bool custom) {
  var commands = new ConcurrentQueue<string>();
  var form = new Form { Text = "Hidden app title test", Width = 640, Height = 300,
    StartPosition = FormStartPosition.Manual, Left = 100, Top = 100 };
  if(custom) { form.FormBorderStyle = FormBorderStyle.None;
    form.Controls.Add(new Label { Text = form.Text, Dock = DockStyle.Top, Height = 24 }); }
  Task.Run(() => { string s; while((s = Console.ReadLine()) != null) commands.Enqueue(s); });
  var timer = new Timer { Interval = 30 };
  timer.Tick += (s,e) => { string cmd; while(commands.TryDequeue(out cmd)) {
    if(cmd == "hide") SetWindowDisplayAffinity(form.Handle, 0x11);
    if(cmd == "unhide") SetWindowDisplayAffinity(form.Handle, 0);
    if(cmd == "move") { form.Left += 60; form.Top += 40; }
    if(cmd == "minimize") form.WindowState = FormWindowState.Minimized;
    if(cmd == "restore") form.WindowState = FormWindowState.Normal;
    if(cmd == "quit") form.Close();
    Console.WriteLine("HMW_ACK"); Console.Out.Flush();
  }};
  form.Shown += (s,e) => {
    if(!SetWindowDisplayAffinity(form.Handle, 0x11)) throw new Exception("Capture exclusion failed");
    form.Activate(); timer.Start();
    Console.WriteLine("HMW_WINDOW " + form.Handle.ToInt64()); Console.Out.Flush();
  };
  Application.Run(form); timer.Dispose(); form.Dispose();
 }
}
'@
[HiddenTargetFixture]::Run($env:HMW_CUSTOM_CAPTION -eq '1')
"#;

    struct Target {
        child: Child,
        input: ChildStdin,
        output: Receiver<String>,
        hwnd: HWND,
    }
    impl Drop for Target {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
    impl Target {
        fn new(custom: bool) -> Self {
            let mut child = Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", FIXTURE])
                .env("HMW_CUSTOM_CAPTION", if custom { "1" } else { "0" })
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("start separate-process window fixture");
            let input = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let (tx, output) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout)
                    .lines()
                    .map_while(std::result::Result::ok)
                {
                    if line.starts_with("HMW_") && tx.send(line).is_err() {
                        break;
                    }
                }
            });
            let mut target = Self {
                child,
                input,
                output,
                hwnd: HWND::default(),
            };
            let message = target
                .output
                .recv_timeout(Duration::from_secs(30))
                .expect("fixture HWND");
            let hwnd = message
                .strip_prefix("HMW_WINDOW ")
                .expect("fixture ready")
                .parse::<isize>()
                .unwrap();
            target.hwnd = HWND(hwnd as *mut _);
            target
        }
        fn command(&mut self, command: &str) {
            writeln!(self.input, "{command}").unwrap();
            self.input.flush().unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match self.output.recv_timeout(Duration::from_millis(10)) {
                    Ok(message) => {
                        assert_eq!(message, "HMW_ACK");
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(error) => panic!("fixture command failed: {error}"),
                }
                // Owned windows on another UI thread can receive synchronous
                // messages during move/minimize/close. Match the service's pump.
                unsafe {
                    let mut message = MSG::default();
                    while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "fixture command timed out: {command}"
                );
            }
        }
    }

    fn bounds(hwnd: HWND) -> RECT {
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut rect).unwrap();
        }
        rect
    }

    #[test]
    fn title_dot_on_another_apps_native_heading_moves_and_disappears_on_unhide() {
        let mut target = Target::new(false);
        let mut indicators = Indicators::default();
        let owner = target.hwnd.0 as isize;
        let foreground = unsafe { GetForegroundWindow() };
        indicators.scan();
        let dot = indicators
            .markers
            .get(&owner)
            .expect("red dot on hidden target")
            .hwnd;
        assert!(
            is_capture_hidden(dot.0 as isize).unwrap(),
            "dot must be excluded from capture"
        );
        assert_eq!(unsafe { GetWindow(dot, GW_OWNER).unwrap() }, target.hwnd);
        assert_eq!(
            unsafe { GetForegroundWindow() },
            foreground,
            "dot must not steal focus"
        );
        assert_eq!(
            crate::window::window_title(target.hwnd),
            "Hidden app title test"
        );
        let ex = unsafe { GetWindowLongW(dot, GWL_EXSTYLE) } as u32;
        assert_eq!(ex & WS_EX_TRANSPARENT.0, WS_EX_TRANSPARENT.0);
        assert_eq!(ex & WS_EX_NOACTIVATE.0, WS_EX_NOACTIVATE.0);
        assert_eq!(
            unsafe { SendMessageW(dot, WM_NCHITTEST, WPARAM(0), LPARAM(0)) },
            LRESULT(HTTRANSPARENT as isize)
        );
        let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
        unsafe {
            assert!(GetWindowRgn(dot, region).0 > 0);
        }
        let mut region_rect = RECT::default();
        unsafe {
            GetRgnBox(region, &mut region_rect);
            let _ = DeleteObject(region);
        }
        assert_eq!(
            region_rect.right - region_rect.left,
            indicators.markers[&owner].rect.unwrap().size
        );
        let before = bounds(dot);
        target.command("move");
        indicators.follow();
        let after = bounds(dot);
        assert_eq!((after.left - before.left, after.top - before.top), (60, 40));
        let cover = Marker {
            hwnd: unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("Covering window"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    100,
                    100,
                    800,
                    400,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap()
            },
            rect: None,
        };
        unsafe {
            SetWindowPos(
                cover.hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .unwrap();
        }
        indicators.follow();
        assert_eq!(
            unsafe { GetWindow(dot, GW_HWNDPREV).unwrap() },
            cover.hwnd,
            "the dot must stay underneath a covering window"
        );
        drop(cover);
        assert!(
            !list_top_windows(true)
                .unwrap()
                .iter()
                .any(|w| w.hwnd == dot.0 as isize),
            "dots must not appear as targets"
        );
        target.command("unhide");
        indicators.follow();
        assert!(!indicators.markers.contains_key(&owner));
        assert!(!unsafe { IsWindow(dot) }.as_bool());
    }

    #[test]
    fn custom_heading_dot_hides_on_minimize_returns_on_restore_and_closes_with_target() {
        let mut target = Target::new(true);
        let owner = target.hwnd.0 as isize;
        let mut indicators = Indicators::default();
        indicators.scan();
        let dot = indicators
            .markers
            .get(&owner)
            .expect("dot on custom heading")
            .hwnd;
        let target_box = bounds(target.hwnd);
        let dot_box = bounds(dot);
        assert!(dot_box.top >= target_box.top && dot_box.bottom <= target_box.top + 24);
        target.command("minimize");
        indicators.follow();
        assert!(!indicators.markers.contains_key(&owner));
        target.command("restore");
        indicators.scan();
        assert!(indicators.markers.contains_key(&owner));
        let dot = indicators.markers[&owner].hwnd;
        target.command("quit");
        indicators.follow();
        assert!(!indicators.markers.contains_key(&owner));
        assert!(!unsafe { IsWindow(dot) }.as_bool());
    }

    #[test]
    fn unhide_controller_does_not_unhide_its_target_markers_and_shutdown_cleans_them_up() {
        let _target = Target::new(false);
        let owner = _target.hwnd.0 as isize;
        let mut indicators = Indicators::default();
        indicators.scan();
        let dot = indicators.markers[&owner].hwnd;
        crate::hider::apply_to_process(
            unsafe { windows::Win32::System::Threading::GetCurrentProcessId() },
            crate::model::HideAction::UnhideProcessWindows,
            "unused for controller",
        )
        .unwrap();
        assert!(is_capture_hidden(dot.0 as isize).unwrap());
        drop(indicators);
        assert!(!unsafe { IsWindow(dot) }.as_bool());
    }
}
