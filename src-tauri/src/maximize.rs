//! 启动外部程序后，可选地将其主窗口最大化，避免小屏下窗口超出屏幕。

#[cfg(windows)]
pub fn maximize_process_window(pid: u32) {
    use std::time::Duration;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_MAXIMIZE,
    };

    struct Ctx {
        pid: u32,
        hwnd: Option<HWND>,
    }

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == ctx.pid && IsWindowVisible(hwnd).0 != 0 {
            ctx.hwnd = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    std::thread::spawn(move || {
        for _ in 0..60 {
            let mut ctx = Ctx { pid, hwnd: None };
            unsafe {
                let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut Ctx as isize));
                if let Some(hwnd) = ctx.hwnd {
                    let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                    let _ = SetForegroundWindow(hwnd);
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

#[cfg(not(windows))]
pub fn maximize_process_window(_pid: u32) {}
