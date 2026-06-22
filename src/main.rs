#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("uvie-rs-win is a Windows tray app.");
}

#[cfg(windows)]
mod win {
    #![allow(unsafe_op_in_unsafe_fn)]

    use serde::{Deserialize, Serialize};
    use std::ffi::c_void;
    use std::fs;
    use std::mem::{size_of, zeroed};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::ptr::{null, null_mut};
    use uvie_rs_win::session::{Edit, SessionEngine};
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Registry::*;
    use windows_sys::Win32::System::Threading::*;
    use windows_sys::Win32::UI::Accessibility::*;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const APP_NAME: &str = "UVie Rust Win";
    const CLASS_NAME: &str = "UVieRustWinHiddenWindow";
    const MUTEX_NAME: &str = "Local\\UVieRustWinSingleInstance";
    const CONFIG_FILE: &str = "uvie-rs-win.json";
    const RUN_VALUE: &str = "UVieRustWin";
    const TASK_NAME: &str = "UVieRustWin";
    const TRAY_MESSAGE: u32 = WM_APP + 1;
    const WM_REFRESH_TRAY: u32 = WM_APP + 2;
    const MENU_TOGGLE: usize = 1001;
    const MENU_STARTUP: usize = 1002;
    const MENU_ADMIN: usize = 1003;
    const MENU_ABOUT: usize = 1004;
    const MENU_EXIT: usize = 1005;
    const TRAY_ID: u32 = 1;
    const INJECTED_MARKER: usize = 0x5556_4945;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Config {
        enabled: bool,
        run_at_startup: bool,
        run_as_admin: bool,
        hotkey: String,
        quick_telex: bool,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                enabled: true,
                run_at_startup: false,
                run_as_admin: false,
                hotkey: "Ctrl+Shift".to_string(),
                quick_telex: false,
            }
        }
    }

    struct App {
        instance: HINSTANCE,
        hwnd: HWND,
        mutex: HANDLE,
        keyboard_hook: HHOOK,
        mouse_hook: HHOOK,
        foreground_hook: HWINEVENTHOOK,
        taskbar_created: u32,
        tray: NOTIFYICONDATAW,
        menu: HMENU,
        icon_v: HICON,
        icon_e: HICON,
        config: Config,
        engine: SessionEngine,
        exe_path: PathBuf,
        config_path: PathBuf,
        ctrl_down: bool,
        shift_down: bool,
        alt_down: bool,
        win_down: bool,
        hotkey_armed: bool,
        hotkey_cancelled: bool,
    }

    static mut APP: *mut App = null_mut();

    pub fn main() {
        unsafe {
            let instance = GetModuleHandleW(null());
            let exe_path =
                std::env::current_exe().unwrap_or_else(|_| PathBuf::from("uvie-rs-win.exe"));
            let config_path = exe_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(CONFIG_FILE);
            let config = load_config(&config_path);

            if config.run_as_admin && !is_elevated() && relaunch(&exe_path, true) {
                return;
            }

            let mutex_name = wide(MUTEX_NAME);
            let mutex = CreateMutexW(null(), TRUE, mutex_name.as_ptr());
            if mutex.is_null() {
                return;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                message(
                    null_mut(),
                    "UVie Rust Win is already running.\n\nExit the tray app before testing a new build.",
                );
                CloseHandle(mutex);
                return;
            }

            let mut app = Box::new(App {
                instance,
                hwnd: null_mut(),
                mutex,
                keyboard_hook: null_mut(),
                mouse_hook: null_mut(),
                foreground_hook: null_mut(),
                taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                tray: zeroed(),
                menu: null_mut(),
                icon_v: create_letter_icon('V', rgb(20, 132, 82)),
                icon_e: create_letter_icon('E', rgb(95, 105, 120)),
                engine: SessionEngine::new(config.quick_telex),
                config,
                exe_path,
                config_path,
                ctrl_down: false,
                shift_down: false,
                alt_down: false,
                win_down: false,
                hotkey_armed: true,
                hotkey_cancelled: false,
            });

            APP = app.as_mut();
            if !create_hidden_window(&mut app) {
                return;
            }
            add_tray_icon(&mut app);
            save_config(&app);
            let _ = sync_startup(&app);

            if !install_hooks(&mut app) {
                message(app.hwnd, "Cannot install keyboard hooks.");
                cleanup(&mut app);
                return;
            }

            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            cleanup(&mut app);
            APP = null_mut();
        }
    }

    unsafe fn create_hidden_window(app: &mut App) -> bool {
        let class = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            hInstance: app.instance,
            lpszClassName: class.as_ptr(),
            hIcon: app.icon_v,
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return false;
        }
        app.hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(APP_NAME).as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            app.instance,
            null_mut(),
        );
        !app.hwnd.is_null()
    }

    unsafe fn install_hooks(app: &mut App) -> bool {
        app.keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), app.instance, 0);
        app.mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), app.instance, 0);
        app.foreground_hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(foreground_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        !app.keyboard_hook.is_null() && !app.mouse_hook.is_null() && !app.foreground_hook.is_null()
    }

    unsafe fn cleanup(app: &mut App) {
        if !app.foreground_hook.is_null() {
            UnhookWinEvent(app.foreground_hook);
        }
        if !app.mouse_hook.is_null() {
            UnhookWindowsHookEx(app.mouse_hook);
        }
        if !app.keyboard_hook.is_null() {
            UnhookWindowsHookEx(app.keyboard_hook);
        }
        Shell_NotifyIconW(NIM_DELETE, &app.tray);
        if !app.menu.is_null() {
            DestroyMenu(app.menu);
        }
        if !app.icon_v.is_null() {
            DestroyIcon(app.icon_v);
        }
        if !app.icon_e.is_null() {
            DestroyIcon(app.icon_e);
        }
        if !app.mutex.is_null() {
            CloseHandle(app.mutex);
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let app = &mut *APP;
        if msg == app.taskbar_created {
            add_tray_icon(app);
            update_tray(app);
            return 0;
        }
        match msg {
            TRAY_MESSAGE => {
                match lparam as u32 {
                    WM_LBUTTONUP => toggle_enabled(app),
                    WM_RBUTTONUP | WM_CONTEXTMENU => popup_menu(app),
                    _ => {}
                }
                0
            }
            WM_REFRESH_TRAY => {
                update_tray(app);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code != HC_ACTION as i32 {
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        let app = &mut *APP;
        let k = &*(lparam as *const KBDLLHOOKSTRUCT);
        if (k.flags & LLKHF_INJECTED) != 0 || k.dwExtraInfo == INJECTED_MARKER {
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }

        let msg = wparam as u32;
        let was_ctrl_shift = app.ctrl_down && app.shift_down && !app.alt_down && !app.win_down;
        update_modifier_state(app, k.vkCode, msg);

        if msg == WM_KEYUP || msg == WM_SYSKEYUP {
            if was_ctrl_shift
                && is_ctrl_shift_modifier(k.vkCode)
                && app.hotkey_armed
                && !app.hotkey_cancelled
            {
                toggle_enabled(app);
                app.hotkey_armed = false;
            }
            if !app.ctrl_down && !app.shift_down {
                app.hotkey_armed = true;
                app.hotkey_cancelled = false;
            }
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }
        if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }

        if was_ctrl_shift && !is_ctrl_shift_modifier(k.vkCode) {
            app.hotkey_cancelled = true;
        }

        if !app.config.enabled {
            app.engine.reset();
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }
        if app.ctrl_down || app.alt_down || app.win_down {
            app.engine.reset();
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }

        if k.vkCode == VK_ESCAPE as u32 {
            app.engine.reset();
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }
        if k.vkCode == VK_BACK as u32 && app.engine.is_composing() {
            let edit = app.engine.backspace_visible();
            apply_edit(app, edit);
            return 1;
        }

        let Some(ch) = vk_to_ascii(app, k.vkCode, k.scanCode) else {
            app.engine.reset();
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        };

        if is_break_char(ch) {
            app.engine.reset();
            return CallNextHookEx(app.keyboard_hook, code, wparam, lparam);
        }

        match app.engine.feed(ch) {
            Edit::Pass => CallNextHookEx(app.keyboard_hook, code, wparam, lparam),
            edit => {
                apply_edit(app, edit);
                1
            }
        }
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32
            && matches!(
                wparam as u32,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            )
        {
            let app = &mut *APP;
            refresh_modifier_state(app);
            app.engine.reset();
        }
        CallNextHookEx(null_mut(), code, wparam, lparam)
    }

    unsafe extern "system" fn foreground_proc(
        _: HWINEVENTHOOK,
        _: u32,
        _: HWND,
        _: i32,
        _: i32,
        _: u32,
        _: u32,
    ) {
        if !APP.is_null() {
            let app = &mut *APP;
            refresh_modifier_state(app);
            app.engine.reset();
        }
    }

    unsafe fn apply_edit(app: &mut App, edit: Edit) {
        match edit {
            Edit::Pass => {}
            Edit::Replace { backspaces, text } => {
                if !send_replacement(backspaces, &text) {
                    app.engine.reset();
                }
            }
        }
    }

    unsafe fn send_replacement(backspaces: usize, text: &str) -> bool {
        let mut inputs = Vec::with_capacity((backspaces + text.encode_utf16().count()) * 2 + 2);
        // ponytail: this treats all Thorium text fields as Omnibox-like. Upgrade
        // to UI Automation focused-control detection if mid-text edits regress.
        if backspaces > 0 && is_foreground_process("thorium.exe") {
            append_key(&mut inputs, VK_DELETE);
        }
        append_backspaces(&mut inputs, backspaces);
        append_text(&mut inputs, text);
        send_inputs(&mut inputs)
    }

    fn append_backspaces(inputs: &mut Vec<INPUT>, count: usize) {
        for _ in 0..count {
            let down = keyboard_input(VK_BACK, 0, 0);
            let up = keyboard_input(VK_BACK, 0, KEYEVENTF_KEYUP);
            inputs.push(down);
            inputs.push(up);
        }
    }

    fn append_key(inputs: &mut Vec<INPUT>, vk: VIRTUAL_KEY) {
        inputs.push(keyboard_input(vk, 0, 0));
        inputs.push(keyboard_input(vk, 0, KEYEVENTF_KEYUP));
    }

    fn append_text(inputs: &mut Vec<INPUT>, text: &str) {
        for unit in text.encode_utf16() {
            let down = keyboard_input(0, unit, KEYEVENTF_UNICODE);
            let up = keyboard_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
            inputs.push(down);
            inputs.push(up);
        }
    }

    fn keyboard_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        let mut input: INPUT = unsafe { zeroed() };
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = vk;
        input.Anonymous.ki.wScan = scan;
        input.Anonymous.ki.dwFlags = flags;
        input.Anonymous.ki.dwExtraInfo = INJECTED_MARKER;
        input
    }

    unsafe fn send_inputs(inputs: &mut [INPUT]) -> bool {
        if inputs.is_empty() {
            return true;
        }
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        ) == inputs.len() as u32
    }

    unsafe fn current_keyboard_layout() -> HKL {
        let hwnd = GetForegroundWindow();
        if !hwnd.is_null() {
            let thread_id = GetWindowThreadProcessId(hwnd, null_mut());
            if thread_id != 0 {
                return GetKeyboardLayout(thread_id);
            }
        }
        GetKeyboardLayout(0)
    }

    unsafe fn is_foreground_process(name: &str) -> bool {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return false;
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if process.is_null() {
            return false;
        }

        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(process);
        if !ok || len == 0 {
            return false;
        }

        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/'])
            .next()
            .is_some_and(|exe| exe.eq_ignore_ascii_case(name))
    }

    unsafe fn vk_to_ascii(app: &App, vk: u32, scan: u32) -> Option<char> {
        let mut state = [0u8; 256];
        if GetKeyboardState(state.as_mut_ptr()) == 0 {
            return None;
        }
        state[VK_SHIFT as usize] = if app.shift_down { 0x80 } else { 0 };
        state[VK_CONTROL as usize] = if app.ctrl_down { 0x80 } else { 0 };
        state[VK_MENU as usize] = if app.alt_down { 0x80 } else { 0 };

        let layout = current_keyboard_layout();
        let mut buf = [0u16; 8];
        let n = ToUnicodeEx(vk, scan, state.as_ptr(), buf.as_mut_ptr(), 7, 0, layout);
        if n != 1 {
            return None;
        }
        char::from_u32(buf[0] as u32).filter(|c| c.is_ascii_graphic() || *c == ' ')
    }

    fn is_break_char(ch: char) -> bool {
        ch.is_whitespace()
            || matches!(
                ch,
                '.' | ','
                    | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '/'
                    | '\\'
                    | '"'
                    | '\''
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '-'
                    | '+'
                    | '='
            )
    }

    fn update_modifier_state(app: &mut App, vk: u32, msg: u32) {
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if !down && !up {
            return;
        }
        match vk {
            x if x == VK_CONTROL as u32 || x == VK_LCONTROL as u32 || x == VK_RCONTROL as u32 => {
                app.ctrl_down = down
            }
            x if x == VK_SHIFT as u32 || x == VK_LSHIFT as u32 || x == VK_RSHIFT as u32 => {
                app.shift_down = down
            }
            x if x == VK_MENU as u32 || x == VK_LMENU as u32 || x == VK_RMENU as u32 => {
                app.alt_down = down
            }
            x if x == VK_LWIN as u32 || x == VK_RWIN as u32 => app.win_down = down,
            _ => {}
        }
    }

    unsafe fn refresh_modifier_state(app: &mut App) {
        app.ctrl_down = (GetAsyncKeyState(VK_CONTROL as i32) & 0x8000u16 as i16) != 0;
        app.shift_down = (GetAsyncKeyState(VK_SHIFT as i32) & 0x8000u16 as i16) != 0;
        app.alt_down = (GetAsyncKeyState(VK_MENU as i32) & 0x8000u16 as i16) != 0;
        app.win_down = (GetAsyncKeyState(VK_LWIN as i32) & 0x8000u16 as i16) != 0
            || (GetAsyncKeyState(VK_RWIN as i32) & 0x8000u16 as i16) != 0;
    }

    fn is_ctrl_shift_modifier(vk: u32) -> bool {
        vk == VK_CONTROL as u32
            || vk == VK_LCONTROL as u32
            || vk == VK_RCONTROL as u32
            || vk == VK_SHIFT as u32
            || vk == VK_LSHIFT as u32
            || vk == VK_RSHIFT as u32
    }

    unsafe fn toggle_enabled(app: &mut App) {
        app.config.enabled = !app.config.enabled;
        app.engine.reset();
        save_config(app);
        update_tray(app);
    }

    unsafe fn popup_menu(app: &mut App) {
        if !app.menu.is_null() {
            DestroyMenu(app.menu);
        }
        app.menu = CreatePopupMenu();
        AppendMenuW(
            app.menu,
            MF_STRING
                | if app.config.enabled {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                },
            MENU_TOGGLE,
            wide("Bật Tiếng Việt").as_ptr(),
        );
        AppendMenuW(
            app.menu,
            MF_STRING
                | if app.config.run_at_startup {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                },
            MENU_STARTUP,
            wide("Chạy cùng Windows").as_ptr(),
        );
        AppendMenuW(
            app.menu,
            MF_STRING
                | if app.config.run_as_admin {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                },
            MENU_ADMIN,
            wide("Chạy bằng Admin").as_ptr(),
        );
        AppendMenuW(app.menu, MF_SEPARATOR, 0, null());
        AppendMenuW(app.menu, MF_STRING, MENU_ABOUT, wide("Giới thiệu").as_ptr());
        AppendMenuW(app.menu, MF_SEPARATOR, 0, null());
        AppendMenuW(app.menu, MF_STRING, MENU_EXIT, wide("Thoát").as_ptr());

        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        SetForegroundWindow(app.hwnd);
        let cmd = TrackPopupMenu(
            app.menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            app.hwnd,
            null(),
        );
        PostMessageW(app.hwnd, WM_NULL, 0, 0);

        match cmd as usize {
            MENU_TOGGLE => toggle_enabled(app),
            MENU_STARTUP => {
                let old = app.config.run_at_startup;
                app.config.run_at_startup = !old;
                if sync_startup(app) {
                    save_config(app);
                } else {
                    app.config.run_at_startup = old;
                    let _ = sync_startup(app);
                    message(app.hwnd, "Không cập nhật được startup.");
                }
                update_tray(app);
            }
            MENU_ADMIN => toggle_admin(app),
            MENU_ABOUT => message(
                app.hwnd,
                "UVie Rust Win\n\nTelex-only tray app.\nHotkey: Ctrl+Shift",
            ),
            MENU_EXIT => PostQuitMessage(0),
            _ => {}
        }
    }

    unsafe fn toggle_admin(app: &mut App) {
        let old = app.config.run_as_admin;
        app.config.run_as_admin = !old;
        save_config(app);
        let _ = sync_startup(app);

        if relaunch(&app.exe_path, app.config.run_as_admin) {
            PostQuitMessage(0);
            return;
        }

        app.config.run_as_admin = old;
        save_config(app);
        let _ = sync_startup(app);
        update_tray(app);
    }

    fn load_config(path: &PathBuf) -> Config {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_config(app: &App) {
        if let Ok(json) = serde_json::to_string_pretty(&app.config) {
            let _ = fs::write(&app.config_path, json);
        }
    }

    fn sync_startup(app: &App) -> bool {
        if !app.config.run_at_startup {
            let ok = set_hkcu_run(false, &app.exe_path);
            let _ = delete_task();
            return ok;
        }
        if app.config.run_as_admin {
            let _ = set_hkcu_run(false, &app.exe_path);
            set_task(&app.exe_path)
        } else {
            let _ = delete_task();
            set_hkcu_run(true, &app.exe_path)
        }
    }

    fn set_hkcu_run(enabled: bool, exe: &Path) -> bool {
        unsafe {
            let mut key = null_mut();
            let path = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                null_mut(),
                0,
                KEY_SET_VALUE,
                null(),
                &mut key,
                null_mut(),
            ) != ERROR_SUCCESS
            {
                return false;
            }
            let name = wide(RUN_VALUE);
            let ok = if enabled {
                let value = wide(&format!("\"{}\"", exe.display()));
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr() as *const u8,
                    (value.len() * 2) as u32,
                ) == ERROR_SUCCESS
            } else {
                let rc = RegDeleteValueW(key, name.as_ptr());
                rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND
            };
            RegCloseKey(key);
            ok
        }
    }

    fn set_task(exe: &Path) -> bool {
        Command::new("schtasks")
            .args([
                "/Create",
                "/SC",
                "ONLOGON",
                "/TN",
                TASK_NAME,
                "/TR",
                &format!("\"{}\"", exe.display()),
                "/RL",
                "HIGHEST",
                "/F",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn delete_task() -> bool {
        Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|s| s.success())
            .unwrap_or(true)
    }

    unsafe fn relaunch(exe: &Path, admin: bool) -> bool {
        let exe_w = wide(&exe.to_string_lossy());
        if admin {
            ShellExecuteW(
                null_mut(),
                wide("runas").as_ptr(),
                exe_w.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            ) as isize
                > 32
        } else {
            let arg = wide(&format!("\"{}\"", exe.display()));
            ShellExecuteW(
                null_mut(),
                wide("open").as_ptr(),
                wide("explorer.exe").as_ptr(),
                arg.as_ptr(),
                null(),
                SW_SHOWNORMAL,
            ) as isize
                > 32
        }
    }

    unsafe fn is_elevated() -> bool {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: TOKEN_ELEVATION = zeroed();
        let mut len = size_of::<TOKEN_ELEVATION>() as u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            len,
            &mut len,
        ) != 0;
        CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }

    unsafe fn add_tray_icon(app: &mut App) {
        app.tray = zeroed();
        app.tray.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        app.tray.hWnd = app.hwnd;
        app.tray.uID = TRAY_ID;
        app.tray.uCallbackMessage = TRAY_MESSAGE;
        app.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        app.tray.hIcon = current_icon(app);
        set_tip(app);
        Shell_NotifyIconW(NIM_ADD, &app.tray);
    }

    unsafe fn update_tray(app: &mut App) {
        app.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        app.tray.hIcon = current_icon(app);
        set_tip(app);
        Shell_NotifyIconW(NIM_MODIFY, &app.tray);
    }

    fn current_icon(app: &App) -> HICON {
        if app.config.enabled {
            app.icon_v
        } else {
            app.icon_e
        }
    }

    fn set_tip(app: &mut App) {
        let tip = if app.config.enabled {
            "UVie Rust: Vietnamese"
        } else {
            "UVie Rust: English"
        };
        copy_wide_fixed(&mut app.tray.szTip, tip);
    }

    unsafe fn create_letter_icon(letter: char, bg: COLORREF) -> HICON {
        let screen = GetDC(null_mut());
        let dc = CreateCompatibleDC(screen);
        let color = CreateCompatibleBitmap(screen, 32, 32);
        let old = SelectObject(dc, color as _);
        let brush = CreateSolidBrush(bg);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 32,
            bottom: 32,
        };
        FillRect(dc, &rect, brush);
        DeleteObject(brush as _);
        let font = CreateFontW(
            23,
            0,
            0,
            0,
            FW_BOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            wide("Segoe UI").as_ptr(),
        );
        let old_font = SelectObject(dc, font as _);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, rgb(255, 255, 255));
        let txt = wide(&letter.to_string());
        DrawTextW(
            dc,
            txt.as_ptr(),
            1,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(dc, old_font);
        DeleteObject(font as _);
        SelectObject(dc, old);
        DeleteDC(dc);
        ReleaseDC(null_mut(), screen);
        let mask = CreateBitmap(32, 32, 1, 1, null());
        let info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info);
        DeleteObject(mask as _);
        DeleteObject(color as _);
        icon
    }

    unsafe fn message(hwnd: HWND, text: &str) {
        MessageBoxW(
            hwnd,
            wide(text).as_ptr(),
            wide(APP_NAME).as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }

    fn copy_wide_fixed<const N: usize>(dst: &mut [u16; N], src: &str) {
        dst.fill(0);
        for (i, unit) in src.encode_utf16().take(N.saturating_sub(1)).enumerate() {
            dst[i] = unit;
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
        r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
    }

    trait CommandExtHidden {
        fn creation_flags(&mut self, flags: u32) -> &mut Self;
    }

    impl CommandExtHidden for Command {
        fn creation_flags(&mut self, flags: u32) -> &mut Self {
            use std::os::windows::process::CommandExt;
            CommandExt::creation_flags(self, flags)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn backspace_inputs_use_virtual_key_and_marker() {
            let mut inputs = Vec::new();
            append_backspaces(&mut inputs, 1);

            assert_eq!(inputs.len(), 2);
            assert_eq!(inputs[0].r#type, INPUT_KEYBOARD);
            unsafe {
                assert_eq!(inputs[0].Anonymous.ki.wVk, VK_BACK);
                assert_eq!(inputs[0].Anonymous.ki.wScan, 0);
                assert_eq!(inputs[0].Anonymous.ki.dwFlags, 0);
                assert_eq!(inputs[0].Anonymous.ki.dwExtraInfo, INJECTED_MARKER);
                assert_eq!(inputs[1].Anonymous.ki.wVk, VK_BACK);
                assert_eq!(inputs[1].Anonymous.ki.wScan, 0);
                assert_eq!(inputs[1].Anonymous.ki.dwFlags, KEYEVENTF_KEYUP);
                assert_eq!(inputs[1].Anonymous.ki.dwExtraInfo, INJECTED_MARKER);
            }
        }

        #[test]
        fn thorium_replacement_clears_inline_selection_first() {
            let mut inputs = Vec::new();
            append_key(&mut inputs, VK_DELETE);
            append_backspaces(&mut inputs, 1);
            append_text(&mut inputs, "â");

            assert_eq!(inputs.len(), 6);
            unsafe {
                assert_eq!(inputs[0].Anonymous.ki.wVk, VK_DELETE);
                assert_eq!(inputs[1].Anonymous.ki.wVk, VK_DELETE);
                assert_eq!(inputs[2].Anonymous.ki.wVk, VK_BACK);
                assert_eq!(inputs[3].Anonymous.ki.wVk, VK_BACK);
                assert_eq!(inputs[4].Anonymous.ki.wScan, 'â' as u16);
                assert_eq!(inputs[4].Anonymous.ki.dwFlags, KEYEVENTF_UNICODE);
            }
        }

        #[test]
        fn text_inputs_use_unicode_and_marker() {
            let mut inputs = Vec::new();
            append_text(&mut inputs, "â");

            assert_eq!(inputs.len(), 2);
            assert_eq!(inputs[0].r#type, INPUT_KEYBOARD);
            unsafe {
                assert_eq!(inputs[0].Anonymous.ki.wVk, 0);
                assert_eq!(inputs[0].Anonymous.ki.wScan, 'â' as u16);
                assert_eq!(inputs[0].Anonymous.ki.dwFlags, KEYEVENTF_UNICODE);
                assert_eq!(inputs[0].Anonymous.ki.dwExtraInfo, INJECTED_MARKER);
                assert_eq!(
                    inputs[1].Anonymous.ki.dwFlags,
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                );
                assert_eq!(inputs[1].Anonymous.ki.dwExtraInfo, INJECTED_MARKER);
            }
        }
    }
}

#[cfg(windows)]
fn main() {
    win::main();
}
