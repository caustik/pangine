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
#[ignore = "warning: an enclosing ordered entry and a separately asked nested descendant are not yet correlated"]
fn enclosing_ordered_entries_do_not_yet_constrain_descendant_group_matches() {
    let mut pangine = Pangine::new();
    let question = must_ref(&mut pangine, "([row]->['selected-group'])([left]->['selected-left'])");
    let subject = must_ref(&mut pangine, "([row]->(([left]->[A])([right]->[B]))) ([row]->(([left]->[B])([right]->[A])))");

    let result = pangine.complete_subject(&subject, &question).unwrap();
    assert_eq!(result.completions().len(), 4, "the current rule crosses the two intended diagonal pairings");

    let weighted = must_ref(&mut pangine, "([row]->x2(([left]->[A])([right]->[B]))) ([row]->x3(([left]->[B])([right]->[A])))");
    assert_eq!(
        pangine.complete_subject(&weighted, &question).unwrap().completions().len(),
        4,
        "coefficient ancestry must not hide or accidentally resolve the same limitation"
    );
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
