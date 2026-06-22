use crate::context::AppContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionStrategy {
    BackspaceText,
    DeleteThenBackspaceText,
    EmptyPrefixBackspaceText,
    SlowBackspaceText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppProfile {
    pub strategy: InjectionStrategy,
}

impl Default for AppProfile {
    fn default() -> Self {
        Self {
            strategy: InjectionStrategy::BackspaceText,
        }
    }
}

pub fn resolve_profile(context: &AppContext) -> AppProfile {
    let Some(exe) = context.exe_name.as_deref() else {
        return AppProfile::default();
    };

    let strategy = if is_browser_exe(exe) {
        InjectionStrategy::DeleteThenBackspaceText
    } else {
        InjectionStrategy::BackspaceText
    };

    AppProfile { strategy }
}

fn is_browser_exe(exe: &str) -> bool {
    matches!(
        exe.to_ascii_lowercase().as_str(),
        "thorium.exe" | "chrome.exe" | "msedge.exe" | "brave.exe"
    )
}

#[cfg(test)]
mod tests {
    use super::{InjectionStrategy, resolve_profile};
    use crate::context::AppContext;

    fn context(exe_name: Option<&str>) -> AppContext {
        AppContext {
            exe_name: exe_name.map(str::to_string),
            window_title: None,
        }
    }

    #[test]
    fn missing_exe_uses_default_strategy() {
        assert_eq!(
            resolve_profile(&context(None)).strategy,
            InjectionStrategy::BackspaceText
        );
    }

    #[test]
    fn unknown_exe_uses_default_strategy() {
        assert_eq!(
            resolve_profile(&context(Some("notepad.exe"))).strategy,
            InjectionStrategy::BackspaceText
        );
    }

    #[test]
    fn browser_exe_uses_autocomplete_strategy() {
        for exe in ["thorium.exe", "chrome.exe", "msedge.exe", "brave.exe"] {
            assert_eq!(
                resolve_profile(&context(Some(exe))).strategy,
                InjectionStrategy::DeleteThenBackspaceText,
                "{exe}"
            );
        }
    }

    #[test]
    fn browser_matching_is_case_insensitive() {
        assert_eq!(
            resolve_profile(&context(Some("Thorium.EXE"))).strategy,
            InjectionStrategy::DeleteThenBackspaceText
        );
    }
}
