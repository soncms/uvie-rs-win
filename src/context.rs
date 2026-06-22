#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppContext {
    pub exe_name: Option<String>,
    pub window_title: Option<String>,
    pub class_name: Option<String>,
    pub focus: Option<FocusInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusInfo {
    pub control_type: Option<String>,
    pub name: Option<String>,
    pub automation_id: Option<String>,
    pub class_name: Option<String>,
    pub framework_id: Option<String>,
    pub localized_control_type: Option<String>,
    pub process_id: Option<u32>,
    pub has_keyboard_focus: Option<bool>,
    pub value_available: bool,
    pub is_password: bool,
}

#[cfg(windows)]
pub fn current_app_context() -> AppContext {
    use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId,
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

    let mut class_buf = [0u16; 256];
    let class_len = unsafe { GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32) };
    let class_name =
        (class_len > 0).then(|| String::from_utf16_lossy(&class_buf[..class_len as usize]));

    AppContext {
        exe_name,
        window_title,
        class_name,
        focus: current_focus_info(),
    }
}

#[cfg(windows)]
fn current_focus_info() -> Option<FocusInfo> {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, UIA_ComboBoxControlTypeId, UIA_DocumentControlTypeId,
        UIA_EditControlTypeId, UIA_PaneControlTypeId, UIA_TextControlTypeId, UIA_ValuePatternId,
    };

    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let element = automation.GetFocusedElement().ok()?;
            let control_type_id = element.CurrentControlType().ok();
            let control_type = control_type_id.map(|id| {
                if id == UIA_EditControlTypeId {
                    "Edit"
                } else if id == UIA_DocumentControlTypeId {
                    "Document"
                } else if id == UIA_TextControlTypeId {
                    "Text"
                } else if id == UIA_ComboBoxControlTypeId {
                    "ComboBox"
                } else if id == UIA_PaneControlTypeId {
                    "Pane"
                } else {
                    "Other"
                }
                .to_string()
            });

            Some(FocusInfo {
                control_type,
                name: bstr_to_string(element.CurrentName().ok()),
                automation_id: bstr_to_string(element.CurrentAutomationId().ok()),
                class_name: bstr_to_string(element.CurrentClassName().ok()),
                framework_id: bstr_to_string(element.CurrentFrameworkId().ok()),
                localized_control_type: bstr_to_string(element.CurrentLocalizedControlType().ok()),
                process_id: element.CurrentProcessId().ok().map(|id| id as u32),
                has_keyboard_focus: element
                    .CurrentHasKeyboardFocus()
                    .ok()
                    .map(|value| value.as_bool()),
                value_available: element.GetCurrentPattern(UIA_ValuePatternId).is_ok(),
                is_password: element
                    .CurrentIsPassword()
                    .map(|value| value.as_bool())
                    .unwrap_or(false),
            })
        })();
        if initialized {
            CoUninitialize();
        }
        result
    }
}

#[cfg(windows)]
fn bstr_to_string(value: Option<windows::core::BSTR>) -> Option<String> {
    let value = value?;
    let text = value.to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(not(windows))]
pub fn current_app_context() -> AppContext {
    AppContext::default()
}
