//! 桌面图标显示/隐藏：托盘/热键切换 + 双击空白桌面切换。
//!
//! 切换优先给 `SHELLDLL_DefView` 发原生 `WM_COMMAND`(0x7402)，保留 Explorer
//! 的淡入淡出与右键菜单状态；失败时回退到 `ShowWindow`。
//! 双击由 `WH_MOUSE_LL` 低级鼠标钩子检测，并用 MSAA(`AccessibleObjectFromPoint`)
//! 排除桌面图标，避免双击快捷方式时误切换。

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::{AccessibleObjectFromPoint, IAccessible};
use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetMessageW,
    GetSystemMetrics, IsWindowVisible, PostThreadMessageW, SendMessageW, SetWindowsHookExW,
    ShowWindow, UnhookWindowsHookEx, WindowFromPoint, HHOOK, MSG, MSLLHOOKSTRUCT, SM_CXDOUBLECLK,
    SM_CYDOUBLECLK, SW_HIDE, SW_SHOW, WH_MOUSE_LL, WM_APP, WM_COMMAND, WM_LBUTTONDOWN,
};

const TOGGLE_ICONS_CMD: usize = 0x7402;
const ROLE_SYSTEM_ICON: i32 = 0x14;
const ROLE_SYSTEM_LISTITEM: i32 = 0x22;
const WM_APP_REFRESH: u32 = WM_APP + 1;

static ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static LAST_CLICK_TIME: AtomicU32 = AtomicU32::new(0);
static LAST_CLICK_X: AtomicI32 = AtomicI32::new(0);
static LAST_CLICK_Y: AtomicI32 = AtomicI32::new(0);
static HAS_LAST_CLICK: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<AppHandle> = OnceLock::new();

/// 启动钩子线程；`enabled` 决定是否安装全局鼠标钩子。
pub fn init(app: AppHandle, enabled: bool) {
    let _ = APP.set(app);
    ENABLED.store(enabled, Ordering::Relaxed);
    if let Err(error) = thread::Builder::new().name("desktop-icons-hook".into()).spawn(hook_thread)
    {
        portable_kit::logging::warn(&format!("启动桌面图标钩子线程失败：{error}"));
    }
}

/// 运行中开关双击检测（来自设置页保存）。
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    let tid = HOOK_THREAD_ID.load(Ordering::Relaxed);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_APP_REFRESH, WPARAM(0), LPARAM(0));
        }
    }
}

/// 切换桌面图标，返回切换后是否已隐藏。
pub fn toggle_icons() -> Result<bool, String> {
    let views = find_defviews();
    if views.is_empty() {
        return Err("未找到桌面图标视图（Explorer 可能未运行）".into());
    }
    let handles: Vec<isize> = views.iter().map(|h| h.0 as isize).collect();
    let before = icons_visible(&views);
    portable_kit::logging::info(&format!("desktop views={handles:?} visible_before={before}"));
    unsafe {
        for hwnd in &views {
            SendMessageW(*hwnd, WM_COMMAND, Some(WPARAM(TOGGLE_ICONS_CMD)), Some(LPARAM(0)));
        }
    }
    thread::sleep(Duration::from_millis(90));
    let mut after = icons_visible(&views);
    portable_kit::logging::info(&format!("after 0x7402 visible={after}"));
    if before == after {
        portable_kit::logging::info("0x7402 未改变图标可见性，使用 ShowWindow 回退");
        // 原生命令在个别 Windows 版本上无效时，直接隐藏/显示桌面视图。
        unsafe {
            for hwnd in &views {
                let _ = ShowWindow(*hwnd, if before { SW_HIDE } else { SW_SHOW });
            }
        }
        thread::sleep(Duration::from_millis(80));
        after = icons_visible(&views);
        portable_kit::logging::info(&format!("after ShowWindow visible={after}"));
    }
    if before == after {
        return Err("切换桌面图标失败（桌面视图未响应）".into());
    }
    Ok(!after)
}

fn hook_thread() {
    unsafe {
        HOOK_THREAD_ID.store(GetCurrentThreadId(), Ordering::Relaxed);
    }
    let mut hook: Option<HHOOK> = None;
    apply_hook(&mut hook);
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 <= 0 {
            break;
        }
        if msg.message == WM_APP_REFRESH {
            apply_hook(&mut hook);
        }
    }
    if let Some(h) = hook {
        unsafe {
            let _ = UnhookWindowsHookEx(h);
        }
    }
}

