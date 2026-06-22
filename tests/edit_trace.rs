use serde::Deserialize;
use uvie_rs_win::session::{Edit, SessionEngine};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraceCase {
    name: String,
    events: Vec<String>,
    expected_screens: Vec<String>,
    expected_final: String,
}

#[test]
fn edit_traces_match_expected_screens() {
    let cases: Vec<TraceCase> =
        serde_json::from_str(include_str!("edit_traces.json")).expect("valid edit trace JSON");

    for case in cases {
        let mut engine = SessionEngine::default();
        let mut screen = String::new();
        let mut screens = Vec::new();
        for event in &case.events {
            apply_event(&mut engine, &mut screen, event);
            screens.push(screen.clone());
        }

        assert_eq!(screens, case.expected_screens, "trace {}", case.name);
        assert_eq!(screen, case.expected_final, "final {}", case.name);
    }
}

fn apply_event(engine: &mut SessionEngine, screen: &mut String, event: &str) {
    if event == "<BS>" {
        if engine.restore_after_boundary_backspace() {
            screen.pop();
        } else {
            apply_edit(screen, engine.backspace_visible(), None);
        }
        return;
    }

    let ch = event.chars().next().expect("single-char event");
    apply_edit(screen, engine.feed(ch), Some(ch));
}

fn apply_edit(screen: &mut String, edit: Edit, original: Option<char>) {
    match edit {
        Edit::Pass => {
            if let Some(ch) = original {
                screen.push(ch);
            }
        }
        Edit::Replace { backspaces, text } => {
            for _ in 0..backspaces {
                screen.pop();
            }
            screen.push_str(&text);
        }
    }
}
