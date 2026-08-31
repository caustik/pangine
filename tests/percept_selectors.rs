use pangine::{ConceptId, Pangine};

#[test]
fn a_percept_selector_keeps_one_meaning_regardless_of_its_value_shape() {
    let mut pangine = Pangine::new();
    let container = pangine.reference_percept("container");
    let member = pangine.reference_percept("member");

    must_ref(&mut pangine, "['member'] = [record]->[value]");
    must_ref(&mut pangine, "['container'] = ['member']");

    assert!(
        pangine.reference_concept("['container'] @ [record]->['implicit-value']").expect("valid question").is_none(),
        "a Percept must remain its own source instead of implicitly expanding its current value"
    );
    assert_eq!(pangine.get_value(&container), Some(member));

    let explicit = must_ref(&mut pangine, "['member'] @ [record]->['explicit-value']");
    assert_eq!(pangine.format_concept(&explicit, false), "{[record]->[value]}");
}

#[test]
fn the_global_percept_uses_the_same_source_identity_as_other_percepts() {
    let mut pangine = Pangine::new();
    let global = pangine.global_percept();
    let _known = must_ref(&mut pangine, "[known]");
    let answer = pangine.reference_percept("answer");

    let result = pangine.complete_selector(&global, &answer).expect("the global Percept is a valid selector");
    assert!(!result.completions().is_empty());
    assert!(
        result.completions().iter().flat_map(|completion| completion.evidence()).all(|evidence| evidence.source_percept() == Some(&global)),
        "the global Percept must remain the selected source instead of expanding into other Percept identities"
    );
}

#[test]
fn the_global_percept_is_an_ordinary_language_question_selector() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['memory'] = [known]");

    let result = must_ref(&mut pangine, "['*'] @ ['answer']");
    assert_eq!(pangine.format_concept(&result, false), "[known]");
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}")).unwrap_or_else(|| panic!("{input:?} was null"))
}
