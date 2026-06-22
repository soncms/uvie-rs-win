use serde::Deserialize;
use uvie_rs_win::session::{Edit, SessionEngine};

#[derive(Debug, Deserialize)]
struct Case {
    input: String,
    expected: String,
    tags: Vec<String>,
}

fn type_seq(input: &str) -> String {
    let mut engine = SessionEngine::default();
    let mut screen = String::new();
    for ch in input.chars() {
        match engine.feed(ch) {
            Edit::Pass => screen.push(ch),
            Edit::Replace { backspaces, text } => {
                for _ in 0..backspaces {
                    screen.pop();
                }
                screen.push_str(&text);
            }
        }
    }
    screen
}

#[test]
fn telex_corpus() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("telex_corpus.json")).expect("valid Telex corpus JSON");

    for case in cases {
        assert_eq!(
            type_seq(&case.input),
            case.expected,
            "input {:?}, tags {:?}",
            case.input,
            case.tags
        );
    }
}