fn apply_hook(hook: &mut Option<HHOOK>) {
    let want = ENABLED.load(Ordering::Relaxed);
    if want && hook.is_none() {
        match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) } {
            Ok(h) => {
                *hook = Some(h);
                portable_kit::logging::info("desktop double-click watcher enabled");
            }
            Err(error) => {
                portable_kit::logging::error(&format!("安装桌面双击鼠标钩子失败：{error}"));
            }
        }
    } else if !want && hook.is_some() {
        if let Some(h) = hook.take() {
            unsafe {
                let _ = UnhookWindowsHookEx(h);
            }
            portable_kit::logging::info("desktop double-click watcher disabled");
        }
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_LBUTTONDOWN && ENABLED.load(Ordering::Relaxed) {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let now = info.time;
        let pt = info.pt;
        let double_click_time = unsafe { GetDoubleClickTime() };
        let double_click_cx = unsafe { GetSystemMetrics(SM_CXDOUBLECLK) };
        let double_click_cy = unsafe { GetSystemMetrics(SM_CYDOUBLECLK) };
        let had_click = HAS_LAST_CLICK.swap(false, Ordering::Relaxed);
        let elapsed = now.wrapping_sub(LAST_CLICK_TIME.load(Ordering::Relaxed));
        let dx = (pt.x - LAST_CLICK_X.load(Ordering::Relaxed)).abs();
        let dy = (pt.y - LAST_CLICK_Y.load(Ordering::Relaxed)).abs();
        if had_click
            && elapsed <= double_click_time
            && dx <= double_click_cx
            && dy <= double_click_cy
        {
            portable_kit::logging::info(&format!(
                "desktop double-click detected at {}x{}",
                pt.x, pt.y
            ));
            if let Some(app) = APP.get().cloned() {
                let click = POINT { x: pt.x, y: pt.y };
                let _ = thread::Builder::new()
                    .name("desktop-icons-click".into())
                    .spawn(move || handle_click(app, click));
            }
        } else {
            LAST_CLICK_TIME.store(now, Ordering::Relaxed);
            LAST_CLICK_X.store(pt.x, Ordering::Relaxed);
            LAST_CLICK_Y.store(pt.y, Ordering::Relaxed);
            HAS_LAST_CLICK.store(true, Ordering::Relaxed);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn handle_click(app: AppHandle, pt: POINT) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let hwnd = unsafe { WindowFromPoint(pt) };
    let class = class_name(hwnd);
    let empty = is_empty_desktop(pt);
    portable_kit::logging::info(&format!("desktop double-click hit class={class} empty={empty}"));
    if !empty {
        return;
    }
    match toggle_icons() {
        Ok(hidden) => {
            portable_kit::logging::info(&format!(
                "desktop icons toggled by double-click hidden={hidden}"
            ));
            let _ = app.emit("desktop-icons-toggled", hidden);
        }
        Err(error) => {
            portable_kit::logging::warn(&format!("desktop icons toggle failed: {error}"));
        }
    }
}

fn is_empty_desktop(pt: POINT) -> bool {
    let hwnd = unsafe { WindowFromPoint(pt) };
    if hwnd.0.is_null() {
        return false;
    }
    match class_name(hwnd).as_str() {
        "Progman" | "WorkerW" => true,
        "SHELLDLL_DefView" | "SysListView32" => match icon_under_point(pt) {
            Some(true) => false,
            Some(false) => true,
            None => false,
        },
        _ => false,
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

/// 返回 `Some(true)` 表示光标下是桌面图标，`Some(false)` 表示空白桌面；
/// `None` 表示 MSAA 命中测试失败（此时不切换，避免误伤图标）。
fn icon_under_point(pt: POINT) -> Option<bool> {
    unsafe {
        let mut acc: Option<IAccessible> = None;
        let mut child = VARIANT::default();
        AccessibleObjectFromPoint(pt, &mut acc, &mut child).ok()?;
        let acc = acc?;
        let role = acc.get_accRole(&child).ok()?;
        let value = &role.Anonymous.Anonymous;
        if value.vt == VT_I4 {
            let role_value = value.Anonymous.lVal;
            Some(role_value == ROLE_SYSTEM_LISTITEM || role_value == ROLE_SYSTEM_ICON)
        } else {
            Some(false)
        }
    }
}

fn find_defviews() -> Vec<HWND> {
    let mut out = Vec::new();
    unsafe {
        if let Ok(progman) = FindWindowW(w!("Progman"), PCWSTR::null()) {
            push_defview(&mut out, progman);
        }
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut out as *mut Vec<HWND> as isize));
    }
    out
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let out = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    if class_name(hwnd) == "SHELLDLL_DefView" {
        push_unique(out, hwnd);
    }
    push_defview(out, hwnd);
    BOOL(1)
}

fn push_defview(out: &mut Vec<HWND>, parent: HWND) {
    if let Ok(view) =
        unsafe { FindWindowExW(Some(parent), None, w!("SHELLDLL_DefView"), PCWSTR::null()) }
    {
        push_unique(out, view);
    }
}

fn push_unique(out: &mut Vec<HWND>, hwnd: HWND) {
    if !out.iter().any(|item| item.0 == hwnd.0) {
        out.push(hwnd);
    }
}

fn icons_visible(views: &[HWND]) -> bool {
    views.iter().any(|hwnd| {
        let target =
            unsafe { FindWindowExW(Some(*hwnd), None, w!("SysListView32"), PCWSTR::null()) }
                .unwrap_or(*hwnd);
        unsafe { IsWindowVisible(target).as_bool() }
    })
}
