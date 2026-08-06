//! Warning checks for the placeholder decision behavior behind `^`.
//!
//! Pangine is expected to retain a decision or sampling operator. Positive
//! filtering, greatest-coefficient selection, and canonical tie breaking are
//! only the current deterministic fallback.

use pangine::Pangine;

#[test]
#[ignore = "warning: positive greatest-coefficient choice is a placeholder"]
fn current_decision_prefers_the_greatest_positive_coefficient() {
    assert_eq!(run("['choice'] = x2[tea]x3[coffee]; ^['choice']"), Some("[coffee]".to_owned()));
    assert_eq!(run("['mixed'] = x2[tea]![coffee]; ^['mixed']"), Some("[tea]".to_owned()));
    assert_eq!(run("['negative'] = ![tea]![coffee]; ^['negative']"), None);
}

#[test]
#[ignore = "warning: canonical tie breaking is a placeholder"]
fn current_decision_breaks_ties_by_canonical_spelling() {
    assert_eq!(run("['keep-a']=[A];['keep-b']=[B];['choice']=$['keep-a']*$['keep-b'];^['choice']"), Some("[A]".to_owned()));
    assert_eq!(run("['keep-b']=[B];['keep-a']=[A];['choice']=$['keep-a']*$['keep-b'];^['choice']"), Some("[A]".to_owned()));
}

fn run(script: &str) -> Option<String> {
    let mut pangine = Pangine::new();
    let concept = pangine.parse_script_text(script).unwrap_or_else(|error| panic!("failed to parse {script:?}: {error}"))?;
    Some(pangine.format_concept(&concept, false))
}
