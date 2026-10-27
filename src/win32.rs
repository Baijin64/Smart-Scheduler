#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::c_void;

pub type HANDLE = *mut c_void;
pub type HWND = *mut c_void;
pub type HMENU = *mut c_void;
pub type HICON = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HDC = *mut c_void;
pub type HKEY = *mut c_void;
pub type BOOL = i32;
pub type DWORD = u32;
pub type UINT = u32;
pub type WORD = u16;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type ULONG = u32;
pub type NTSTATUS = i32;
pub type KPRIORITY = i32;

pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
pub const NULL: *mut c_void = std::ptr::null_mut();

// Process Access Rights & Priority Classes
pub const PROCESS_SET_INFORMATION: DWORD = 0x0200;
pub const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;

pub const IDLE_PRIORITY_CLASS: DWORD = 0x00000040;
pub const BELOW_NORMAL_PRIORITY_CLASS: DWORD = 0x00004000;
pub const NORMAL_PRIORITY_CLASS: DWORD = 0x00000020;
pub const ABOVE_NORMAL_PRIORITY_CLASS: DWORD = 0x00008000;
pub const HIGH_PRIORITY_CLASS: DWORD = 0x00000080;

// Registry constants for GPU preferences
pub const HKEY_CURRENT_USER: HKEY = 0x80000001usize as HKEY;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x80000002usize as HKEY;
pub const KEY_SET_VALUE: DWORD = 0x0002;
pub const KEY_QUERY_VALUE: DWORD = 0x0001;
pub const REG_SZ: DWORD = 1;

// Token / Privilege constants
pub const TOKEN_ADJUST_PRIVILEGES: DWORD = 0x0020;
pub const TOKEN_QUERY: DWORD = 0x0008;
pub const SE_PRIVILEGE_ENABLED: DWORD = 0x00000002;
pub const TokenElevation: u32 = 20;

// NT System Information
pub const SystemProcessInformation: u32 = 5;
pub const STATUS_INFO_LENGTH_MISMATCH: NTSTATUS = 0xC0000004u32 as i32;

// Logical Processor Information
pub const RelationProcessorCore: u32 = 0;
pub const RelationCache: u32 = 2;

// Window & Tray Constants
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_COMMAND: UINT = 0x0111;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_USER: UINT = 0x0400;
pub const WM_TRAYICON: UINT = WM_USER + 1;
pub const WM_MOUSEMOVE: UINT = 0x0200;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_RBUTTONUP: UINT = 0x0205;
pub const WM_CONTEXTMENU: UINT = 0x007B;

pub const NIM_ADD: DWORD = 0x00000000;
pub const NIM_MODIFY: DWORD = 0x00000001;
pub const NIM_DELETE: DWORD = 0x00000002;
pub const NIF_MESSAGE: UINT = 0x00000001;
pub const NIF_ICON: UINT = 0x00000002;
pub const NIF_TIP: UINT = 0x00000004;

pub const MF_STRING: UINT = 0x00000000;
pub const MF_CHECKED: UINT = 0x00000008;
pub const MF_UNCHECKED: UINT = 0x00000000;
pub const MF_GRAYED: UINT = 0x00000001;
pub const MF_POPUP: UINT = 0x00000010;
pub const MF_SEPARATOR: UINT = 0x00000800;

pub const TPM_BOTTOMALIGN: UINT = 0x0020;
pub const TPM_LEFTALIGN: UINT = 0x0000;
pub const TPM_RIGHTBUTTON: UINT = 0x0002;

pub const SW_SHOWNORMAL: i32 = 1;
pub const ERROR_ALREADY_EXISTS: DWORD = 183;
pub const ERROR_SUCCESS: DWORD = 0;

// Native File Dialog Constants (comdlg32)
pub const OFN_FILEMUSTEXIST: DWORD = 0x00001000;
pub const OFN_PATHMUSTEXIST: DWORD = 0x00000800;
pub const OFN_NOCHANGEDIR: DWORD = 0x00000008;
pub const OFN_HIDEREADONLY: DWORD = 0x00000004;

