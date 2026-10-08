// Synchronous, same-architecture desktop CBT hook. Runs outside DllMain.
// Only window creation is intercepted; unrelated apps keep normal affinity.
#include <windows.h>
#include <commctrl.h>
#include <vector>
#include <string>

extern "C" int HmwGateDecision(const unsigned char*, DWORD, DWORD, const wchar_t*, const wchar_t*, const wchar_t*);
extern "C" void HmwEnsureGateWorker();
extern "C" DWORD HmwInstallHooks();
extern "C" BOOL HmwProtectBeforeShow(HWND);
extern "C" void HmwReplayPendingShow(HWND);

#if defined(_M_IX86) || defined(__i386__)
static const wchar_t* name = L"Local\\HideMyWindows.NormalGate.v1.x86";
static const wchar_t* lock_name = L"Local\\HideMyWindows.NormalGate.v1.x86.Lock";
#elif defined(_M_ARM64) || defined(__aarch64__)
static const wchar_t* name = L"Local\\HideMyWindows.NormalGate.v1.arm64";
static const wchar_t* lock_name = L"Local\\HideMyWindows.NormalGate.v1.arm64.Lock";
#else
static const wchar_t* name = L"Local\\HideMyWindows.NormalGate.v1.x64";
static const wchar_t* lock_name = L"Local\\HideMyWindows.NormalGate.v1.x64.Lock";
#endif
static const wchar_t* tracked = L"HideMyWindows.GateWindow";
static const wchar_t* owned = L"HideMyWindows.GateAffinity";
static const wchar_t* pending = L"HideMyWindows.PendingShow";
static const wchar_t* failure = L"HideMyWindows.ProtectionFailure";
struct SharedPolicy {
    volatile LONG sequence;
    DWORD owner, excluded, length;
    unsigned char bytes[65536];
};
static HANDLE writer_mapping = nullptr, writer_lock = nullptr;
static SharedPolicy* writer = nullptr;
static HHOOK hook = nullptr;

struct Reader {
    HANDLE mapping = nullptr, owner = nullptr;
    SharedPolicy* view = nullptr;
    DWORD owner_pid = 0;
    ~Reader() {
        if (view) UnmapViewOfFile(view);
        if (mapping) CloseHandle(mapping);
        if (owner) CloseHandle(owner);
    }
};
static thread_local Reader reader;

// Never wait for the controller while executing inside another app. A torn
// update is retried a bounded number of times, then the show is held for retry.
static int decide(HWND hwnd) {
    if (!reader.view) {
        reader.mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, name);
        if (!reader.mapping) return GetLastError() == ERROR_FILE_NOT_FOUND ? 0 : -1;
        reader.view = static_cast<SharedPolicy*>(MapViewOfFile(reader.mapping, FILE_MAP_READ, 0, 0, sizeof(SharedPolicy)));
        if (!reader.view) { CloseHandle(reader.mapping); reader.mapping = nullptr; return -1; }
    }
    auto view = reader.view;
    DWORD owner = view->owner;
    if (!owner || owner == GetCurrentProcessId() || view->excluded == GetCurrentProcessId()) return 0;
    if (reader.owner_pid != owner) {
        if (reader.owner) CloseHandle(reader.owner);
        reader.owner = OpenProcess(SYNCHRONIZE, FALSE, owner);
        reader.owner_pid = owner;
    }
    // A controller that exited must not leave apps permanently locally hidden.
    if (!reader.owner) return -1;
    DWORD state = WaitForSingleObject(reader.owner, 0);
    if (state == WAIT_OBJECT_0) return 0;
    if (state != WAIT_TIMEOUT) return -1;
    std::vector<unsigned char> bytes;
    bool consistent = false;
    for (int attempt = 0; attempt < 3; ++attempt) {
        LONG before = view->sequence;
        if (before & 1) continue;
        MemoryBarrier();
        DWORD length = view->length;
        if (length > sizeof(view->bytes)) return -1;
        bytes.assign(view->bytes, view->bytes + length);
        MemoryBarrier();
        if (before == view->sequence) { consistent = true; break; }
    }
    if (!consistent) return -1;
    wchar_t path[32768]{}, title[32768]{}, cls[256]{};
    if (!GetModuleFileNameW(nullptr, path, 32768)) return -1;
    const wchar_t* process = wcsrchr(path, L'\\');
    process = process ? process + 1 : path;
    GetWindowTextW(hwnd, title, 32768);
    GetClassNameW(hwnd, cls, 256);
    return HmwGateDecision(bytes.data(), static_cast<DWORD>(bytes.size()), GetCurrentProcessId(), process, title, cls);
}

