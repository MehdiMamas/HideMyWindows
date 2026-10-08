# Read-only window metadata recorder. Does not capture screen contents or titles.
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
public static class HmwWindowCheck {
 delegate bool Callback(IntPtr h, IntPtr context);
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 public class Window {
  public string Handle,Owner,Process,Class; public uint Pid,Affinity,Cloaked;
  public bool Visible,Minimized,AffinityReadable; public int AffinityError,CloakReadResult;
  public Rect Bounds; public long TransitionMarker,CloakMarker,SeenMarker,ObservedMarker,CloakResult;
 }
 [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback,IntPtr context);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr h,uint cmd);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect rect);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr h,StringBuilder text,int size);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr GetPropW(IntPtr h,string name);
 [DllImport("user32.dll",SetLastError=true)] static extern bool GetWindowDisplayAffinity(IntPtr h,out uint value);
 [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h,uint attr,out uint value,uint size);
 public static string ProcessName(IntPtr h) {
  uint pid; GetWindowThreadProcessId(h,out pid);
  try { return Process.GetProcessById((int)pid).ProcessName; } catch { return "unknown"; }
 }
 public static Window[] Read() {
  var result=new List<Window>();
  EnumWindows((h,context)=> {
   uint pid; GetWindowThreadProcessId(h,out pid);
   string name=ProcessName(h);
   long marker=GetPropW(h,"HideMyWindows.CaptureTransitions").ToInt64();
   long owned=GetPropW(h,"HideMyWindows.MinimizedCaptureCloak").ToInt64();
   uint affinity; bool readable=GetWindowDisplayAffinity(h,out affinity);
   int error=readable ? 0 : Marshal.GetLastWin32Error();
   if(name.IndexOf("whatsapp",StringComparison.OrdinalIgnoreCase)<0 && marker==0 && owned==0 && (!readable || affinity!=17)) return true;
   uint cloaked; int hr=DwmGetWindowAttribute(h,14,out cloaked,4);
   Rect rect; GetWindowRect(h,out rect);
   var cls=new StringBuilder(256); GetClassNameW(h,cls,cls.Capacity);
   result.Add(new Window { Handle=h.ToInt64().ToString("X"),Owner=GetWindow(h,4).ToInt64().ToString("X"),Process=name,Pid=pid,Class=cls.ToString(),
    Visible=IsWindowVisible(h),Minimized=IsIconic(h),Affinity=affinity,AffinityReadable=readable,AffinityError=error,
    Cloaked=cloaked,CloakReadResult=hr,Bounds=rect,TransitionMarker=marker,CloakMarker=owned,
    SeenMarker=GetPropW(h,"HideMyWindows.CaptureWasVisible").ToInt64(),
    ObservedMarker=GetPropW(h,"HideMyWindows.CapturePresentationObserved").ToInt64(),
    CloakResult=GetPropW(h,"HideMyWindows.CaptureCloakResult").ToInt64() });
   return true;
  },IntPtr.Zero);
  return result.ToArray();
 }
}
'@
Write-Host 'Records window state for 20 seconds; no screenshots or chat content.'
Read-Host 'Press Enter, switch to another app, then minimize protected WhatsApp' | Out-Null
$records = [System.Collections.Generic.List[object]]::new()
$watch = [System.Diagnostics.Stopwatch]::StartNew()
$previous = ''
while ($watch.Elapsed.TotalSeconds -lt 20) {
    $foreground = [HmwWindowCheck]::GetForegroundWindow()
    $state = [ordered]@{
        foregroundHandle = $foreground.ToInt64().ToString('X')
        foregroundProcess = [HmwWindowCheck]::ProcessName($foreground)
        windows = @([HmwWindowCheck]::Read())
    }
    $serialized = $state | ConvertTo-Json -Depth 6 -Compress
    if ($serialized -ne $previous) {
        $records.Add([ordered]@{ elapsedMs = $watch.ElapsedMilliseconds; state = $state })
        $previous = $serialized
    }
    Start-Sleep -Milliseconds 100
}
$destination = Join-Path $PSScriptRoot ('WhatsApp-window-state-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.json')
ConvertTo-Json -InputObject @($records.ToArray()) -Depth 8 | Set-Content -Encoding UTF8 -LiteralPath $destination
Write-Host "Saved: $destination"