#[repr(C)]
pub struct OPENFILENAMEW {
    pub lStructSize: DWORD,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: *const u16,
    pub lpstrCustomFilter: *mut u16,
    pub nMaxCustFilter: DWORD,
    pub nFilterIndex: DWORD,
    pub lpstrFile: *mut u16,
    pub nMaxFile: DWORD,
    pub lpstrFileTitle: *mut u16,
    pub nMaxFileTitle: DWORD,
    pub lpstrInitialDir: *const u16,
    pub lpstrTitle: *const u16,
    pub Flags: DWORD,
    pub nFileOffset: WORD,
    pub nFileExtension: WORD,
    pub lpstrDefExt: *const u16,
    pub lCustData: LPARAM,
    pub lpfnHook: *mut c_void,
    pub lpTemplateName: *const u16,
    pub pvReserved: *mut c_void,
    pub dwReserved: DWORD,
    pub FlagsEx: DWORD,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GROUP_AFFINITY {
    pub Mask: usize,
    pub Group: WORD,
    pub Reserved: [WORD; 3],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PROCESSOR_RELATIONSHIP {
    pub Flags: u8,
    pub EfficiencyClass: u8,
    pub Reserved: [u8; 20],
    pub GroupCount: WORD,
    pub GroupMask: [GROUP_AFFINITY; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CACHE_RELATIONSHIP {
    pub Level: u8,
    pub Associativity: u8,
    pub LineSize: WORD,
    pub CacheSize: DWORD,
    pub Type: u32,
    pub Reserved: [u8; 18],
    pub GroupCount: WORD,
    pub GroupMask: GROUP_AFFINITY,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *mut u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SYSTEM_PROCESS_INFORMATION {
    pub NextEntryOffset: ULONG,
    pub NumberOfThreads: ULONG,
    pub WorkingSetPrivateSize: i64,
    pub HardFaultCount: ULONG,
    pub NumberOfThreadsHighWatermark: ULONG,
    pub CycleTime: u64,
    pub CreateTime: i64,
    pub UserTime: i64,
    pub KernelTime: i64,
    pub ImageName: UNICODE_STRING,
    pub BasePriority: KPRIORITY,
    pub UniqueProcessId: usize,
    pub InheritedFromUniqueProcessId: usize,
    pub HandleCount: ULONG,
    pub SessionId: ULONG,
    pub UniqueProcessKey: usize,
    pub PeakVirtualSize: usize,
    pub VirtualSize: usize,
    pub PageFaultCount: ULONG,
    pub PeakWorkingSetSize: usize,
    pub WorkingSetSize: usize,
    pub QuotaPeakPagedPoolUsage: usize,
    pub QuotaPagedPoolUsage: usize,
    pub QuotaPeakNonPagedPoolUsage: usize,
    pub QuotaNonPagedPoolUsage: usize,
    pub PagefileUsage: usize,
    pub PeakPagefileUsage: usize,
    pub PrivatePageCount: usize,
    pub ReadOperationCount: i64,
    pub WriteOperationCount: i64,
    pub OtherOperationCount: i64,
    pub ReadTransferCount: i64,
    pub WriteTransferCount: i64,
    pub OtherTransferCount: i64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LUID {
    pub LowPart: DWORD,
    pub HighPart: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LUID_AND_ATTRIBUTES {
    pub Luid: LUID,
    pub Attributes: DWORD,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TOKEN_PRIVILEGES {
    pub PrivilegeCount: DWORD,
    pub Privileges: [LUID_AND_ATTRIBUTES; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TOKEN_ELEVATION {
    pub TokenIsElevated: DWORD,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

pub type WNDPROC = unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: Option<WNDPROC>,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: DWORD,
    pub hWnd: HWND,
    pub uID: UINT,
    pub uFlags: UINT,
    pub uCallbackMessage: UINT,
    pub hIcon: HICON,
    pub szTip: [u16; 128],
    pub dwState: DWORD,
    pub dwStateMask: DWORD,
    pub szInfo: [u16; 256],
    pub uTimeoutOrVersion: UINT,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: DWORD,
    pub guidItem: GUID,
    pub hBalloonIcon: HICON,
}

#[repr(C)]
pub struct ICONINFO {
    pub fIcon: BOOL,
    pub xHotspot: DWORD,
    pub yHotspot: DWORD,
    pub hbmMask: HBITMAP,
    pub hbmColor: HBITMAP,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SYSTEMTIME {
    pub wYear: WORD,
    pub wMonth: WORD,
    pub wDayOfWeek: WORD,
    pub wDay: WORD,
    pub wHour: WORD,
    pub wMinute: WORD,
    pub wSecond: WORD,
    pub wMilliseconds: WORD,
}

#[link(name = "ntdll")]
extern "system" {
    pub fn NtQuerySystemInformation(
        SystemInformationClass: u32,
        SystemInformation: *mut c_void,
        SystemInformationLength: ULONG,
        ReturnLength: *mut ULONG,
    ) -> NTSTATUS;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn QueryFullProcessImageNameW(
        hProcess: HANDLE,
        dwFlags: DWORD,
        lpExeName: *mut u16,
        lpdwSize: *mut DWORD,
    ) -> BOOL;
    pub fn SetProcessAffinityMask(hProcess: HANDLE, dwProcessAffinityMask: usize) -> BOOL;
    pub fn GetProcessAffinityMask(
        hProcess: HANDLE,
        lpProcessAffinityMask: *mut usize,
        lpSystemAffinityMask: *mut usize,
    ) -> BOOL;
    pub fn SetPriorityClass(hProcess: HANDLE, dwPriorityClass: DWORD) -> BOOL;
    pub fn GetPriorityClass(hProcess: HANDLE) -> DWORD;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetProcessHeap() -> HANDLE;
    pub fn HeapSetInformation(
        HeapHandle: HANDLE,
        HeapInformationClass: u32,
        HeapInformation: *mut c_void,
        HeapInformationLength: usize,
    ) -> BOOL;
    pub fn SetProcessWorkingSetSize(
        hProcess: HANDLE,
        dwMinimumWorkingSetSize: usize,
        dwMaximumWorkingSetSize: usize,
    ) -> BOOL;
    pub fn GetLogicalProcessorInformationEx(
        RelationshipType: u32,
        Buffer: *mut u8,
        ReturnedLength: *mut DWORD,
    ) -> BOOL;
    pub fn GetModuleFileNameW(hModule: HINSTANCE, lpFilename: *mut u16, nSize: DWORD) -> DWORD;
    pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    pub fn GetEnvironmentVariableW(lpName: *const u16, lpBuffer: *mut u16, nSize: DWORD) -> DWORD;
    pub fn GetCurrentPackageFamilyName(
        packageFamilyNameLength: *mut u32,
        packageFamilyName: *mut u16,
    ) -> DWORD;
    pub fn CreateMutexW(
        lpMutexAttributes: *mut c_void,
        bInitialOwner: BOOL,
        lpName: *const u16,
    ) -> HANDLE;
    pub fn GetLastError() -> DWORD;
    pub fn GetTickCount64() -> u64;
    pub fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    pub fn GetUserDefaultUILanguage() -> u16;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn OpenProcessToken(
        ProcessHandle: HANDLE,
        DesiredAccess: DWORD,
        TokenHandle: *mut HANDLE,
    ) -> BOOL;
    pub fn LookupPrivilegeValueW(
        lpSystemName: *const u16,
        lpName: *const u16,
        lpLuid: *mut LUID,
    ) -> BOOL;
    pub fn AdjustTokenPrivileges(
        TokenHandle: HANDLE,
        DisableAllPrivileges: BOOL,
        NewState: *const TOKEN_PRIVILEGES,
        BufferLength: DWORD,
        PreviousState: *mut TOKEN_PRIVILEGES,
        ReturnLength: *mut DWORD,
    ) -> BOOL;
    pub fn GetTokenInformation(
        TokenHandle: HANDLE,
        TokenInformationClass: u32,
        TokenInformation: *mut c_void,
        TokenInformationLength: DWORD,
        ReturnLength: *mut DWORD,
    ) -> BOOL;
    pub fn RegCreateKeyExW(
        hKey: HKEY,
        lpSubKey: *const u16,
        Reserved: DWORD,
        lpClass: *mut u16,
        dwOptions: DWORD,
        samDesired: DWORD,
        lpSecurityAttributes: *const c_void,
        phkResult: *mut HKEY,
        lpdwDisposition: *mut DWORD,
    ) -> i32;
    pub fn RegSetValueExW(
        hKey: HKEY,
        lpValueName: *const u16,
        Reserved: DWORD,
        dwType: DWORD,
        lpData: *const u8,
        cbData: DWORD,
    ) -> i32;
    pub fn RegCloseKey(hKey: HKEY) -> i32;
    pub fn RegOpenKeyExW(
        hKey: HKEY,
        lpSubKey: *const u16,
        ulOptions: DWORD,
        samDesired: DWORD,
        phkResult: *mut HKEY,
    ) -> i32;
    pub fn RegQueryValueExW(
        hKey: HKEY,
        lpValueName: *const u16,
        lpReserved: *mut DWORD,
        lpType: *mut DWORD,
        lpData: *mut u8,
        lpcbData: *mut DWORD,
    ) -> i32;
    pub fn RegDeleteValueW(hKey: HKEY, lpValueName: *const u16) -> i32;
}

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(lpWndClass: *const WNDCLASSEXW) -> WORD;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: DWORD,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn SetTimer(hWnd: HWND, nIDEvent: usize, uElapse: UINT, lpTimerFunc: *mut c_void) -> usize;
    pub fn KillTimer(hWnd: HWND, uIDEvent: usize) -> BOOL;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(hMenu: HMENU, uFlags: UINT, uIDNewItem: usize, lpNewItem: *const u16) -> BOOL;
    pub fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: UINT,
        x: i32,
        y: i32,
        nReserved: i32,
        hWnd: HWND,
        prcRect: *const c_void,
    ) -> BOOL;
    pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
    pub fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    pub fn GetForegroundWindow() -> HWND;
    pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut DWORD) -> DWORD;
    pub fn CreateIconIndirect(piconinfo: *const ICONINFO) -> HICON;
    pub fn DestroyIcon(hIcon: HICON) -> BOOL;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateBitmap(
        nWidth: i32,
        nHeight: i32,
        nPlanes: UINT,
        nBitCount: UINT,
        lpBits: *const c_void,
    ) -> HBITMAP;
    pub fn DeleteObject(ho: *mut c_void) -> BOOL;
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(dwMessage: DWORD, lpData: *const NOTIFYICONDATAW) -> BOOL;
    pub fn ShellExecuteW(
        hwnd: HWND,
        lpOperation: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShowCmd: i32,
    ) -> HINSTANCE;
}

#[link(name = "comdlg32")]
extern "system" {
    pub fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> BOOL;
}

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Opens the native Windows File Picker (`comdlg32.dll!GetOpenFileNameW`) and returns the full path.
/// `filter` is a list of `(description, pattern)` pairs, e.g. `[("应用程序", "*.exe")]`.
pub fn pick_file(hwnd_owner: HWND, title_str: &str, filter_pairs: &[(&str, &str)]) -> Option<String> {
    unsafe {
        let mut file_buf = [0u16; 1024];
        let mut filter: Vec<u16> = Vec::new();
        for (desc, pat) in filter_pairs.iter().chain(&[("所有文件 (*.*)", "*.*")]) {
            filter.extend(desc.encode_utf16());
            filter.push(0);
            filter.extend(pat.encode_utf16());
            filter.push(0);
        }
        filter.push(0);

        let title = to_wide(title_str);

        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as DWORD;
        ofn.hwndOwner = hwnd_owner;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.nFilterIndex = 1;
        ofn.lpstrFile = file_buf.as_mut_ptr();
        ofn.nMaxFile = file_buf.len() as DWORD;
        ofn.lpstrTitle = title.as_ptr();
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_HIDEREADONLY;

        if GetOpenFileNameW(&mut ofn) != 0 {
            let end = file_buf.iter().position(|&c| c == 0).unwrap_or(0);
            if end > 0 {
                return Some(String::from_utf16_lossy(&file_buf[..end]));
            }
        }
        None
    }
}

/// Picks any `.exe` and returns only its lowercase file name (e.g., `"game.exe"`).
pub fn pick_exe_file_name(hwnd_owner: HWND, title_str: &str) -> Option<String> {
    let full_path = pick_file(hwnd_owner, title_str, &[("应用程序 (*.exe)", "*.exe")])?;
    Some(
        std::path::Path::new(&full_path)
            .file_name()?
            .to_string_lossy()
            .to_ascii_lowercase(),
    )
}

/// Full lowercase image path of a process (`None` if the handle cannot be opened).
pub fn process_image_path_lower(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == NULL || h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len: DWORD = buf.len() as DWORD;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let mut s = String::from_utf16_lossy(&buf[..len as usize]);
        s.make_ascii_lowercase();
        Some(s)
    }
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "SmartScheduler";

/// Whether the portable build is registered under `HKCU\...\Run`.
pub fn autostart_enabled() -> bool {
    unsafe {
        let subkey = to_wide(RUN_KEY);
        let mut hkey: HKEY = NULL;
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
            return false;
        }
        let name = to_wide(RUN_VALUE);
        let mut size: DWORD = 0;
        let found = RegQueryValueExW(
            hkey,
            name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        ) == 0;
        RegCloseKey(hkey);
        found
    }
}

/// Adds/removes the current exe under `HKCU\...\Run` (portable build only; MSIX uses its own startupTask).
pub fn set_autostart(enable: bool) -> bool {
    unsafe {
        let subkey = to_wide(RUN_KEY);
        let name = to_wide(RUN_VALUE);
        let mut hkey: HKEY = NULL;
        let mut disp: DWORD = 0;
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut hkey,
            &mut disp,
        ) != 0
        {
            return false;
        }
        let ok = if enable {
            let mut exe_buf = [0u16; 1024];
            let n = GetModuleFileNameW(NULL as _, exe_buf.as_mut_ptr(), 1024);
            if n == 0 {
                RegCloseKey(hkey);
                return false;
            }
            let cmd = to_wide(&format!("\"{}\"", String::from_utf16_lossy(&exe_buf[..n as usize])));
            RegSetValueExW(hkey, name.as_ptr(), 0, REG_SZ, cmd.as_ptr() as *const u8, (cmd.len() * 2) as DWORD) == 0
        } else {
            // 2 = ERROR_FILE_NOT_FOUND counts as already removed
            let r = RegDeleteValueW(hkey, name.as_ptr());
            r == 0 || r == 2
        };
        RegCloseKey(hkey);
        ok
    }
}
#[derive(Clone, Debug)]
pub struct CpuPreset {
    pub label: String,
    pub label_en: String,
    pub spec: String,
}

impl CpuPreset {
    pub fn display_label(&self, is_zh: bool) -> &str {
        if is_zh || self.label_en.is_empty() {
            &self.label
        } else {
            &self.label_en
        }
    }
}

/// Converts a bitmask like `0x0000FFFF` into compact range notation like `"0-15"` or `"0,2,4,6"`
pub fn mask_to_spec_string(mask: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut bit = 0u32;
    while bit < usize::BITS {
        if (mask & (1usize << bit)) != 0 {
            let start = bit;
            while bit + 1 < usize::BITS && (mask & (1usize << (bit + 1))) != 0 {
                bit += 1;
            }
            let end = bit;
            if end > start + 1 {
                parts.push(format!("{}-{}", start, end));
            } else if end == start + 1 {
                parts.push(format!("{},{}", start, end));
            } else {
                parts.push(format!("{}", start));
            }
        }
        bit += 1;
    }
    parts.join(",")
}

fn clean_cpu_model_name(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some((before_at, _)) = s.split_once('@') {
        s = before_at.trim();
    }
    let mut parts: Vec<&str> = s.split_whitespace().collect();
    if let Some(&"Processor") = parts.last() {
        parts.pop();
    }
    if let Some(&"CPU") = parts.last() {
        parts.pop();
    }
    if let Some(last) = parts.last() {
        if last.ends_with("-Core") || last.ends_with("-core") {
            parts.pop();
        }
    }
    let res = parts.join(" ");
    if res.is_empty() {
        raw.trim().to_string()
    } else {
        res
    }
}

pub fn query_cpu_name_from_registry() -> Option<String> {
    unsafe {
        let subkey = to_wide("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0");
        let mut hkey: HKEY = NULL;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
            return None;
        }
        let val_name = to_wide("ProcessorNameString");
        let mut buf = [0u16; 128];
        let mut size: DWORD = (buf.len() * 2) as DWORD;
        let mut ty: DWORD = 0;
        let ok = RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            buf.as_mut_ptr() as *mut u8,
            &mut size,
        ) == 0;
        RegCloseKey(hkey);
        if !ok || size == 0 {
            return None;
        }
        let chars = (size as usize) / 2;
        let s = String::from_utf16_lossy(&buf[..chars]);
        let end = s.find('\0').unwrap_or(s.len());
        Some(clean_cpu_model_name(&s[..end]))
    }
}

pub fn query_cpu_name() -> String {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let ext = core::arch::x86_64::__cpuid(0x8000_0000);
        if ext.eax >= 0x8000_0004 {
            let mut brand = [0u8; 48];
            for i in 0..3 {
                let leaf = 0x8000_0002 + i;
                let res = core::arch::x86_64::__cpuid(leaf);
                let offset = (i as usize) * 16;
                brand[offset..offset + 4].copy_from_slice(&res.eax.to_le_bytes());
                brand[offset + 4..offset + 8].copy_from_slice(&res.ebx.to_le_bytes());
                brand[offset + 8..offset + 12].copy_from_slice(&res.ecx.to_le_bytes());
                brand[offset + 12..offset + 16].copy_from_slice(&res.edx.to_le_bytes());
            }
            let end = brand.iter().position(|&b| b == 0).unwrap_or(brand.len());
            let raw = String::from_utf8_lossy(&brand[..end]);
            let s = clean_cpu_model_name(raw.trim());
            if !s.is_empty() {
                return s;
            }
        }
    }
    query_cpu_name_from_registry().unwrap_or_else(|| "Unknown CPU".to_string())
}

/// Automatically detects CPU hardware topology (AMD Multi-CCD / 3D V-Cache L3 domains,
/// Intel P-Core / E-Core EfficiencyClass, and SMT physical cores) on ANY machine!
pub fn detect_cpu_presets() -> (String, Vec<CpuPreset>) {
    let mut presets: Vec<CpuPreset> = Vec::new();
    let topo_summary: String;

    unsafe {
        // 1. Query L3 Cache Domains (RelationCache = 2) -> Identifies AMD CCDs & 3D V-Cache size
        let mut len: DWORD = 0;
        GetLogicalProcessorInformationEx(RelationCache, std::ptr::null_mut(), &mut len);
        let mut l3_groups: Vec<(usize, u32)> = Vec::new(); // (mask, size_mb)
        if len > 0 {
            let mut buf = vec![0u8; len as usize];
            if GetLogicalProcessorInformationEx(RelationCache, buf.as_mut_ptr(), &mut len) != 0 {
                let mut offset = 0usize;
                while offset + 8 <= len as usize {
                    let rel_size = *(buf.as_ptr().add(offset + 4) as *const DWORD) as usize;
                    if rel_size == 0 {
                        break;
                    }
                    let cache = &*(buf.as_ptr().add(offset + 8) as *const CACHE_RELATIONSHIP);
                    if cache.Level == 3 && cache.GroupMask.Mask != 0 {
                        let mb = cache.CacheSize / (1024 * 1024);
                        if !l3_groups.iter().any(|(m, _)| *m == cache.GroupMask.Mask) {
                            l3_groups.push((cache.GroupMask.Mask, mb));
                        }
                    }
                    offset += rel_size;
                }
            }
        }
        l3_groups.sort_by_key(|(m, _)| m.trailing_zeros());

        // 2. Query Physical Cores & EfficiencyClass (RelationProcessorCore = 0) -> Intel P/E cores & SMT
        let mut core_len: DWORD = 0;
        GetLogicalProcessorInformationEx(RelationProcessorCore, std::ptr::null_mut(), &mut core_len);
        let mut core_masks: Vec<(usize, u8)> = Vec::new(); // (core_mask, efficiency_class)
        let mut smt0_mask: usize = 0;

        if core_len > 0 {
            let mut buf = vec![0u8; core_len as usize];
            if GetLogicalProcessorInformationEx(
                RelationProcessorCore,
                buf.as_mut_ptr(),
                &mut core_len,
            ) != 0
            {
                let mut offset = 0usize;
                while offset + 8 <= core_len as usize {
                    let rel_size = *(buf.as_ptr().add(offset + 4) as *const DWORD) as usize;
                    if rel_size == 0 {
                        break;
                    }
                    let proc = &*(buf.as_ptr().add(offset + 8) as *const PROCESSOR_RELATIONSHIP);
                    if proc.GroupCount > 0 {
                        let m = proc.GroupMask[0].Mask;
                        if m != 0 {
                            core_masks.push((m, proc.EfficiencyClass));
                            // Lowest set bit is physical thread 0 (no SMT sibling)
                            smt0_mask |= m & m.wrapping_neg();
                        }
                    }
                    offset += rel_size;
                }
            }
        }

        let cpu_name = query_cpu_name();
        let physical_cores = core_masks.len();
        let all_logical: usize = core_masks.iter().fold(0, |acc, &(m, _)| acc | m);
        let logical_threads = all_logical.count_ones() as usize;

        // Check if Intel Hybrid (multiple EfficiencyClass values)
        let min_eff = core_masks.iter().map(|(_, e)| *e).min().unwrap_or(0);
        let max_eff = core_masks.iter().map(|(_, e)| *e).max().unwrap_or(0);

        let total_l3_mb: u32 = l3_groups.iter().map(|(_, mb)| *mb).sum();
        let is_asymmetric_x3d = if l3_groups.len() >= 2 {
            let max_mb = l3_groups.iter().map(|(_, mb)| *mb).max().unwrap_or(0);
            let min_mb = l3_groups.iter().map(|(_, mb)| *mb).min().unwrap_or(0);
            max_mb > min_mb && min_mb > 0
        } else {
            false
        };

        let cache_str = if is_asymmetric_x3d && l3_groups.len() == 2 {
            format!("{}MB L3 ({}MB+{}MB)", total_l3_mb, l3_groups[0].1, l3_groups[1].1)
        } else if l3_groups.len() > 1 && l3_groups.windows(2).all(|w| w[0].1 == w[1].1) {
            format!("{}MB L3 ({}MB×{})", total_l3_mb, l3_groups[0].1, l3_groups.len())
        } else if total_l3_mb > 0 {
            format!("{}MB L3", total_l3_mb)
        } else {
            String::new()
        };

        let core_str = if max_eff > min_eff {
            let p_cnt = core_masks.iter().filter(|(_, e)| *e == max_eff).count();
            let e_cnt = core_masks.iter().filter(|(_, e)| *e != max_eff).count();
            format!("{}核{}线程 ({}P+{}E)", physical_cores, logical_threads, p_cnt, e_cnt)
        } else {
            format!("{}核{}线程", physical_cores, logical_threads)
        };

        topo_summary = if !cache_str.is_empty() {
            format!("{} [{} · {}]", cpu_name, core_str, cache_str)
        } else {
            format!("{} [{}]", cpu_name, core_str)
        };

        if max_eff > min_eff {
            // Case A: Hybrid Architecture (Intel P-Core + E-Core)
            let mut p_mask = 0usize;
            let mut e_mask = 0usize;
            for &(m, eff) in &core_masks {
                if eff == max_eff {
                    p_mask |= m;
                } else {
                    e_mask |= m;
                }
            }
            let p_spec = mask_to_spec_string(p_mask);
            presets.push(CpuPreset {
                label: format!("P-Cores 性能大核 ({})", p_spec),
                label_en: format!("P-Cores Performance ({})", p_spec),
                spec: p_spec,
            });
            // Game preset: P-cores excluding Core 0
            if let Some(&(first_core_m, _)) = core_masks.first() {
                let p_no_c0 = p_mask & !first_core_m;
                if p_no_c0 != 0 {
                    let spec = mask_to_spec_string(p_no_c0);
                    presets.push(CpuPreset {
                        label: format!("P-Cores 纯净游戏核 (避开首核: {})", spec),
                        label_en: format!("P-Cores Gaming (Skip Core 0: {})", spec),
                        spec,
                    });
                }
            }
            let e_spec = mask_to_spec_string(e_mask);
            presets.push(CpuPreset {
                label: format!("E-Cores 能效小核 ({})", e_spec),
                label_en: format!("E-Cores Efficiency ({})", e_spec),
                spec: e_spec,
            });
        } else if l3_groups.len() >= 2 {
            // Case B: Multi-CCD / Multi-L3 Architecture (e.g., Ryzen 9 9950X3D, 7950X3D, 7900X3D, 9950X)
            let max_mb = l3_groups.iter().map(|(_, mb)| *mb).max().unwrap_or(0);

            for (idx, &(mask, mb)) in l3_groups.iter().enumerate() {
                let spec = mask_to_spec_string(mask);
                let (tag, tag_en) = if is_asymmetric_x3d && mb == max_mb {
                    (
                        format!("CCD{} 3D大缓存簇 ({}MB: {})", idx, mb, spec),
                        format!("CCD{} 3D V-Cache ({}MB: {})", idx, mb, spec),
                    )
                } else if is_asymmetric_x3d {
                    (
                        format!("CCD{} 高频簇 ({}MB: {})", idx, mb, spec),
                        format!("CCD{} Frequency Cluster ({}MB: {})", idx, mb, spec),
                    )
                } else {
                    (
                        format!("CCD{} 核心簇 ({}MB L3: {})", idx, mb, spec),
                        format!("CCD{} Core Cluster ({}MB L3: {})", idx, mb, spec),
                    )
                };
                presets.push(CpuPreset {
                    label: tag,
                    label_en: tag_en,
                    spec,
                });

                // For the primary/V-Cache CCD, also offer a "skip first 2 cores" game preset
                if (idx == 0 || (is_asymmetric_x3d && mb == max_mb)) && core_masks.len() >= 4 {
                    let mut ccd_cores: Vec<usize> = core_masks
                        .iter()
                        .map(|(cm, _)| *cm)
                        .filter(|&cm| (cm & mask) == cm)
                        .collect();
                    ccd_cores.sort_by_key(|cm| cm.trailing_zeros());
                    if ccd_cores.len() > 2 {
                        let game_mask = mask & !(ccd_cores[0] | ccd_cores[1]);
                        if game_mask != 0 {
                            let g_spec = mask_to_spec_string(game_mask);
                            presets.push(CpuPreset {
                                label: format!("CCD{} 纯净游戏核 (避开前2核: {})", idx, g_spec),
                                label_en: format!("CCD{} Gaming (Skip First 2 Cores: {})", idx, g_spec),
                                spec: g_spec,
                            });
                        }
                    }
                }
            }
        } else {
            // Case C: Standard Single-CCD CPU
            let all_mask: usize = core_masks.iter().fold(0, |acc, &(m, _)| acc | m);
            if core_masks.len() > 2 {
                let no_c0 = all_mask & !core_masks[0].0;
                let spec = mask_to_spec_string(no_c0);
                presets.push(CpuPreset {
                    label: format!("游戏核心 (避开首核中断: {})", spec),
                    label_en: format!("Gaming Cores (Skip Core 0 IRQ: {})", spec),
                    spec,
                });
                let half = core_masks.len() / 2;
                let first_half: usize = core_masks[..half].iter().fold(0, |a, &(m, _)| a | m);
                let second_half: usize = core_masks[half..].iter().fold(0, |a, &(m, _)| a | m);
                let s1 = mask_to_spec_string(first_half);
                let s2 = mask_to_spec_string(second_half);
                presets.push(CpuPreset {
                    label: format!("前半核心簇 ({})", s1),
                    label_en: format!("First Half Cores ({})", s1),
                    spec: s1,
                });
                presets.push(CpuPreset {
                    label: format!("后半核心簇 ({})", s2),
                    label_en: format!("Second Half Cores ({})", s2),
                    spec: s2,
                });
            }
        }

        // Always append SMT-off (Physical cores only) preset if SMT is enabled
        let all_logical: usize = core_masks.iter().fold(0, |acc, &(m, _)| acc | m);
        if smt0_mask != 0 && smt0_mask != all_logical {
            let smt_spec = mask_to_spec_string(smt0_mask);
            presets.push(CpuPreset {
                label: format!("仅物理主核 (关闭超线程 SMT0: {})", smt_spec),
                label_en: format!("Physical Cores Only (SMT0: {})", smt_spec),
                spec: smt_spec,
            });
        }
    }

    (topo_summary, presets)
}

pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = NULL;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len: DWORD = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as DWORD,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

pub fn enable_debug_privilege() -> bool {
    unsafe {
        let mut token: HANDLE = NULL;
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return false;
        }
        let mut luid = LUID {
            LowPart: 0,
            HighPart: 0,
        };
        let priv_name = to_wide("SeDebugPrivilege");
        if LookupPrivilegeValueW(std::ptr::null(), priv_name.as_ptr(), &mut luid) == 0 {
            CloseHandle(token);
            return false;
        }
        let tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let ok = AdjustTokenPrivileges(
            token,
            0,
            &tp,
            std::mem::size_of::<TOKEN_PRIVILEGES>() as DWORD,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        CloseHandle(token);
        ok != 0
    }
}

pub fn set_windows_gpu_preference(exe_path: &str, pref_code: u8) -> bool {
    if pref_code != 1 && pref_code != 2 {
        return false;
    }
    unsafe {
        let subkey = to_wide("Software\\Microsoft\\DirectX\\UserGpuPreferences");
        let mut hkey: HKEY = NULL;
        let mut disp: DWORD = 0;
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut hkey,
            &mut disp,
        ) != 0
        {
            return false;
        }
        let val_name = to_wide(exe_path);
        let val_data = to_wide(&format!("GpuPreference={};", pref_code));
        let ok = RegSetValueExW(
            hkey,
            val_name.as_ptr(),
            0,
            REG_SZ,
            val_data.as_ptr() as *const u8,
            (val_data.len() * 2) as DWORD,
        ) == 0;
        RegCloseKey(hkey);
        ok
    }
}

pub fn create_status_icon(bg_rgb: u32) -> HICON {
    let mut pixels = [0u32; 16 * 16];
    let r_bg = ((bg_rgb >> 16) & 0xFF) as i32;
    let g_bg = ((bg_rgb >> 8) & 0xFF) as i32;
    let b_bg = (bg_rgb & 0xFF) as i32;

    // 16x16 bitmap for a crisp "S" glyph inside the circle
    for y in 0..16 {
        for x in 0..16 {
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();

            let is_s_glyph = ((y == 4 || y == 5) && x >= 5 && x <= 10)
                || ((y == 6) && x >= 5 && x <= 6)
                || ((y == 7 || y == 8) && x >= 5 && x <= 10)
                || ((y == 9) && x >= 9 && x <= 10)
                || ((y == 10 || y == 11) && x >= 5 && x <= 10);

            if is_s_glyph {
                pixels[y * 16 + x] = 0xFFFFFFFF;
            } else if dist <= 6.8 {
                pixels[y * 16 + x] = 0xFF000000 | (bg_rgb & 0x00FFFFFF);
            } else if dist <= 7.6 {
                let alpha = ((7.6 - dist) / 0.8 * 255.0).clamp(0.0, 255.0) as u32;
                let r = (r_bg as u32 * alpha) / 255;
                let g = (g_bg as u32 * alpha) / 255;
                let b = (b_bg as u32 * alpha) / 255;
                pixels[y * 16 + x] = (alpha << 24) | (r << 16) | (g << 8) | b;
            } else {
                pixels[y * 16 + x] = 0x00000000;
            }
        }
    }

    let mask_bits = [0u8; 32];
    unsafe {
        let hbm_color = CreateBitmap(16, 16, 1, 32, pixels.as_ptr() as *const c_void);
        let hbm_mask = CreateBitmap(16, 16, 1, 1, mask_bits.as_ptr() as *const c_void);
        let ii = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let icon = CreateIconIndirect(&ii);
        DeleteObject(hbm_color as *mut c_void);
        DeleteObject(hbm_mask as *mut c_void);
        icon
    }
}

pub fn now_hms() -> String {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond)
    }
}

/// Hints the Windows NT Heap manager to return unused virtual memory to the OS,
/// and trims unreferenced DLL/UI code pages from the physical Working Set.
pub fn optimize_heap_and_trim_working_set() {
    unsafe {
        let h_heap = GetProcessHeap();
        if h_heap != NULL {
            let mut info = [1u32, 0u32]; // Version = 1, Flags = 0 (HeapOptimizeResources)
            let _ = HeapSetInformation(
                h_heap,
                3, // HeapOptimizeResources
                info.as_mut_ptr() as *mut c_void,
                std::mem::size_of_val(&info),
            );
        }
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}
