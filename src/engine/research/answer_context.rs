//! Explicit context composition using the existing completion join.
//! This selects neither a language operation nor new input/Relevance semantics.

use super::super::{CompletionResult, ConceptId, Pangine};
use std::collections::BTreeSet;

#[test]
#[ignore = "warning: completing an ordinary input and explicitly joining it preserves pairs but also includes its source"]
fn an_ordinary_input_can_join_an_answer_through_the_existing_completion_operation() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{memory} ~= [cat]->[purrs]");
    must_ref(&mut pangine, "{memory} ~= [dog]->[barks]");
    must_ref(&mut pangine, "{memory} @ {animal}->{sound}");
    must_ref(&mut pangine, "{context} = [current]");
    let animal = pangine.reference_percept("animal");
    let context = pangine.reference_percept("context");
    let original = pangine.linked_answer_value(&animal).expect("original Answer value");
    let answer = pangine.answer_snapshot(&animal).expect("original Answer");
    let input = pangine.complete_question(std::slice::from_ref(&context), &context).expect("explicit input question");
    let projection = must_ref(&mut pangine, "{animal}->{sound}->{context}");
    let combined = join(&pangine, answer.result(), &input, &projection);
    assert_eq!(values(&mut pangine, &combined), expected_values(&mut pangine, &["[cat]->[purrs]->[current]", "[dog]->[barks]->[current]"]));
    assert!(combined.completions().iter().all(|row| row.evidence().len() == 2));
    assert_eq!(pangine.linked_answer_value(&animal), Some(original.clone()));
    assert_eq!(pangine.get_value(&context), Some(must_ref(&mut pangine, "[current]")));

    // The supplied context has become an explicit source, just as in a normal
    // question. This experiment is not a weight-free parameter convention.
    must_ref(&mut pangine, "{context} = [later]");
    let later_input = pangine.complete_question(std::slice::from_ref(&context), &context).expect("later input question");
    let later = join(&pangine, answer.result(), &later_input, &projection);
    assert_eq!(values(&mut pangine, &later), expected_values(&mut pangine, &["[cat]->[purrs]->[later]", "[dog]->[barks]->[later]"]));
    assert_eq!(values(&mut pangine, &combined), expected_values(&mut pangine, &["[cat]->[purrs]->[current]", "[dog]->[barks]->[current]"]));
    assert_eq!(pangine.linked_answer_value(&animal), Some(original));
}

#[test]
#[ignore = "warning: an explicit join retains independent alternatives and constrains shared bindings without selecting implicit read semantics"]
fn explicit_answer_composition_preserves_each_side_and_checks_shared_bindings() {
    let mut pangine = Pangine::new();
    let animals = complete(&mut pangine, "([cat]->[purrs])([dog]->[barks])", "{animal}->{sound}");
    let days = complete(&mut pangine, "([mon]->[bright])([tue]->[dark])", "{day}->{light}");
    let projection = must_ref(&mut pangine, "{animal}->{sound}->{day}->{light}");
    let combined = join(&pangine, &animals, &days, &projection);
    assert_eq!(
        values(&mut pangine, &combined),
        expected_values(
            &mut pangine,
            &["[cat]->[purrs]->[mon]->[bright]", "[cat]->[purrs]->[tue]->[dark]", "[dog]->[barks]->[mon]->[bright]", "[dog]->[barks]->[tue]->[dark]"]
        )
    );

    let places = complete(&mut pangine, "([cat]->[home])([eel]->[river])", "{animal}->{place}");
    let projection = must_ref(&mut pangine, "{animal}->{sound}->{place}");
    let compatible = join(&pangine, &animals, &places, &projection);
    assert_eq!(values(&mut pangine, &compatible), expected_values(&mut pangine, &["[cat]->[purrs]->[home]"]));
    assert_eq!(animals.completions().len(), 2);
    assert_eq!(days.completions().len(), 2);
    assert_eq!(places.completions().len(), 2);
}

#[test]
#[ignore = "warning: bounded explicit joins associate for these independent and shared-variable sources, including complete evidence"]
fn grouping_explicit_joins_keeps_the_same_rows_and_evidence_in_the_bounded_cases() {
    for final_question in ["{day}->{context}", "{animal}->{context}"] {
        let mut pangine = Pangine::new();
        let animals = complete(&mut pangine, "([cat]->[purrs])([dog]->[barks])", "{animal}->{sound}");
        let days = complete(&mut pangine, "([mon]->[bright])([tue]->[dark])", "{day}->{light}");
        let contexts = complete(&mut pangine, "([cat]->[home])([mon]->[work])", final_question);
        let first_pair = must_ref(&mut pangine, "{animal}->{sound}->{day}->{light}");
        let other_pair = must_ref(&mut pangine, &format!("({{day}}->{{light}})({final_question})"));
        let projection = must_ref(&mut pangine, "{animal}->{sound}->{day}->{light}->{context}");
        let animals_days = join(&pangine, &animals, &days, &first_pair);
        let days_contexts = join(&pangine, &days, &contexts, &other_pair);
        let left = join(&pangine, &animals_days, &contexts, &projection);
        let right = join(&pangine, &animals, &days_contexts, &projection);
        assert_eq!(left.completions().len(), 2);
        assert!(left.completions() == right.completions(), "complete bindings and source routes should agree in {final_question}");
        assert_eq!(values(&mut pangine, &left), values(&mut pangine, &right));
    }
}

#[test]
#[ignore = "warning: completing a compound input searches its parts; an ordinary field label does not impose a universal root scope"]
fn turning_a_value_into_a_question_is_not_the_same_as_binding_that_whole_value() {
    let mut pangine = Pangine::new();
    let whole = complete(&mut pangine, "[current]->[quiet]", "{context}");
    assert_eq!(values(&mut pangine, &whole), expected_values(&mut pangine, &["[current]->[quiet]", "[current]", "[quiet]"]));

    let wrapped = complete(&mut pangine, "[input]->([current]->[quiet])", "[input]->{context}");
    assert_eq!(values(&mut pangine, &wrapped), expected_values(&mut pangine, &["[input]->([current]->[quiet])"]));
    let repeated = complete(&mut pangine, "[input]->([input]->[nested])", "[input]->{context}");
    assert_eq!(values(&mut pangine, &repeated), expected_values(&mut pangine, &["[input]->([input]->[nested])", "[input]->[nested]"]));
}

fn join(pangine: &Pangine, left: &CompletionResult, right: &CompletionResult, projection: &ConceptId) -> CompletionResult {
    let mut left_outputs = BTreeSet::new();
    let mut right_outputs = BTreeSet::new();
    pangine.collect_output_percepts(left.question(), &mut left_outputs);
    pangine.collect_output_percepts(right.question(), &mut right_outputs);
    pangine.join_completion_results(left, &left_outputs, right, &right_outputs, projection)
}

fn complete(pangine: &mut Pangine, subject: &str, question: &str) -> CompletionResult {
    let subject = must_ref(pangine, subject);
    let question = must_ref(pangine, question);
    pangine.complete_subject(&subject, &question).expect("owned structural question")
}

fn values(pangine: &mut Pangine, result: &CompletionResult) -> BTreeSet<ConceptId> {
    result.completions().iter().map(|row| pangine.instantiate_completion(result.question(), row).expect("complete row")).collect()
}

fn expected_values(pangine: &mut Pangine, expected: &[&str]) -> BTreeSet<ConceptId> {
    expected.iter().map(|value| must_ref(pangine, value)).collect()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected a nonempty Concept for {input:?}"))
}
