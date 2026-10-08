// Pre-show protection. Detours supplies architecture-specific trampolines;
// every policy decision stays here, outside DllMain / the loader lock.
#include <windows.h>
#include <tlhelp32.h>
#include <atomic>
#include <vector>
#include "detours.h"

static std::atomic<bool> enabled{false};
static INIT_ONCE once = INIT_ONCE_STATIC_INIT;
static CRITICAL_SECTION gate;
static DWORD install_error = ERROR_NOT_READY;
static const wchar_t* pending = L"HideMyWindows.PendingShow";
static const wchar_t* failure = L"HideMyWindows.ProtectionFailure";
static auto real_create_w = CreateWindowExW;
static auto real_create_a = CreateWindowExA;
static auto real_show = ShowWindow;
static auto real_show_async = ShowWindowAsync;
static auto real_pos = SetWindowPos;
static auto real_defer = DeferWindowPos;
static auto real_affinity = SetWindowDisplayAffinity;

static bool top_level(HWND hwnd) {
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    return pid == GetCurrentProcessId() && IsWindow(hwnd) &&
        !(GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CHILD) && GetParent(hwnd) != HWND_MESSAGE;
}

// Called before visibility, not after a timer observes a painted window.
static bool protect(HWND hwnd) {
    if (!enabled.load(std::memory_order_acquire) || !top_level(hwnd)) return true;
    EnterCriticalSection(&gate);
    bool ok = true;
    if (enabled.load(std::memory_order_relaxed)) {
        DWORD affinity = 0;
        ok = GetWindowDisplayAffinity(hwnd, &affinity) && affinity == WDA_EXCLUDEFROMCAPTURE;
        if (!ok) {
            ok = real_affinity(hwnd, WDA_EXCLUDEFROMCAPTURE) &&
                GetWindowDisplayAffinity(hwnd, &affinity) && affinity == WDA_EXCLUDEFROMCAPTURE;
        }
        if (ok) RemovePropW(hwnd, failure);
        else {
            DWORD error = GetLastError();
            SetPropW(hwnd, failure, reinterpret_cast<HANDLE>(static_cast<ULONG_PTR>(error ? error : ERROR_ACCESS_DENIED)));
        }
    }
    LeaveCriticalSection(&gate);
    return ok;
}

static void remember_show(HWND hwnd, int cmd) {
    SetPropW(hwnd, pending, reinterpret_cast<HANDLE>(static_cast<INT_PTR>(cmd + 1)));
    SetLastError(ERROR_ACCESS_DENIED);
}

static BOOL WINAPI hook_show(HWND hwnd, int cmd) {
    if (cmd != SW_HIDE && !protect(hwnd)) { remember_show(hwnd, cmd); return FALSE; }
    RemovePropW(hwnd, pending);
    return real_show(hwnd, cmd);
}
static BOOL WINAPI hook_show_async(HWND hwnd, int cmd) {
    if (cmd != SW_HIDE && !protect(hwnd)) { remember_show(hwnd, cmd); return FALSE; }
    RemovePropW(hwnd, pending);
    return real_show_async(hwnd, cmd);
}
static BOOL WINAPI hook_pos(HWND hwnd, HWND after, int x, int y, int cx, int cy, UINT flags) {
    if ((flags & SWP_SHOWWINDOW) && !protect(hwnd)) {
        remember_show(hwnd, SW_SHOWNOACTIVATE); return FALSE;
    }
    if (flags & (SWP_SHOWWINDOW | SWP_HIDEWINDOW)) RemovePropW(hwnd, pending);
    return real_pos(hwnd, after, x, y, cx, cy, flags);
}
static HDWP WINAPI hook_defer(HDWP batch, HWND hwnd, HWND after, int x, int y, int cx, int cy, UINT flags) {
    if ((flags & SWP_SHOWWINDOW) && !protect(hwnd)) {
        remember_show(hwnd, SW_SHOWNOACTIVATE); return nullptr;
    }
    if (flags & (SWP_SHOWWINDOW | SWP_HIDEWINDOW)) RemovePropW(hwnd, pending);
    return real_defer(batch, hwnd, after, x, y, cx, cy, flags);
}
static BOOL WINAPI hook_affinity(HWND hwnd, DWORD affinity) {
    EnterCriticalSection(&gate);
    if (enabled.load(std::memory_order_relaxed) && top_level(hwnd)) affinity = WDA_EXCLUDEFROMCAPTURE;
    BOOL result = real_affinity(hwnd, affinity);
    LeaveCriticalSection(&gate);
    return result;
}

