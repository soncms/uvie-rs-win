use crate::context::{AppContext, FocusInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum InjectionStrategy {
    BackspaceText,
    DeleteThenBackspaceText,
    EmptyPrefixBackspaceText,
    SlowBackspaceText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AppBehavior {
    PlainEditor,
    BrowserTextField,
    BrowserAddressBar,
    TerminalLike,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileReason {
    MissingExe,
    Rule,
    BuiltInRule,
    PasswordField,
    BrowserFocusAddressBar,
    BrowserFocusTextField,
    BrowserFallbackAddressBar,
    TerminalExe,
    PlainEditorExe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileConfidence {
    Certain,
    Likely,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppProfile {
    pub behavior: AppBehavior,
    pub strategy: InjectionStrategy,
    pub fallback_strategies: Vec<InjectionStrategy>,
    pub reason: ProfileReason,
    pub confidence: ProfileConfidence,
    pub rule_name: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProfileRule {
    pub name: Option<String>,
    #[serde(default = "default_rule_enabled")]
    pub enabled: bool,
    pub exe: Option<String>,
    pub title_contains: Option<String>,
    pub class_contains: Option<String>,
    pub focus_name_contains: Option<String>,
    pub automation_id_contains: Option<String>,
    pub focus_class_contains: Option<String>,
    pub behavior: Option<AppBehavior>,
    pub strategy: Option<InjectionStrategy>,
    pub fallback_strategies: Vec<InjectionStrategy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuiltInRule {
    name: &'static str,
    exe: Option<&'static str>,
    title_contains: Option<&'static str>,
    class_contains: Option<&'static str>,
    focus_name_contains: Option<&'static str>,
    automation_id_contains: Option<&'static str>,
    focus_class_contains: Option<&'static str>,
    behavior: AppBehavior,
    strategy: Option<InjectionStrategy>,
    fallback_strategies: &'static [InjectionStrategy],
}

impl Default for AppProfile {
    fn default() -> Self {
        Self {
            behavior: AppBehavior::Unknown,
            strategy: InjectionStrategy::BackspaceText,
            fallback_strategies: Vec::new(),
            reason: ProfileReason::MissingExe,
            confidence: ProfileConfidence::Fallback,
            rule_name: None,
        }
    }
}

impl Default for ProfileRule {
    fn default() -> Self {
        Self {
            name: None,
            enabled: true,
            exe: None,
            title_contains: None,
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: None,
            strategy: None,
            fallback_strategies: Vec::new(),
        }
    }
}

impl AppProfile {
    pub fn strategy_chain(&self) -> Vec<InjectionStrategy> {
        let mut strategies = vec![self.strategy];
        for strategy in &self.fallback_strategies {
            if !strategies.contains(strategy) {
                strategies.push(*strategy);
            }
        }
        strategies
    }
}

pub fn resolve_profile(context: &AppContext) -> AppProfile {
    resolve_profile_with_rules(context, &[])
}

pub fn resolve_profile_with_rules(context: &AppContext, rules: &[ProfileRule]) -> AppProfile {
    if let Some(profile) = resolve_rule_profile(context, rules) {
        return profile;
    }
    if let Some(profile) = resolve_builtin_rule_profile(context) {
        return profile;
    }
    resolve_builtin_profile(context)
}

pub fn is_excluded_app(context: &AppContext, excluded_apps: &[String]) -> bool {
    let Some(exe) = context.exe_name.as_deref() else {
        return false;
    };
    excluded_apps
        .iter()
        .any(|excluded| exe.eq_ignore_ascii_case(excluded))
}

pub fn is_password_context(context: &AppContext) -> bool {
    context
        .focus
        .as_ref()
        .map(|focus| focus.is_password)
        .unwrap_or(false)
}

fn resolve_rule_profile(context: &AppContext, rules: &[ProfileRule]) -> Option<AppProfile> {
    let base = resolve_builtin_profile(context);
    let rule = rules
        .iter()
        .find(|rule| rule.enabled && rule_matches(context, rule))?;
    let behavior = rule.behavior.unwrap_or(base.behavior);
    let strategy = rule
        .strategy
        .unwrap_or_else(|| strategy_for_behavior(behavior));
    Some(AppProfile {
        behavior,
        strategy,
        fallback_strategies: rule.fallback_strategies.clone(),
        reason: ProfileReason::Rule,
        confidence: ProfileConfidence::Certain,
        rule_name: None,
    })
}

fn resolve_builtin_rule_profile(context: &AppContext) -> Option<AppProfile> {
    let rule = built_in_rules()
        .iter()
        .find(|rule| built_in_rule_matches(context, rule))?;
    let behavior = rule.behavior;
    Some(AppProfile {
        behavior,
        strategy: rule
            .strategy
            .unwrap_or_else(|| strategy_for_behavior(behavior)),
        fallback_strategies: rule.fallback_strategies.to_vec(),
        reason: ProfileReason::BuiltInRule,
        confidence: ProfileConfidence::Certain,
        rule_name: Some(rule.name),
    })
}

fn resolve_builtin_profile(context: &AppContext) -> AppProfile {
    let Some(exe) = context.exe_name.as_deref() else {
        return AppProfile::default();
    };

    if is_password_context(context) {
        return AppProfile {
            behavior: AppBehavior::Unknown,
            strategy: InjectionStrategy::BackspaceText,
            fallback_strategies: Vec::new(),
            reason: ProfileReason::PasswordField,
            confidence: ProfileConfidence::Certain,
            rule_name: None,
        };
    }

    let (behavior, reason, confidence) = if is_browser_exe(exe) {
        browser_behavior(context.focus.as_ref())
    } else if is_terminal_exe(exe) {
        (
            AppBehavior::TerminalLike,
            ProfileReason::TerminalExe,
            ProfileConfidence::Certain,
        )
    } else {
        (
            AppBehavior::PlainEditor,
            ProfileReason::PlainEditorExe,
            ProfileConfidence::Likely,
        )
    };

    AppProfile {
        behavior,
        strategy: strategy_for_behavior(behavior),
        fallback_strategies: fallback_strategies_for_behavior(behavior),
        reason,
        confidence,
        rule_name: None,
    }
}

fn browser_behavior(focus: Option<&FocusInfo>) -> (AppBehavior, ProfileReason, ProfileConfidence) {
    let Some(focus) = focus else {
        return (
            AppBehavior::BrowserAddressBar,
            ProfileReason::BrowserFallbackAddressBar,
            ProfileConfidence::Fallback,
        );
    };

    if focus_looks_like_address_bar(focus) {
        return (
            AppBehavior::BrowserAddressBar,
            ProfileReason::BrowserFocusAddressBar,
            ProfileConfidence::Certain,
        );
    }

    if focus_looks_like_text_field(focus) {
        return (
            AppBehavior::BrowserTextField,
            ProfileReason::BrowserFocusTextField,
            ProfileConfidence::Likely,
        );
    }

    (
        AppBehavior::BrowserAddressBar,
        ProfileReason::BrowserFallbackAddressBar,
        ProfileConfidence::Fallback,
    )
}

fn focus_looks_like_address_bar(focus: &FocusInfo) -> bool {
    ["address", "omnibox", "search", "url"]
        .iter()
        .any(|needle| focus_contains(focus, needle))
        || is_chrome_edit_without_value_pattern(focus)
}

fn focus_looks_like_text_field(focus: &FocusInfo) -> bool {
    matches!(
        focus.control_type.as_deref(),
        Some("Edit" | "Document" | "Text")
    ) || focus.value_available
}

fn is_chrome_edit_without_value_pattern(focus: &FocusInfo) -> bool {
    matches!(focus.control_type.as_deref(), Some("Edit" | "ComboBox")) && !focus.value_available
}

fn focus_contains(focus: &FocusInfo, needle: &str) -> bool {
    contains_case_insensitive(focus.name.as_deref(), needle)
        || contains_case_insensitive(focus.automation_id.as_deref(), needle)
        || contains_case_insensitive(focus.class_name.as_deref(), needle)
}

fn rule_matches(context: &AppContext, rule: &ProfileRule) -> bool {
    if rule.exe.is_none()
        && rule.title_contains.is_none()
        && rule.class_contains.is_none()
        && rule.focus_name_contains.is_none()
        && rule.automation_id_contains.is_none()
        && rule.focus_class_contains.is_none()
    {
        return false;
    }
    if let Some(exe) = rule.exe.as_deref()
        && !context
            .exe_name
            .as_deref()
            .map(|current| current.eq_ignore_ascii_case(exe))
            .unwrap_or(false)
    {
        return false;
    }
    if let Some(title) = rule.title_contains.as_deref()
        && !contains_case_insensitive(context.window_title.as_deref(), title)
    {
        return false;
    }
    if let Some(class) = rule.class_contains.as_deref()
        && !contains_case_insensitive(context.class_name.as_deref(), class)
    {
        return false;
    }
    if let Some(name) = rule.focus_name_contains.as_deref()
        && !contains_case_insensitive(
            context
                .focus
                .as_ref()
                .and_then(|focus| focus.name.as_deref()),
            name,
        )
    {
        return false;
    }
    if let Some(automation_id) = rule.automation_id_contains.as_deref()
        && !contains_case_insensitive(
            context
                .focus
                .as_ref()
                .and_then(|focus| focus.automation_id.as_deref()),
            automation_id,
        )
    {
        return false;
    }
    if let Some(class) = rule.focus_class_contains.as_deref()
        && !contains_case_insensitive(
            context
                .focus
                .as_ref()
                .and_then(|focus| focus.class_name.as_deref()),
            class,
        )
    {
        return false;
    }
    true
}

fn built_in_rule_matches(context: &AppContext, rule: &BuiltInRule) -> bool {
    field_matches_exact(context.exe_name.as_deref(), rule.exe)
        && field_contains(context.window_title.as_deref(), rule.title_contains)
        && field_contains(context.class_name.as_deref(), rule.class_contains)
        && field_contains(
            context
                .focus
                .as_ref()
                .and_then(|focus| focus.name.as_deref()),
            rule.focus_name_contains,
        )
        && field_contains(
            context
                .focus
                .as_ref()
                .and_then(|focus| focus.automation_id.as_deref()),
            rule.automation_id_contains,
        )
        && field_contains(
            context
                .focus
                .as_ref()
                .and_then(|focus| focus.class_name.as_deref()),
            rule.focus_class_contains,
        )
}

fn field_matches_exact(value: Option<&str>, expected: Option<&str>) -> bool {
    expected
        .map(|expected| {
            value
                .map(|value| value.eq_ignore_ascii_case(expected))
                .unwrap_or(false)
        })
        .unwrap_or(true)
}

fn field_contains(value: Option<&str>, needle: Option<&str>) -> bool {
    needle
        .map(|needle| contains_case_insensitive(value, needle))
        .unwrap_or(true)
}

fn contains_case_insensitive(value: Option<&str>, needle: &str) -> bool {
    value
        .map(|value| {
            value
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase())
        })
        .unwrap_or(false)
}

fn strategy_for_behavior(behavior: AppBehavior) -> InjectionStrategy {
    match behavior {
        AppBehavior::PlainEditor | AppBehavior::BrowserTextField | AppBehavior::Unknown => {
            InjectionStrategy::BackspaceText
        }
        AppBehavior::BrowserAddressBar => InjectionStrategy::DeleteThenBackspaceText,
        AppBehavior::TerminalLike => InjectionStrategy::SlowBackspaceText,
    }
}

fn fallback_strategies_for_behavior(behavior: AppBehavior) -> Vec<InjectionStrategy> {
    match behavior {
        AppBehavior::BrowserAddressBar => vec![InjectionStrategy::EmptyPrefixBackspaceText],
        AppBehavior::TerminalLike => vec![InjectionStrategy::BackspaceText],
        _ => Vec::new(),
    }
}

fn is_browser_exe(exe: &str) -> bool {
    matches!(
        exe.to_ascii_lowercase().as_str(),
        "thorium.exe" | "chrome.exe" | "msedge.exe" | "brave.exe"
    )
}

fn is_terminal_exe(exe: &str) -> bool {
    matches!(
        exe.to_ascii_lowercase().as_str(),
        "windowsterminal.exe" | "wt.exe" | "cmd.exe" | "powershell.exe" | "pwsh.exe"
    )
}

fn built_in_rules() -> &'static [BuiltInRule] {
    const RULES: &[BuiltInRule] = &[
        BuiltInRule {
            name: "builtin:chrome-google-docs",
            exe: Some("chrome.exe"),
            title_contains: Some("Google Docs"),
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: AppBehavior::BrowserTextField,
            strategy: Some(InjectionStrategy::SlowBackspaceText),
            fallback_strategies: &[InjectionStrategy::BackspaceText],
        },
        BuiltInRule {
            name: "builtin:edge-google-docs",
            exe: Some("msedge.exe"),
            title_contains: Some("Google Docs"),
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: AppBehavior::BrowserTextField,
            strategy: Some(InjectionStrategy::SlowBackspaceText),
            fallback_strategies: &[InjectionStrategy::BackspaceText],
        },
        BuiltInRule {
            name: "builtin:thorium-google-docs",
            exe: Some("thorium.exe"),
            title_contains: Some("Google Docs"),
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: AppBehavior::BrowserTextField,
            strategy: Some(InjectionStrategy::SlowBackspaceText),
            fallback_strategies: &[InjectionStrategy::BackspaceText],
        },
        BuiltInRule {
            name: "builtin:brave-google-docs",
            exe: Some("brave.exe"),
            title_contains: Some("Google Docs"),
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: AppBehavior::BrowserTextField,
            strategy: Some(InjectionStrategy::SlowBackspaceText),
            fallback_strategies: &[InjectionStrategy::BackspaceText],
        },
        BuiltInRule {
            name: "builtin:chrome-notion",
            exe: Some("chrome.exe"),
            title_contains: Some("Notion"),
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: AppBehavior::BrowserTextField,
            strategy: Some(InjectionStrategy::SlowBackspaceText),
            fallback_strategies: &[InjectionStrategy::BackspaceText],
        },
        BuiltInRule {
            name: "builtin:vscode",
            exe: Some("Code.exe"),
            title_contains: None,
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: AppBehavior::PlainEditor,
            strategy: Some(InjectionStrategy::BackspaceText),
            fallback_strategies: &[],
        },
    ];
    RULES
}

fn default_rule_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{
        AppBehavior, InjectionStrategy, ProfileConfidence, ProfileReason, ProfileRule,
        is_excluded_app, is_password_context, resolve_profile, resolve_profile_with_rules,
    };
    use crate::context::{AppContext, FocusInfo};

    fn context(exe_name: Option<&str>) -> AppContext {
        AppContext {
            exe_name: exe_name.map(str::to_string),
            window_title: None,
            class_name: None,
            focus: None,
        }
    }

    fn browser_context(focus: FocusInfo) -> AppContext {
        AppContext {
            exe_name: Some("chrome.exe".to_string()),
            window_title: Some("Example".to_string()),
            class_name: Some("Chrome_WidgetWin_1".to_string()),
            focus: Some(focus),
        }
    }

    fn focus(control_type: &str) -> FocusInfo {
        FocusInfo {
            control_type: Some(control_type.to_string()),
            name: None,
            automation_id: None,
            class_name: None,
            framework_id: None,
            localized_control_type: None,
            process_id: None,
            has_keyboard_focus: None,
            value_available: false,
            is_password: false,
        }
    }

    #[test]
    fn missing_exe_uses_default_strategy() {
        let profile = resolve_profile(&context(None));
        assert_eq!(profile.strategy, InjectionStrategy::BackspaceText);
        assert_eq!(profile.behavior, AppBehavior::Unknown);
        assert_eq!(profile.reason, ProfileReason::MissingExe);
        assert_eq!(profile.confidence, ProfileConfidence::Fallback);
    }

    #[test]
    fn unknown_exe_uses_plain_editor_defaults() {
        let profile = resolve_profile(&context(Some("notepad.exe")));
        assert_eq!(profile.strategy, InjectionStrategy::BackspaceText);
        assert_eq!(profile.behavior, AppBehavior::PlainEditor);
        assert_eq!(profile.reason, ProfileReason::PlainEditorExe);
        assert_eq!(profile.confidence, ProfileConfidence::Likely);
    }

    #[test]
    fn browser_exe_without_focus_uses_address_bar_fallback() {
        for exe in ["thorium.exe", "chrome.exe", "msedge.exe", "brave.exe"] {
            let profile = resolve_profile(&context(Some(exe)));
            assert_eq!(profile.behavior, AppBehavior::BrowserAddressBar, "{exe}");
            assert_eq!(
                profile.strategy,
                InjectionStrategy::DeleteThenBackspaceText,
                "{exe}"
            );
            assert_eq!(profile.reason, ProfileReason::BrowserFallbackAddressBar);
            assert_eq!(profile.confidence, ProfileConfidence::Fallback);
        }
    }

    #[test]
    fn browser_matching_is_case_insensitive() {
        assert_eq!(
            resolve_profile(&context(Some("Thorium.EXE"))).strategy,
            InjectionStrategy::DeleteThenBackspaceText
        );
    }

    #[test]
    fn browser_address_focus_uses_address_bar_strategy() {
        let mut focus = focus("Edit");
        focus.name = Some("Address and search bar".to_string());
        focus.value_available = true;
        let profile = resolve_profile(&browser_context(focus));
        assert_eq!(profile.behavior, AppBehavior::BrowserAddressBar);
        assert_eq!(profile.strategy, InjectionStrategy::DeleteThenBackspaceText);
        assert_eq!(profile.reason, ProfileReason::BrowserFocusAddressBar);
        assert_eq!(profile.confidence, ProfileConfidence::Certain);
    }

    #[test]
    fn browser_document_focus_uses_text_field_strategy() {
        let mut focus = focus("Document");
        focus.value_available = true;
        let profile = resolve_profile(&browser_context(focus));
        assert_eq!(profile.behavior, AppBehavior::BrowserTextField);
        assert_eq!(profile.strategy, InjectionStrategy::BackspaceText);
        assert_eq!(profile.reason, ProfileReason::BrowserFocusTextField);
        assert_eq!(profile.confidence, ProfileConfidence::Likely);
    }

    #[test]
    fn browser_edit_without_value_pattern_stays_address_bar_safe() {
        let profile = resolve_profile(&browser_context(focus("Edit")));
        assert_eq!(profile.behavior, AppBehavior::BrowserAddressBar);
        assert_eq!(profile.reason, ProfileReason::BrowserFocusAddressBar);
    }

    #[test]
    fn terminal_exe_uses_slow_strategy() {
        for exe in [
            "WindowsTerminal.exe",
            "wt.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
        ] {
            let profile = resolve_profile(&context(Some(exe)));
            assert_eq!(profile.behavior, AppBehavior::TerminalLike, "{exe}");
            assert_eq!(
                profile.strategy,
                InjectionStrategy::SlowBackspaceText,
                "{exe}"
            );
            assert_eq!(profile.confidence, ProfileConfidence::Certain);
        }
    }

    #[test]
    fn rule_config_takes_precedence_over_builtin_focus() {
        let mut context = browser_context(focus("Edit"));
        context.window_title = Some("Google Docs - Chrome".to_string());
        let rules = [ProfileRule {
            name: Some("user:docs".to_string()),
            enabled: true,
            exe: Some("chrome.exe".to_string()),
            title_contains: Some("Google Docs".to_string()),
            class_contains: None,
            focus_name_contains: None,
            automation_id_contains: None,
            focus_class_contains: None,
            behavior: Some(AppBehavior::BrowserTextField),
            strategy: Some(InjectionStrategy::SlowBackspaceText),
            fallback_strategies: vec![InjectionStrategy::BackspaceText],
        }];

        let profile = resolve_profile_with_rules(&context, &rules);
        assert_eq!(profile.behavior, AppBehavior::BrowserTextField);
        assert_eq!(profile.strategy, InjectionStrategy::SlowBackspaceText);
        assert_eq!(
            profile.strategy_chain(),
            vec![
                InjectionStrategy::SlowBackspaceText,
                InjectionStrategy::BackspaceText
            ]
        );
        assert_eq!(profile.reason, ProfileReason::Rule);
        assert_eq!(profile.confidence, ProfileConfidence::Certain);
    }

    #[test]
    fn incomplete_rule_deserializes_and_does_not_match_everything() {
        let rules: Vec<ProfileRule> =
            serde_json::from_str(r#"[{"behavior":"BrowserTextField"}]"#).unwrap();

        let profile = resolve_profile_with_rules(&context(Some("chrome.exe")), &rules);
        assert_eq!(profile.reason, ProfileReason::BrowserFallbackAddressBar);
    }

    #[test]
    fn disabled_rule_does_not_match() {
        let rules = [ProfileRule {
            enabled: false,
            exe: Some("chrome.exe".to_string()),
            behavior: Some(AppBehavior::BrowserTextField),
            ..ProfileRule::default()
        }];

        let profile = resolve_profile_with_rules(&context(Some("chrome.exe")), &rules);
        assert_eq!(profile.reason, ProfileReason::BrowserFallbackAddressBar);
    }

    #[test]
    fn rule_can_match_focus_fields() {
        let mut context = browser_context(focus("Edit"));
        context.focus.as_mut().unwrap().automation_id = Some("editable-root".to_string());
        let rules = [ProfileRule {
            automation_id_contains: Some("editable".to_string()),
            behavior: Some(AppBehavior::BrowserTextField),
            strategy: Some(InjectionStrategy::BackspaceText),
            ..ProfileRule::default()
        }];

        let profile = resolve_profile_with_rules(&context, &rules);
        assert_eq!(profile.reason, ProfileReason::Rule);
        assert_eq!(profile.behavior, AppBehavior::BrowserTextField);
    }

    #[test]
    fn built_in_google_docs_rule_uses_slow_text_field_strategy() {
        let mut context = context(Some("chrome.exe"));
        context.window_title = Some("Report - Google Docs".to_string());

        let profile = resolve_profile(&context);
        assert_eq!(profile.reason, ProfileReason::BuiltInRule);
        assert_eq!(profile.rule_name, Some("builtin:chrome-google-docs"));
        assert_eq!(profile.behavior, AppBehavior::BrowserTextField);
        assert_eq!(profile.strategy, InjectionStrategy::SlowBackspaceText);
        assert_eq!(
            profile.strategy_chain(),
            vec![
                InjectionStrategy::SlowBackspaceText,
                InjectionStrategy::BackspaceText
            ]
        );
        assert_eq!(profile.confidence, ProfileConfidence::Certain);
    }

    #[test]
    fn address_bar_profile_has_empty_prefix_fallback() {
        let profile = resolve_profile(&context(Some("chrome.exe")));
        assert_eq!(
            profile.strategy_chain(),
            vec![
                InjectionStrategy::DeleteThenBackspaceText,
                InjectionStrategy::EmptyPrefixBackspaceText
            ]
        );
    }

    #[test]
    fn excluded_app_matching_is_case_insensitive() {
        assert!(is_excluded_app(
            &context(Some("Game.EXE")),
            &["game.exe".to_string()]
        ));
    }

    #[test]
    fn password_focus_is_detected_for_passthrough() {
        let mut context = context(Some("chrome.exe"));
        context.focus = Some(FocusInfo {
            is_password: true,
            ..FocusInfo::default()
        });

        assert!(is_password_context(&context));
        let profile = resolve_profile(&context);
        assert_eq!(profile.behavior, AppBehavior::Unknown);
        assert_eq!(profile.reason, ProfileReason::PasswordField);
    }
}
