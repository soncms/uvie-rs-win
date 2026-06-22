use uvie::{InputMethod, UltraFastViEngine};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Replace { backspaces: usize, text: String },
    Pass,
}

#[derive(Debug, Clone)]
pub struct SessionEngine {
    raw: String,
    visible: String,
    quick_telex: bool,
}

impl Default for SessionEngine {
    fn default() -> Self {
        Self::new(false)
    }
}

impl SessionEngine {
    pub fn new(quick_telex: bool) -> Self {
        Self {
            raw: String::new(),
            visible: String::new(),
            quick_telex,
        }
    }

    pub fn reset(&mut self) {
        self.raw.clear();
        self.visible.clear();
    }

    pub fn is_composing(&self) -> bool {
        !self.raw.is_empty()
    }

    pub fn visible(&self) -> &str {
        &self.visible
    }

    pub fn feed(&mut self, ch: char) -> Edit {
        if Self::is_boundary(ch) {
            self.reset();
            return Edit::Pass;
        }

        if !ch.is_ascii_graphic() {
            self.reset();
            return Edit::Pass;
        }

        let before = self.visible.clone();
        let mut candidate_raw = self.raw.clone();
        candidate_raw.push(ch);
        let mut candidate = self.render(&candidate_raw);

        if let Some((normalized_raw, normalized)) = self.normalize_early_tone(&candidate_raw) {
            if score_render(&normalized) > score_render(&candidate) {
                candidate_raw = normalized_raw;
                candidate = normalized;
            }
        }

        if self.should_restore_raw(ch, &candidate_raw, &candidate) {
            let backspaces = before.chars().count();
            self.reset();
            return Edit::Replace {
                backspaces,
                text: candidate_raw,
            };
        }

        if self.should_restore_literal(ch, &candidate_raw, &candidate) {
            self.reset();
            return Edit::Pass;
        }

        let (backspaces, text) = diff(&before, &candidate);
        self.raw = candidate_raw;
        self.visible = candidate;

        let original = ch.to_string();
        if backspaces == 0 && text == original {
            Edit::Pass
        } else {
            Edit::Replace { backspaces, text }
        }
    }

    pub fn backspace_visible(&mut self) -> Edit {
        if self.raw.is_empty() || self.visible.is_empty() {
            self.reset();
            return Edit::Pass;
        }

        let target = remove_last_char(&self.visible);
        let Some((raw, rendered)) = self.find_raw_for_visible(&target) else {
            self.reset();
            return Edit::Pass;
        };

        self.raw = raw;
        self.visible = rendered;
        Edit::Replace {
            backspaces: 1,
            text: String::new(),
        }
    }

    fn render(&self, raw: &str) -> String {
        let mut engine = UltraFastViEngine::new();
        engine.set_input_method(InputMethod::Telex);
        engine.set_quick_start(false);
        engine.set_quick_telex(self.quick_telex);
        engine.set_modern_orthography(true);

        for ch in raw.chars() {
            engine.feed(ch);
        }
        engine.current_composing().to_string()
    }

    fn find_raw_for_visible(&self, target: &str) -> Option<(String, String)> {
        for end in (0..=self.raw.len()).rev() {
            if !self.raw.is_char_boundary(end) {
                continue;
            }
            let raw = &self.raw[..end];
            let rendered = self.render(raw);
            if rendered == target {
                return Some((raw.to_string(), rendered));
            }
        }
        None
    }

    fn normalize_early_tone(&self, raw: &str) -> Option<(String, String)> {
        if self.visible.is_empty() || raw.len() < 3 {
            return None;
        }

        let chars: Vec<char> = raw.chars().collect();
        let tone_pos = chars
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, ch)| is_tone_key(*ch).then_some(i))?;
        if tone_pos + 1 >= chars.len() || !chars[tone_pos + 1..].iter().all(|ch| ch.is_ascii_alphabetic()) {
            return None;
        }
        if !has_u_horn_before(&chars, tone_pos) {
            return None;
        }

        let mut base = chars.clone();
        let tone = base.remove(tone_pos);
        base.push(tone);

        let mut variants = vec![base.iter().collect::<String>()];
        variants.extend(reorder_horn_variants(&base));
        variants
            .into_iter()
            .map(|variant| {
                let rendered = self.render(&variant);
                (variant, rendered)
            })
            .max_by_key(|(_, rendered)| score_render(rendered))
    }

    fn should_restore_literal(&self, ch: char, raw: &str, candidate: &str) -> bool {
        if raw.len() == 1 && matches!(ch, 'w' | 'W') && matches!(candidate, "ư" | "Ư") {
            return true;
        }

        if self.visible.is_empty() {
            return false;
        }

        // If an existing composed vowel would be rewritten into a longer mixed
        // sequence by a horn key, prefer the user-visible literal key.
        if matches!(ch, 'w' | 'W') && ends_with_composed_o(&self.visible) {
            return candidate.chars().count() > self.visible.chars().count()
                || candidate.contains('ư')
                || candidate.contains('Ư');
        }

        // English / invalid passthrough: once the engine falls back to raw ASCII
        // after previously showing Vietnamese, restore literal typing instead of
        // rewriting the whole word under the cursor.
        if candidate == raw && self.visible != self.raw {
            return true;
        }

        false
    }

    fn should_restore_raw(&self, ch: char, raw: &str, candidate: &str) -> bool {
        if self.visible.is_empty() || self.visible == self.raw {
            return false;
        }

        if candidate == raw {
            return true;
        }

        if ends_with_composed_o(&self.visible) && is_impossible_single_o_coda(ch) {
            return true;
        }

        false
    }

    fn is_boundary(ch: char) -> bool {
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
}