static void finish_create(HWND hwnd, DWORD style, DWORD ex_style) {
    if (!hwnd || !top_level(hwnd)) return;
    // A WM_CREATE handler may have tried showing before creation completed.
    int cmd = 0;
    HANDLE queued = GetPropW(hwnd, pending);
    if (queued) cmd = static_cast<int>(reinterpret_cast<INT_PTR>(queued)) - 1;
    else if (style & WS_VISIBLE) {
        cmd = (style & WS_MINIMIZE) ? SW_SHOWMINNOACTIVE :
            (style & WS_MAXIMIZE) ? SW_SHOWMAXIMIZED :
            (ex_style & WS_EX_NOACTIVATE) ? SW_SHOWNOACTIVATE : SW_SHOW;
    } else return;
    if (protect(hwnd)) { RemovePropW(hwnd, pending); real_show(hwnd, cmd); }
    else remember_show(hwnd, cmd);
}
static HWND WINAPI hook_create_w(DWORD ex, LPCWSTR cls, LPCWSTR title, DWORD style,
    int x, int y, int w, int h, HWND parent, HMENU menu, HINSTANCE instance, LPVOID param) {
    bool guarded = enabled.load(std::memory_order_acquire) && !(style & WS_CHILD) && parent != HWND_MESSAGE;
    HWND hwnd = real_create_w(ex, cls, title, guarded ? style & ~WS_VISIBLE : style,
        x, y, w, h, parent, menu, instance, param);
    if (guarded) finish_create(hwnd, style, ex);
    return hwnd;
}
static HWND WINAPI hook_create_a(DWORD ex, LPCSTR cls, LPCSTR title, DWORD style,
    int x, int y, int w, int h, HWND parent, HMENU menu, HINSTANCE instance, LPVOID param) {
    bool guarded = enabled.load(std::memory_order_acquire) && !(style & WS_CHILD) && parent != HWND_MESSAGE;
    HWND hwnd = real_create_a(ex, cls, title, guarded ? style & ~WS_VISIBLE : style,
        x, y, w, h, parent, menu, instance, param);
    if (guarded) finish_create(hwnd, style, ex);
    return hwnd;
}

static BOOL CALLBACK install(PINIT_ONCE, PVOID, PVOID*) {
    InitializeCriticalSection(&gate);
    LONG error = DetourTransactionBegin();
    if (error != NO_ERROR) { install_error = error; return TRUE; }
    std::vector<HANDLE> threads;
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
    if (snapshot == INVALID_HANDLE_VALUE) error = GetLastError();
    else {
        THREADENTRY32 entry{}; entry.dwSize = sizeof(entry);
        if (Thread32First(snapshot, &entry)) do {
            if (entry.th32OwnerProcessID != GetCurrentProcessId() || entry.th32ThreadID == GetCurrentThreadId()) continue;
            HANDLE thread = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_SET_CONTEXT | THREAD_QUERY_INFORMATION, FALSE, entry.th32ThreadID);
            if (!thread) { if (GetLastError() != ERROR_INVALID_PARAMETER) error = GetLastError(); continue; }
            threads.push_back(thread);
            if (error == NO_ERROR) error = DetourUpdateThread(thread);
        } while (Thread32Next(snapshot, &entry));
        CloseHandle(snapshot);
    }
#define ATTACH(real, hook) if (error == NO_ERROR) error = DetourAttach(reinterpret_cast<PVOID*>(&real), reinterpret_cast<PVOID>(hook))
    ATTACH(real_create_w, hook_create_w);
    ATTACH(real_create_a, hook_create_a);
    ATTACH(real_show, hook_show);
    ATTACH(real_show_async, hook_show_async);
    ATTACH(real_pos, hook_pos);
    ATTACH(real_defer, hook_defer);
    ATTACH(real_affinity, hook_affinity);
#undef ATTACH
    if (error == NO_ERROR) error = DetourTransactionCommit();
    else DetourTransactionAbort();
    for (HANDLE thread : threads) CloseHandle(thread);
    install_error = error;
    return TRUE;
}

extern "C" DWORD HmwInstallHooks() {
    InitOnceExecuteOnce(&once, install, nullptr, nullptr);
    return install_error;
}
extern "C" void HmwEnableHooks(BOOL value) {
    if (install_error != NO_ERROR) return;
    EnterCriticalSection(&gate);
    enabled.store(value != FALSE, std::memory_order_release);
    LeaveCriticalSection(&gate);
}
extern "C" BOOL HmwProtectBeforeShow(HWND hwnd) { return protect(hwnd); }
extern "C" void HmwReplayPendingShow(HWND hwnd) {
    HANDLE queued = GetPropW(hwnd, pending);
    if (!queued) return;
    if (enabled.load(std::memory_order_acquire) && !protect(hwnd)) return;
    RemovePropW(hwnd, pending);
    RemovePropW(hwnd, failure);
    real_show(hwnd, static_cast<int>(reinterpret_cast<INT_PTR>(queued)) - 1);
}
static BOOL CALLBACK check_failure(HWND hwnd, LPARAM result) {
    if (top_level(hwnd)) {
        auto code = static_cast<DWORD>(reinterpret_cast<ULONG_PTR>(GetPropW(hwnd, failure)));
        if (code) { *reinterpret_cast<DWORD*>(result) = code; return FALSE; }
    }
    return TRUE;
}
extern "C" DWORD HmwHookFailure() {
    DWORD error = 0;
    EnumWindows(check_failure, reinterpret_cast<LPARAM>(&error));
    return error;
}

extern "C" void HmwRecoverProtection(HWND hwnd) {
    if (enabled.load(std::memory_order_acquire) && top_level(hwnd) &&
        (IsWindowVisible(hwnd) || GetPropW(hwnd, pending))) {
        if (protect(hwnd)) HmwReplayPendingShow(hwnd);
    }
}