extern "C" int HmwNormalGateDecision(HWND hwnd) {
    return GetPropW(hwnd, tracked) ? decide(hwnd) : 0;
}
extern "C" void HmwRememberGateAffinity(HWND hwnd, DWORD affinity) {
    if (GetPropW(hwnd, tracked) && !GetPropW(hwnd, owned))
        SetPropW(hwnd, owned, reinterpret_cast<HANDLE>(static_cast<ULONG_PTR>(affinity + 1)));
}
extern "C" void HmwRestoreGateAffinity(HWND hwnd) {
    HANDLE old = RemovePropW(hwnd, owned);
    if (old) SetWindowDisplayAffinity(hwnd, static_cast<DWORD>(reinterpret_cast<ULONG_PTR>(old) - 1));
}

static LRESULT CALLBACK window_proc(HWND hwnd, UINT message, WPARAM w, LPARAM l, UINT_PTR id, DWORD_PTR) {
    if (message == WM_NCDESTROY) {
        RemoveWindowSubclass(hwnd, window_proc, id);
        RemovePropW(hwnd, tracked);
        RemovePropW(hwnd, owned);
        RemovePropW(hwnd, pending);
        RemovePropW(hwnd, failure);
        return DefSubclassProc(hwnd, message, w, l);
    }
    if (message == WM_SHOWWINDOW && w) {
        // ShowWindow sends this before WM_WINDOWPOSCHANGING. Protect before
        // the app's own shown handler can draw or inspect the window.
        bool ok = HmwProtectBeforeShow(hwnd) != FALSE;
        if (!ok) SetPropW(hwnd, pending, reinterpret_cast<HANDLE>(SW_SHOWNOACTIVATE + 1));
        if (!ok || GetPropW(hwnd, owned)) HmwEnsureGateWorker();
        if (!ok) return 0;
    }
    if (message == WM_WINDOWPOSCHANGING) {
        auto pos = reinterpret_cast<WINDOWPOS*>(l);
        if ((pos->flags & SWP_SHOWWINDOW) && !HmwProtectBeforeShow(hwnd)) {
            pos->flags = (pos->flags & ~SWP_SHOWWINDOW) | SWP_HIDEWINDOW;
            SetPropW(hwnd, pending, reinterpret_cast<HANDLE>(SW_SHOWNOACTIVATE + 1));
        }
    }
    LRESULT result = DefSubclassProc(hwnd, message, w, l);
    if (message == WM_CREATE && result != -1) {
        // The CBT hook removed initial WS_VISIBLE. WM_CREATE has now completed
        // in the owning thread; affinity can be verified before replaying show.
        if (GetPropW(hwnd, pending)) {
            if (HmwProtectBeforeShow(hwnd)) HmwReplayPendingShow(hwnd);
            if (GetPropW(hwnd, owned) || GetPropW(hwnd, pending)) HmwEnsureGateWorker();
        }
    }
    return result;
}

