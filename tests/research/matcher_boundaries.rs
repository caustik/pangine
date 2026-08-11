//! Warning checks for matcher and selector boundaries that remain unresolved.

use pangine::{ConceptId, Pangine};

#[test]
#[ignore = "warning: ordered nesting and associativity remain open design questions"]
fn explicit_ordered_nesting_currently_changes_question_matches() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['left'] ~= ([cat]->[eats])->[food]");
    must_ref(&mut pangine, "['right'] ~= [cat]->([eats]->[food])");

    ask(&mut pangine, "['left'] @ ['left-answer']->[eats]");
    assert_eq!(must_ref(&mut pangine, "$['left-answer']"), must_ref(&mut pangine, "[cat]"));

    ask(&mut pangine, "['right'] @ ['right-answer']->[eats]");
    assert!(pangine.reference_concept("$['right-answer']").unwrap().is_none());
}

#[test]
#[ignore = "warning: unresolved Percepts in structural subjects remain invalid"]
fn unresolved_percept_subjects_require_plain_source_selection_or_explicit_evaluation() {
    let mut pangine = Pangine::new();

    let answer = must_ref(&mut pangine, "[Alice] @ ['answer']");
    assert_eq!(pangine.format_concept(&answer, false), "[Alice]");
    assert_eq!(must_ref(&mut pangine, "$['answer']"), answer);

    for invalid in [
        "x2['Alice'] @ ['invalid-answer']",
        "['Alice']['Alice'] @ ['invalid-answer']",
        "['*'] @ ['invalid-answer']",
        "!['Alice'] @ ['invalid-answer']",
        "['Alice']->[context] @ ['invalid-answer']",
    ] {
        assert!(pangine.reference_concept(invalid).is_err(), "expected invalid selector: {invalid}");
    }
}

#[test]
#[ignore = "warning: combining partial matches into an unseen whole is provisional"]
fn distinct_partial_experience_can_induce_an_unseen_complete_answer() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}*{[B]->[D]}");
    for partial in ["{[E]->[A]}*{[P1]->[Q1]}", "{[E]->[A]}*{[P2]->[Q2]}", "{[E]->[A]}*{[P3]->[Q3]}"] {
        must_ref(&mut pangine, &format!("['memory'] ~= {partial}"));
    }

    let unseen = must_ref(&mut pangine, "{[E]->[A]}*{[B]->[D]}");
    assert!(!pangine.get_relevance_map(&memory).into_iter().any(|(_, concept)| concept == unseen));

    ask(&mut pangine, "['memory'] @ {['X']->[A]}*{[B]->[D]}");
    assert!(named_candidates(&mut pangine, "X").iter().any(|name| name == "E"));
}

fn named_candidates(pangine: &mut Pangine, percept: &str) -> Vec<String> {
    let Some(value) = pangine.reference_concept(&format!("$['{percept}']")).unwrap() else {
        return Vec::new();
    };
    pangine
        .get_relevance_map(&value)
        .into_iter()
        .map(|(_, candidate)| pangine.get_name(&candidate).unwrap_or_else(|| panic!("expected named candidate, got {candidate:?}")).to_owned())
        .collect()
}

fn ask(pangine: &mut Pangine, input: &str) {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
