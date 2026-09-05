//! Current reference/evaluation boundaries and a test-only one-layer comparison.

use pangine::{ConceptId, ConceptKind, Pangine};
use std::collections::BTreeSet;

#[test]
#[ignore = "warning: a singleton shape is still a source selector; an enclosing relationship preserves it as data"]
fn singleton_shapes_can_be_inspected_in_an_explicit_relationship() {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{memory} ~= [cat]");
    ask(&mut pangine, "{memory} @ {item}");
    let item = pangine.reference_percept("item");
    let original = pangine.linked_answer_value(&item).expect("original answer");

    ask(&mut pangine, "(&{item}) @ {direct}");
    assert_value(&mut pangine, "${direct}", "[cat]");
    ask(&mut pangine, "{saved-shape} = &{item}");
    ask(&mut pangine, "{saved-shape} @ {stored}");
    assert_value(&mut pangine, "${stored}", "{item}");
    ask(&mut pangine, "([shape]->(&{item})) @ [shape]->{wrapped}");
    assert_value(&mut pangine, "${wrapped}", "{item}");
    assert_eq!(pangine.linked_answer_value(&item), Some(original));
}

#[test]
#[ignore = "warning: a represented reference constraint works through a join but adds its own source contribution"]
fn a_reference_can_be_matched_by_identity_through_an_ordinary_input_record() {
    for (first, second) in [("[A]", "[B]"), ("{A}", "{B}")] {
        let mut pangine = Pangine::new();
        ask(&mut pangine, "{A} = [first-value]");
        ask(&mut pangine, "{B} = [second-value]");
        ask(&mut pangine, &format!("{{relations}} = ([relation]->{first})([relation]->{second})"));
        ask(&mut pangine, &format!("{{request}} = [desired]->{first}"));
        ask(&mut pangine, "{relations}{request} @ ([relation]->{found})([desired]->{found})");
        assert_value(&mut pangine, "${found}", &format!("x2({first})"));
        assert_value(&mut pangine, "${A}", "[first-value]");
        assert_value(&mut pangine, "${B}", "[second-value]");
    }
}

#[test]
#[ignore = "warning: current recursive reads of detached aliases lose correlations retained by direct linked reads"]
fn a_detached_question_shape_currently_reads_its_outputs_independently() {
    let mut pangine = animals();
    let expected = must_ref(&mut pangine, "([cat]->[purrs])([dog]->[barks])");
    assert_eq!(must_ref(&mut pangine, "$(&{animal})"), expected);
    ask(&mut pangine, "{alias} = &{animal}");
    assert_value(&mut pangine, "${alias}", "([cat][dog])->([barks][purrs])");

    let alias = pangine.reference_percept("alias");
    let first_read = read_one_layer(&mut pangine, &alias).expect("represented shape");
    assert_eq!(first_read, must_ref(&mut pangine, "{animal}->{sound}"));
    assert_eq!(read_one_layer(&mut pangine, &first_read), Some(expected.clone()));
    assert_eq!(read_at_each_value(&mut pangine, &alias, ReadRule::Recursive), Some(expected));
}

#[test]
#[ignore = "warning: one ordinary input prevents the current whole-expression linked projection"]
fn an_ordinary_input_can_currently_break_a_linked_read_into_marginals() {
    let mut pangine = animals();
    ask(&mut pangine, "{context} = [current]");
    let mixed = must_ref(&mut pangine, "{animal}->{sound}->{context}");
    let detached_fields = must_ref(&mut pangine, "([cat][dog])->([barks][purrs])->[current]");
    let expected_pairs = must_ref(&mut pangine, "([cat]->[purrs]->[current])([dog]->[barks]->[current])");
    assert_eq!(pangine.evaluate_concept(&mixed), Some(detached_fields.clone()));
    assert_eq!(read_one_layer(&mut pangine, &mixed), Some(detached_fields.clone()));
    assert_eq!(read_at_each_value(&mut pangine, &mixed, ReadRule::Recursive), Some(detached_fields));
    assert_eq!(must_ref(&mut pangine, "$({animal}->{sound}->${context})"), expected_pairs);
}