static LRESULT CALLBACK gate_callback(int code, WPARAM w, LPARAM l) {
    if (code == HCBT_CREATEWND) {
        HWND hwnd = reinterpret_cast<HWND>(w);
        auto create = reinterpret_cast<CBT_CREATEWNDW*>(l)->lpcs;
        DWORD pid = 0; GetWindowThreadProcessId(hwnd, &pid);
        if (pid == GetCurrentProcessId() && !(create->style & WS_CHILD) && create->hwndParent != HWND_MESSAGE) {
            // Keep callback and worker code valid even after the global hook is
            // removed. DllMain itself never installs hooks or starts threads.
            HMODULE module = nullptr;
            GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                reinterpret_cast<LPCWSTR>(gate_callback), &module);
            if (SetPropW(hwnd, tracked, reinterpret_cast<HANDLE>(1)) &&
                SetWindowSubclass(hwnd, window_proc, 0x484d57, 0)) {
                if (create->style & WS_VISIBLE) {
                    int cmd = (create->style & WS_MINIMIZE) ? SW_SHOWMINNOACTIVE :
                        (create->style & WS_MAXIMIZE) ? SW_SHOWMAXIMIZED :
                        (create->dwExStyle & WS_EX_NOACTIVATE) ? SW_SHOWNOACTIVATE : SW_SHOW;
                    if (!SetPropW(hwnd, pending, reinterpret_cast<HANDLE>(cmd + 1))) return 1;
                    create->style &= ~WS_VISIBLE;
                }
            } else {
                RemovePropW(hwnd, tracked);
                SetPropW(hwnd, failure, reinterpret_cast<HANDLE>(ERROR_NOT_SUPPORTED));
                return 1; // Do not allow a window we could not intercept.
            }
        }
    }
    return CallNextHookEx(nullptr, code, w, l);
}

extern "C" DWORD HmwUpdateNormalGate(const unsigned char* bytes, DWORD length) {
    if (!writer || !bytes || length > sizeof(writer->bytes)) return ERROR_INVALID_PARAMETER;
    InterlockedIncrement(&writer->sequence);
    writer->length = length;
    memcpy(writer->bytes, bytes, length);
    MemoryBarrier();
    InterlockedIncrement(&writer->sequence);
    return ERROR_SUCCESS;
}
extern "C" void HmwStopNormalGate() {
    if (writer) writer->owner = 0;
    if (hook) { UnhookWindowsHookEx(hook); hook = nullptr; }
    if (writer) { UnmapViewOfFile(writer); writer = nullptr; }
    if (writer_mapping) { CloseHandle(writer_mapping); writer_mapping = nullptr; }
    if (writer_lock) { ReleaseMutex(writer_lock); CloseHandle(writer_lock); writer_lock = nullptr; }
}
extern "C" DWORD HmwStartNormalGate(const unsigned char* bytes, DWORD length, DWORD excluded_pid) {
    if (writer || !bytes || length > 65536) return ERROR_INVALID_PARAMETER;
    writer_lock = CreateMutexW(nullptr, FALSE, lock_name);
    if (!writer_lock) return GetLastError();
    DWORD wait = WaitForSingleObject(writer_lock, 0);
    if (wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED) {
        CloseHandle(writer_lock); writer_lock = nullptr; return ERROR_ALREADY_EXISTS;
    }
    writer_mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0, sizeof(SharedPolicy), name);
    if (!writer_mapping) { DWORD error = GetLastError(); HmwStopNormalGate(); return error; }
    writer = static_cast<SharedPolicy*>(MapViewOfFile(writer_mapping, FILE_MAP_WRITE, 0, 0, sizeof(SharedPolicy)));
    if (!writer) { DWORD error = GetLastError(); HmwStopNormalGate(); return error; }
    writer->owner = 0;
    writer->sequence = 0;
    writer->excluded = excluded_pid;
    HmwUpdateNormalGate(bytes, length);
    writer->owner = GetCurrentProcessId();
    HMODULE module = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCWSTR>(gate_callback), &module);
    hook = SetWindowsHookExW(WH_CBT, gate_callback, module, 0);
    if (!hook) { DWORD error = GetLastError(); HmwStopNormalGate(); return error; }
    return ERROR_SUCCESS;
}

static BOOL CALLBACK recover(HWND hwnd, LPARAM) {
    DWORD pid = 0; GetWindowThreadProcessId(hwnd, &pid);
    if (pid == GetCurrentProcessId() && GetPropW(hwnd, tracked) &&
        (GetPropW(hwnd, pending) || GetPropW(hwnd, owned))) {
        if (HmwProtectBeforeShow(hwnd)) HmwReplayPendingShow(hwnd);
    }
    return TRUE;
}
extern "C" void HmwRecoverNormalWindows() { EnumWindows(recover, 0); }