pub fn diff(prev: &str, next: &str) -> (usize, String) {
    let mut common = 0usize;
    for (a, b) in prev.chars().zip(next.chars()) {
        if a != b {
            break;
        }
        common += 1;
    }

    let prev_len = prev.chars().count();
    let suffix = next.chars().skip(common).collect();
    (prev_len.saturating_sub(common), suffix)
}

fn remove_last_char(s: &str) -> String {
    let mut out = s.to_string();
    out.pop();
    out
}

fn ends_with_composed_o(s: &str) -> bool {
    matches!(s.chars().last(), Some('ô' | 'Ô' | 'ơ' | 'Ơ'))
}

fn is_impossible_single_o_coda(ch: char) -> bool {
    matches!(
        ch,
        'b' | 'd'
            | 'g'
            | 'h'
            | 'k'
            | 'l'
            | 'q'
            | 'v'
            | 'z'
            | 'B'
            | 'D'
            | 'G'
            | 'H'
            | 'K'
            | 'L'
            | 'Q'
            | 'V'
            | 'Z'
    )
}

fn is_tone_key(ch: char) -> bool {
    matches!(ch, 's' | 'f' | 'r' | 'x' | 'j' | 'S' | 'F' | 'R' | 'X' | 'J')
}

fn score_render(s: &str) -> usize {
    s.chars().filter(|ch| !ch.is_ascii()).count() * 10 + s.chars().count()
}

fn reorder_horn_variants(chars: &[char]) -> Vec<String> {
    let mut variants = Vec::new();
    for i in 0..chars.len().saturating_sub(2) {
        if matches!(chars[i], 'u' | 'U')
            && matches!(chars[i + 1], 'w' | 'W')
            && matches!(chars[i + 2], 'o' | 'O')
        {
            let mut v = chars.to_vec();
            v.swap(i + 1, i + 2);
            variants.push(v.iter().collect());
        }
    }
    variants
}

fn has_u_horn_before(chars: &[char], end: usize) -> bool {
    chars[..end]
        .windows(2)
        .any(|pair| matches!(pair, ['u' | 'U', 'w' | 'W']))
}

#[cfg(test)]
mod tests {
    use super::{Edit, SessionEngine};

    fn type_seq(engine: &mut SessionEngine, seq: &str) -> String {
        let mut screen = String::new();
        for ch in seq.chars() {
            apply(&mut screen, engine.feed(ch), ch);
        }
        screen
    }

    fn apply(screen: &mut String, edit: Edit, original: char) {
        match edit {
            Edit::Pass => screen.push(original),
            Edit::Replace { backspaces, text } => {
                for _ in 0..backspaces {
                    screen.pop();
                }
                screen.push_str(&text);
            }
        }
    }

    fn backspace(screen: &mut String, engine: &mut SessionEngine) {
        match engine.backspace_visible() {
            Edit::Pass => {
                screen.pop();
            }
            Edit::Replace { backspaces, text } => {
                for _ in 0..backspaces {
                    screen.pop();
                }
                screen.push_str(&text);
            }
        }
    }

    #[test]
    fn visible_backspace_g_o_circumflex() {
        let mut e = SessionEngine::default();
        let mut screen = type_seq(&mut e, "goo");
        assert_eq!(screen, "gô");
        backspace(&mut screen, &mut e);
        assert_eq!(screen, "g");
    }

    #[test]
    fn visible_backspace_toi() {
        let mut e = SessionEngine::default();
        let mut screen = type_seq(&mut e, "tooi");
        assert_eq!(screen, "tôi");
        backspace(&mut screen, &mut e);
        assert_eq!(screen, "tô");
        backspace(&mut screen, &mut e);
        assert_eq!(screen, "t");
    }

    #[test]
    fn telex_basics() {
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "tieengs"), "tiếng");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "ow"), "ơ");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "uw"), "ư");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "aa"), "â");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "aw"), "ă");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "ee"), "ê");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "oo"), "ô");
    }

    #[test]
    fn no_wowo_loop_after_o_circumflex() {
        let mut e = SessionEngine::default();
        let out = type_seq(&mut e, "oowowo");
        assert!(!out.contains("ôưoưo"));
    }

    #[test]
    fn english_passthrough() {
        for word in ["account", "window", "google", "workflow"] {
            let mut e = SessionEngine::default();
            assert_eq!(type_seq(&mut e, word), word);
        }
    }

    #[test]
    fn tone_placement() {
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "hoas"), "hoá");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "thuys"), "thuý");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "quys"), "quý");
    }

    #[test]
    fn early_tone_then_continue_nucleus() {
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "nguwfoi"), "người");
        let mut e = SessionEngine::default();
        assert_eq!(type_seq(&mut e, "thuwfong"), "thường");
    }
}