#[test]
#[ignore = "warning: one-layer substitution preserves supplied Answer data but does not provide its language-level name"]
fn one_layer_reading_keeps_an_encoded_answer_available_as_data() {
    let mut pangine = animals();
    let animal = pangine.reference_percept("animal");
    let original = pangine.linked_answer_value(&animal).expect("original answer value");
    let archive = pangine.reference_percept("archive");
    assert!(pangine.set_percept_value(&archive, Some(original.clone())));

    assert_ne!(pangine.evaluate_concept(&archive), Some(original.clone()));
    assert_eq!(read_one_layer(&mut pangine, &archive), Some(original.clone()));
    assert_ne!(read_at_each_value(&mut pangine, &archive, ReadRule::Recursive), Some(original.clone()));
    assert_eq!(pangine.linked_answer_value(&animal), Some(original));
}

#[test]
#[ignore = "warning: recursively checking each supplied value restores alias projection but retains an Answer-specific stopping rule"]
fn checking_each_supplied_value_recognizes_wrapped_shapes_and_still_stops_reference_cycles() {
    let mut pangine = animals();
    let alias = pangine.reference_percept("alias");
    let mut shape = must_ref(&mut pangine, "{animal}->{sound}");
    for depth in 0..5 {
        let wrapper = pangine.reference_name(&format!("layer-{depth}"));
        shape = pangine.compose_ordered(&[wrapper, shape]).expect("owned shape").unwrap();
        let direct = pangine.evaluate_concept(&shape).expect("direct linked projection");
        assert!(pangine.set_percept_value(&alias, Some(shape.clone())));
        assert_eq!(read_at_each_value(&mut pangine, &alias, ReadRule::Recursive), Some(direct));
        assert_eq!(read_one_layer(&mut pangine, &alias), Some(shape.clone()));
    }

    ask(&mut pangine, "{cycle-a} = {cycle-b}");
    ask(&mut pangine, "{cycle-b} = [next]->{cycle-a}");
    let cycle = pangine.reference_percept("cycle-a");
    let expected = must_ref(&mut pangine, "[next]->{cycle-a}");
    assert_eq!(read_at_each_value(&mut pangine, &cycle, ReadRule::Recursive), Some(expected));
}

// Bounded comparisons, not an engine API or a selected meaning of `$`.
// Both recognize a linked projection at every visited value. OneLayer stops at
// supplied values; Recursive follows them until a linked projection or a cycle.
#[derive(Clone, Copy)]
enum ReadRule {
    OneLayer,
    Recursive,
}

fn read_one_layer(pangine: &mut Pangine, concept: &ConceptId) -> Option<ConceptId> {
    read_at_each_value(pangine, concept, ReadRule::OneLayer)
}

fn read_at_each_value(pangine: &mut Pangine, concept: &ConceptId, rule: ReadRule) -> Option<ConceptId> {
    read_inner(pangine, concept, rule, &mut BTreeSet::new())
}

fn read_inner(pangine: &mut Pangine, concept: &ConceptId, rule: ReadRule, visited: &mut BTreeSet<ConceptId>) -> Option<ConceptId> {
    if let Some(view) = pangine.answer_view(concept) {
        return view.materialize(pangine);
    }
    match pangine.concept_kind(concept)? {
        ConceptKind::Named(_) => Some(concept.clone()),
        ConceptKind::Percept { .. } => {
            if !visited.insert(concept.clone()) {
                return Some(concept.clone());
            }
            let value = pangine.get_value(concept).and_then(|value| match rule {
                ReadRule::OneLayer => Some(value),
                ReadRule::Recursive => read_inner(pangine, &value, rule, visited),
            });
            visited.remove(concept);
            value
        }
        ConceptKind::Ordered { components } => {
            let components = components.clone();
            let values = components.iter().map(|component| read_inner(pangine, component, rule, visited)).collect::<Option<Vec<_>>>()?;
            pangine.compose_ordered(&values).ok().flatten()
        }
        ConceptKind::Unordered => {
            let entries = pangine.get_relevance_map(concept);
            let values =
                entries.into_iter().filter_map(|(amount, value)| read_inner(pangine, &value, rule, visited).map(|value| (amount, value))).collect::<Vec<_>>();
            pangine.compose_union(&values).ok().flatten()
        }
    }
}

fn animals() -> Pangine {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{memory} ~= [cat]->[purrs]");
    ask(&mut pangine, "{memory} ~= [dog]->[barks]");
    ask(&mut pangine, "{memory} @ {animal}->{sound}");
    pangine
}

fn assert_value(pangine: &mut Pangine, input: &str, expected: &str) {
    assert_eq!(must_ref(pangine, input), must_ref(pangine, expected), "{input}");
}

fn ask(pangine: &mut Pangine, input: &str) -> Option<ConceptId> {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    ask(pangine, input).unwrap_or_else(|| panic!("expected a nonempty Concept for {input:?}"))
}
