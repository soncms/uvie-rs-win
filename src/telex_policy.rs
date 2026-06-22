#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    PlainAscii,
    Vietnamese,
    InvalidComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidatePolicy {
    pub kind: CandidateKind,
    pub has_vietnamese_mark: bool,
    pub valid_syllable: bool,
}

pub fn classify_candidate(raw: &str, visible: &str) -> CandidatePolicy {
    let has_vietnamese_mark = visible.chars().any(is_vietnamese_marked_char);
    if !has_vietnamese_mark {
        return CandidatePolicy {
            kind: CandidateKind::PlainAscii,
            has_vietnamese_mark,
            valid_syllable: false,
        };
    }

    let valid_syllable = visible
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|part| !part.is_empty())
        .all(is_likely_vietnamese_syllable);
    let repeated_tone = has_repeated_tone_key(raw);
    let kind = if valid_syllable && !repeated_tone {
        CandidateKind::Vietnamese
    } else {
        CandidateKind::InvalidComposition
    };

    CandidatePolicy {
        kind,
        has_vietnamese_mark,
        valid_syllable,
    }
}

pub fn is_likely_vietnamese_syllable(s: &str) -> bool {
    if matches!(s, "đ" | "Đ") {
        return true;
    }

    let stripped = strip_vietnamese_marks(s);
    if stripped.is_empty() || !stripped.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return false;
    }

    let lower = stripped.to_ascii_lowercase();
    let rest = strip_initial(&lower);
    let Some(rest) = rest else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }

    let nucleus = strip_final(rest);
    is_known_vowel_cluster(nucleus)
}

pub fn strip_vietnamese_marks(s: &str) -> String {
    s.chars().map(strip_vietnamese_char).collect()
}

fn strip_initial(s: &str) -> Option<&str> {
    const DOUBLE_INITIALS: &[&str] = &[
        "ngh", "ch", "gh", "gi", "kh", "ng", "nh", "ph", "qu", "th", "tr",
    ];
    for initial in DOUBLE_INITIALS {
        if let Some(rest) = s.strip_prefix(initial) {
            return Some(rest);
        }
    }

    if let Some(first) = s.chars().next()
        && matches!(
            first,
            'b' | 'c'
                | 'd'
                | 'g'
                | 'h'
                | 'k'
                | 'l'
                | 'm'
                | 'n'
                | 'p'
                | 'q'
                | 'r'
                | 's'
                | 't'
                | 'v'
                | 'x'
        )
    {
        return Some(&s[first.len_utf8()..]);
    }

    Some(s)
}

fn strip_final(s: &str) -> &str {
    const FINALS: &[&str] = &["ng", "nh", "ch", "c", "m", "n", "p", "t"];
    for final_part in FINALS {
        if s.len() > final_part.len()
            && let Some(rest) = s.strip_suffix(final_part)
        {
            return rest;
        }
    }
    s
}

fn is_known_vowel_cluster(s: &str) -> bool {
    const CLUSTERS: &[&str] = &[
        "a", "e", "i", "o", "u", "y", "ai", "ao", "au", "ay", "eo", "eu", "ia", "ie", "iu", "oa",
        "oe", "oi", "oo", "ua", "ue", "ui", "uo", "uu", "uy", "ya", "ye", "ieu", "oai", "oao",
        "oay", "uay", "uoi", "uou", "uya", "uye", "uyu", "yeu",
    ];
    CLUSTERS.contains(&s)
}

fn has_repeated_tone_key(raw: &str) -> bool {
    let mut tone_count = 0usize;
    let mut seen_vowel = false;
    for ch in raw.chars() {
        if is_ascii_vowel_key(ch) {
            seen_vowel = true;
            continue;
        }

        if seen_vowel
            && matches!(
                ch,
                's' | 'f' | 'r' | 'x' | 'j' | 'S' | 'F' | 'R' | 'X' | 'J'
            )
        {
            tone_count += 1;
            if tone_count > 1 {
                return true;
            }
        }
    }
    false
}

fn is_ascii_vowel_key(ch: char) -> bool {
    matches!(
        ch,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'A' | 'E' | 'I' | 'O' | 'U' | 'Y'
    )
}

fn is_vietnamese_marked_char(ch: char) -> bool {
    strip_vietnamese_char(ch) != ch
}

fn strip_vietnamese_char(ch: char) -> char {
    match ch {
        'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ' | 'ầ' | 'ẩ'
        | 'ẫ' | 'ậ' => 'a',
        'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ấ' | 'Ầ' | 'Ẩ'
        | 'Ẫ' | 'Ậ' => 'A',
        'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
        'É' | 'È' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' => 'E',
        'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' => 'I',
        'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ' | 'ờ' | 'ở'
        | 'ỡ' | 'ợ' => 'o',
        'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ớ' | 'Ờ' | 'Ở'
        | 'Ỡ' | 'Ợ' => 'O',
        'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
        'Ú' | 'Ù' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' => 'U',
        'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'Y',
        'đ' => 'd',
        'Đ' => 'D',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::{CandidateKind, classify_candidate, is_likely_vietnamese_syllable};

    #[test]
    fn accepts_common_vietnamese_syllables() {
        for syllable in [
            "tiếng", "người", "được", "chuyên", "huyễn", "cuối", "muốn", "hoá", "thuý", "quý",
            "tôi", "là", "chào", "rất", "nhiều", "kêu", "đều", "hoài", "cừu", "huệ", "rượu",
            "xoáy", "yếu",
        ] {
            assert!(is_likely_vietnamese_syllable(syllable), "{syllable}");
        }
    }

    #[test]
    fn classifies_plain_ascii_without_marks() {
        let policy = classify_candidate("google", "google");
        assert_eq!(policy.kind, CandidateKind::PlainAscii);
        assert!(!policy.has_vietnamese_mark);
    }

    #[test]
    fn classifies_vietnamese_candidate() {
        let policy = classify_candidate("tieengs", "tiếng");
        assert_eq!(policy.kind, CandidateKind::Vietnamese);
        assert!(policy.valid_syllable);
    }

    #[test]
    fn repeated_tone_is_invalid_composition() {
        let policy = classify_candidate("timmff", "tìmf");
        assert_eq!(policy.kind, CandidateKind::InvalidComposition);
    }

    #[test]
    fn initial_tone_letters_are_consonants_not_repeated_tones() {
        let policy = classify_candidate("raast", "rất");
        assert_eq!(policy.kind, CandidateKind::Vietnamese);
    }
}
