//! Warning checks for matcher and selector boundaries that remain unresolved.

use pangine::{ConceptId, Pangine};

#[test]
#[ignore = "warning: ordered nesting and associativity remain open design questions"]
fn explicit_ordered_nesting_currently_changes_question_matches() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "{left} ~= ([cat]->[eats])->[food]");
    must_ref(&mut pangine, "{right} ~= [cat]->([eats]->[food])");

    ask(&mut pangine, "{left} @ {left-answer}->[eats]");
    assert_eq!(must_ref(&mut pangine, "${left-answer}"), must_ref(&mut pangine, "[cat]"));

    ask(&mut pangine, "{right} @ {right-answer}->[eats]");
    assert!(pangine.reference_concept("${right-answer}").unwrap().is_none());
}

#[test]
#[ignore = "warning: embedded Percepts are represented data while plain Percept selectors select retained sources"]
fn structural_subjects_keep_percepts_literal_instead_of_implicitly_selecting_them() {
    let mut pangine = Pangine::new();

    let answer = must_ref(&mut pangine, "[Alice] @ {answer}");
    assert_eq!(pangine.format_concept(&answer, false), "[Alice]");
    assert_eq!(must_ref(&mut pangine, "${answer}"), answer);
    assert!(pangine.reference_concept("{*} @ {global-answer}").is_ok(), "the read-only global Percept follows the ordinary Percept selector path");

    must_ref(&mut pangine, "{Alice} = [must-not-be-read]");
    for subject in ["x2{Alice}", "{Alice}{Alice}", "!{Alice}", "{Alice}->[context]"] {
        let subject_value = must_ref(&mut pangine, subject);
        ask(&mut pangine, &format!("{subject} @ {{literal-answer}}"));
        let output = pangine.reference_percept("literal-answer");
        let answer = pangine.answer_snapshot(&output).expect("literal structural answer");
        let reference = pangine.reference_percept("Alice");
        let bindings = answer.result().completions().iter().filter_map(|row| row.binding(&output)).collect::<Vec<_>>();
        assert!(bindings.contains(&&subject_value));
        assert!(bindings.contains(&&reference));
        assert!(answer.result().completions().iter().flat_map(|row| row.evidence()).all(|evidence| evidence.source_percept().is_none()));
        assert_eq!(pangine.get_value(&reference), Some(must_ref(&mut pangine, "[must-not-be-read]")));
    }
}

#[test]
#[ignore = "warning: an enclosing ordered entry and a separately asked nested descendant are not yet correlated"]
fn enclosing_ordered_entries_do_not_yet_constrain_descendant_group_matches() {
    let mut pangine = Pangine::new();
    let question = must_ref(&mut pangine, "([row]->{selected-group})([left]->{selected-left})");
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
