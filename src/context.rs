#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppContext {
    pub exe_name: Option<String>,
    pub window_title: Option<String>,
}

#[cfg(windows)]
pub fn current_app_context() -> AppContext {
    use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    };

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return AppContext::default();
    }

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }

    let exe_name = if pid == 0 {
        None
    } else {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) };
        if process.is_null() {
            None
        } else {
            let mut buf = [0u16; 260];
            let mut len = buf.len() as u32;
            let ok =
                unsafe { QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) != 0 };
            unsafe {
                CloseHandle(process);
            }
            if ok && len > 0 {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                path.rsplit(['\\', '/']).next().map(str::to_string)
            } else {
                None
            }
        }
    };

    let title_len = unsafe { GetWindowTextLengthW(hwnd) };
    let window_title = if title_len > 0 {
        let mut buf = vec![0u16; title_len as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        (copied > 0).then(|| String::from_utf16_lossy(&buf[..copied as usize]))
    } else {
        None
    };

    AppContext {
        exe_name,
        window_title,
    }
}
