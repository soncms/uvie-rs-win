use crate::profile::InjectionStrategy;
use std::mem::{size_of, zeroed};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY, VK_BACK, VK_DELETE,
};

pub const INJECTED_MARKER: usize = 0x5556_4945;
const EMPTY_PREFIX: &str = "\u{202f}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub backspaces: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendReport {
    pub success: bool,
    pub attempted: Vec<InjectionStrategy>,
    pub used_strategy: Option<InjectionStrategy>,
}

impl Replacement {
    pub fn new(backspaces: usize, text: String) -> Self {
        Self { backspaces, text }
    }
}

pub fn build_inputs(strategy: InjectionStrategy, replacement: &Replacement) -> Vec<INPUT> {
    let text_units = replacement.text.encode_utf16().count();
    let mut inputs = Vec::with_capacity((replacement.backspaces + text_units + 2) * 2);

    match strategy {
        InjectionStrategy::BackspaceText | InjectionStrategy::SlowBackspaceText => {
            append_backspaces(&mut inputs, replacement.backspaces);
            append_text(&mut inputs, &replacement.text);
        }
        InjectionStrategy::DeleteThenBackspaceText => {
            if replacement.backspaces > 0 {
                append_key(&mut inputs, VK_DELETE);
            }
            append_backspaces(&mut inputs, replacement.backspaces);
            append_text(&mut inputs, &replacement.text);
        }
        InjectionStrategy::EmptyPrefixBackspaceText => {
            append_text(&mut inputs, EMPTY_PREFIX);
            append_backspaces(&mut inputs, replacement.backspaces + 1);
            append_text(&mut inputs, &replacement.text);
        }
    }

    inputs
}

pub fn send_replacement(strategy: InjectionStrategy, replacement: &Replacement) -> bool {
    if strategy == InjectionStrategy::SlowBackspaceText {
        return send_slow_replacement(replacement);
    }

    let mut inputs = build_inputs(strategy, replacement);
    unsafe { send_inputs(&mut inputs) }
}

pub fn send_replacement_with_fallback(
    strategies: &[InjectionStrategy],
    replacement: &Replacement,
) -> SendReport {
    let mut attempted = Vec::new();
    let chain = normalize_strategy_chain(strategies);

    for strategy in chain {
        attempted.push(strategy);
        if send_replacement(strategy, replacement) {
            return SendReport {
                success: true,
                attempted,
                used_strategy: Some(strategy),
            };
        }
    }

    SendReport {
        success: false,
        attempted,
        used_strategy: None,
    }
}

pub fn normalize_strategy_chain(strategies: &[InjectionStrategy]) -> Vec<InjectionStrategy> {
    let mut normalized = Vec::new();
    for strategy in strategies {
        if !normalized.contains(strategy) {
            normalized.push(*strategy);
        }
    }
    if normalized.is_empty() {
        normalized.push(InjectionStrategy::BackspaceText);
    }
    normalized
}

fn send_slow_replacement(replacement: &Replacement) -> bool {
    let mut backspaces = Vec::with_capacity(replacement.backspaces * 2);
    append_backspaces(&mut backspaces, replacement.backspaces);
    if unsafe { !send_inputs(&mut backspaces) } {
        return false;
    }

    if replacement.backspaces > 0 && !replacement.text.is_empty() {
        thread::sleep(Duration::from_millis(3));
    }

    let mut text = Vec::with_capacity(replacement.text.encode_utf16().count() * 2);
    append_text(&mut text, &replacement.text);
    unsafe { send_inputs(&mut text) }
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
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        ) == inputs.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_replacement_uses_backspaces_then_unicode() {
        let inputs = build_inputs(
            InjectionStrategy::BackspaceText,
            &Replacement::new(1, "â".to_string()),
        );

        assert_eq!(inputs.len(), 4);
        unsafe {
            assert_eq!(inputs[0].Anonymous.ki.wVk, VK_BACK);
            assert_eq!(inputs[1].Anonymous.ki.wVk, VK_BACK);
            assert_eq!(inputs[2].Anonymous.ki.wScan, 'â' as u16);
            assert_eq!(inputs[2].Anonymous.ki.dwFlags, KEYEVENTF_UNICODE);
        }
    }

    #[test]
    fn browser_replacement_clears_inline_selection_first() {
        let inputs = build_inputs(
            InjectionStrategy::DeleteThenBackspaceText,
            &Replacement::new(1, "â".to_string()),
        );

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
    fn browser_replacement_skips_delete_for_pure_insert() {
        let inputs = build_inputs(
            InjectionStrategy::DeleteThenBackspaceText,
            &Replacement::new(0, "â".to_string()),
        );

        assert_eq!(inputs.len(), 2);
        unsafe {
            assert_eq!(inputs[0].Anonymous.ki.wVk, 0);
            assert_eq!(inputs[0].Anonymous.ki.wScan, 'â' as u16);
            assert_eq!(inputs[0].Anonymous.ki.dwFlags, KEYEVENTF_UNICODE);
        }
    }

    #[test]
    fn empty_prefix_adds_prefix_and_extra_backspace() {
        let inputs = build_inputs(
            InjectionStrategy::EmptyPrefixBackspaceText,
            &Replacement::new(1, "â".to_string()),
        );

        assert_eq!(inputs.len(), 8);
        unsafe {
            assert_eq!(inputs[0].Anonymous.ki.wScan, '\u{202f}' as u16);
            assert_eq!(inputs[0].Anonymous.ki.dwFlags, KEYEVENTF_UNICODE);
            assert_eq!(inputs[2].Anonymous.ki.wVk, VK_BACK);
            assert_eq!(inputs[4].Anonymous.ki.wVk, VK_BACK);
            assert_eq!(inputs[6].Anonymous.ki.wScan, 'â' as u16);
        }
    }

    #[test]
    fn slow_strategy_matches_default_sequence_for_now() {
        let default = build_inputs(
            InjectionStrategy::BackspaceText,
            &Replacement::new(1, "â".to_string()),
        );
        let slow = build_inputs(
            InjectionStrategy::SlowBackspaceText,
            &Replacement::new(1, "â".to_string()),
        );

        assert_eq!(input_signature(&slow), input_signature(&default));
    }

    #[test]
    fn all_inputs_use_marker() {
        let inputs = build_inputs(
            InjectionStrategy::EmptyPrefixBackspaceText,
            &Replacement::new(1, "â".to_string()),
        );

        for input in inputs {
            unsafe {
                assert_eq!(input.Anonymous.ki.dwExtraInfo, INJECTED_MARKER);
            }
        }
    }

    #[test]
    fn strategy_chain_deduplicates_and_defaults() {
        assert_eq!(
            normalize_strategy_chain(&[
                InjectionStrategy::BackspaceText,
                InjectionStrategy::BackspaceText,
                InjectionStrategy::SlowBackspaceText,
            ]),
            vec![
                InjectionStrategy::BackspaceText,
                InjectionStrategy::SlowBackspaceText,
            ]
        );
        assert_eq!(
            normalize_strategy_chain(&[]),
            vec![InjectionStrategy::BackspaceText]
        );
    }

    fn input_signature(inputs: &[INPUT]) -> Vec<(VIRTUAL_KEY, u16, KEYBD_EVENT_FLAGS)> {
        inputs
            .iter()
            .map(|input| unsafe {
                (
                    input.Anonymous.ki.wVk,
                    input.Anonymous.ki.wScan,
                    input.Anonymous.ki.dwFlags,
                )
            })
            .collect()
    }
}
