//! Experiments in continuing an inquiry through the same Concept grammar.

use pangine::{CompletionRemainderSide, ConceptId, Pangine};
use std::collections::BTreeSet;

const OBSERVATIONS: &str = include_str!("recursive_inquiry.pae");

#[test]
#[ignore = "warning: recursive inquiry is a capability experiment, not a settled question or evidence policy"]
fn observations_can_be_inspected_questioned_and_revisited_without_choosing() {
    let mut pangine = observations();
    let remembered = must_ref(&mut pangine, "${observations}");

    ask(&mut pangine, "{observations} @ ({record}->[object]->{object})({record}->[features]->([red][round]))");
    assert_value(&mut pangine, "$({record}->{object})", "([o-1]->[amber])([o-2]->[amber])");
    ask(&mut pangine, "{observations} @ ({record}->[observer]->{observer})({record}->[features]->{features})");
    let open = must_ref(&mut pangine, "$({record}->{object}->{observer}->{features})");
    assert_eq!(open, must_ref(&mut pangine, "([o-1]->[amber]->[north]->([red][round]))([o-2]->[amber]->[south]->([heavy][red][round]))"));

    ask(&mut pangine, "{observed-pairs} = $({record}->{object})");
    ask(&mut pangine, "{observed-pairs} @ {inspected-record}->[amber]");
    assert_value(&mut pangine, "${inspected-record}", "[o-1][o-2]");
    ask(&mut pangine, "{selection} = ${object}");
    assert_value(&mut pangine, "^{selection}", "[amber]");
    assert_eq!(must_ref(&mut pangine, "$({record}->{object}->{observer}->{features})"), open);
    assert_eq!(must_ref(&mut pangine, "${observations}"), remembered);
}

#[test]
#[ignore = "warning: different explicit readings exercise composition without choosing a universal conclusion policy"]
fn the_same_open_observations_support_different_explicit_conclusions() {
    let mut pangine = observations();
    ask(&mut pangine, "{observations} @ ({record}->[object]->{object})({record}->[features]->{features})");
    let record = pangine.reference_percept("record");
    let original = pangine.linked_answer_value(&record).expect("open observation answer");

    ask(&mut pangine, "{by-support} = ${object}");
    assert_value(&mut pangine, "^{by-support}", "[amber]");
    ask(&mut pangine, "($({object}->{features})) @ {blue-object}->([blue][round])");
    assert_value(&mut pangine, "${blue-object}", "[cedar]");
    assert_value(&mut pangine, "^{blue-object}", "[cedar]");
    assert_eq!(pangine.linked_answer_value(&record), Some(original));
    assert_value(&mut pangine, "${object}", "x2[amber][birch][cedar]");
}

#[test]
#[ignore = "warning: a question may complete supplied context; its remainder does not establish that context as observed"]
fn completion_context_and_observed_properties_remain_distinct_in_the_evidence() {
    let mut pangine = observations();
    ask(&mut pangine, "{observations} @ ({record}->[object]->{object})({record}->[features]->([red][round][heavy]))");
    let record = pangine.reference_percept("record");
    let object = pangine.reference_percept("object");
    let answer = pangine.answer_snapshot(&object).expect("completed context");
    let heavy = must_ref(&mut pangine, "[heavy]");
    let supported = must_ref(&mut pangine, "[o-2]");
    let incomplete = must_ref(&mut pangine, "[o-1]");
    assert_eq!(answer.result().completions().len(), 2);

    for row in answer.result().completions() {
        let missing = row
            .evidence()
            .iter()
            .flat_map(|evidence| evidence.remainders())
            .filter(|remainder| remainder.side() == CompletionRemainderSide::Question)
            .map(|remainder| remainder.concept().clone())
            .collect::<BTreeSet<_>>();
        match row.binding(&record) {
            Some(id) if id == &incomplete => assert_eq!(missing, BTreeSet::from([heavy.clone()])),
            Some(id) if id == &supported => assert!(missing.is_empty()),
            unexpected => panic!("unexpected observation binding {unexpected:?}"),
        }
    }

    // The current visible result alone gives both records the completed shape.
    assert_value(&mut pangine, "$({record}->{object})", "([o-1]->[amber])([o-2]->[amber])");
}

#[test]
#[ignore = "warning: encoded Answer inspection tests the existing codec, not a proposed language schema"]
fn encoded_answer_can_be_supplied_as_experience_for_structural_inspection() {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{memory} ~= [item]->([red][round])");
    ask(&mut pangine, "{memory} @ {object}->([red][round][heavy])");
    let object = pangine.reference_percept("object");
    let encoded = pangine.linked_answer_value(&object).expect("live answer Concept");
    let archive = pangine.reference_percept("archive");
    assert!(pangine.set_percept_value(&archive, Some(encoded.clone())));

    ask(&mut pangine, "{archive} @ [pangine-answer-remainder]->[pangine-answer-question-remainder]->{path}->{missing}");
    assert_value(&mut pangine, "${missing}", "[heavy]");
    ask(&mut pangine, "{archive} @ [pangine-answer-source]->{origin}->{source}");
    assert_value(&mut pangine, "${source}", "[item]->([red][round])");
    assert_value(&mut pangine, "${origin}", "[pangine-answer-percept-source]->{memory}");
    assert_eq!(pangine.linked_answer_value(&object), Some(encoded));
}

#[test]
#[ignore = "warning: direct and stored question inspection is an experiment in recursive composition"]
fn question_shapes_can_be_inspected_directly_or_after_storing_them() {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{memory} ~= [cat]->[purrs]");
    ask(&mut pangine, "{memory} @ {animal}->{sound}");
    let shape = must_ref(&mut pangine, "&{animal}");
    let structure = must_ref(&mut pangine, "{left}->{right}");
    let direct = pangine.complete_subject(&shape, &structure).expect("a question is a Concept subject");
    assert_eq!(direct.completions().len(), 1);
    assert_eq!(must_ref(&mut pangine, "(&{animal}) @ {left}->{right}"), shape);
    assert_value(&mut pangine, "${left}", "{animal}");
    assert_value(&mut pangine, "${right}", "{sound}");

    ask(&mut pangine, "{shape} = &{animal}");
    ask(&mut pangine, "{shape} @ {left}->{right}");
    assert_value(&mut pangine, "${left}", "{animal}");
    assert_value(&mut pangine, "${right}", "{sound}");
    assert_value(&mut pangine, "${animal}", "[cat]");
    assert_value(&mut pangine, "${sound}", "[purrs]");
    assert_eq!(must_ref(&mut pangine, "$({left}->{right})"), shape);

    // Reading a linked projection substitutes once; reading a detached copy
    // follows the Percepts it contains. This is a separate evaluation boundary.
    ask(&mut pangine, "{detached} = $({left}->{right})");
    assert_value(&mut pangine, "${detached}", "[cat]->[purrs]");
}

#[test]
#[ignore = "warning: inspecting an Answer of an Answer relies on Rust naming its encoded value"]
fn an_inspection_answer_retains_the_previous_answer_as_its_source() {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{memory} ~= [item]->([red][round])");
    ask(&mut pangine, "{memory} @ {object}->([red][round][heavy])");
    let object = pangine.reference_percept("object");
    let original = pangine.linked_answer_value(&object).expect("original answer");
    let archive = pangine.reference_percept("archive");
    assert!(pangine.set_percept_value(&archive, Some(original.clone())));
    ask(&mut pangine, "{archive} @ [pangine-answer-remainder]->[pangine-answer-question-remainder]->{path}->{missing}");

    let missing = pangine.reference_percept("missing");
    let inspection = pangine.linked_answer_value(&missing).expect("inspection answer");
    let inspection_archive = pangine.reference_percept("inspection-archive");
    assert!(pangine.set_percept_value(&inspection_archive, Some(inspection)));
    ask(&mut pangine, "{inspection-archive} @ [pangine-answer-source]->([pangine-answer-percept-source]->{owner})->{inspected-source}");
    let source = pangine.reference_percept("inspected-source");
    let view = pangine.answer_view(&source).expect("second inspection answer");
    let possibilities = view.possibilities(&mut pangine).expect("second inspection possibilities");
    let source_values = possibilities.iter().map(|possibility| possibility.value().clone()).collect::<BTreeSet<_>>();
    let observation = must_ref(&mut pangine, "[item]->([red][round])");
    assert_eq!(source_values, BTreeSet::from([original.clone(), observation]));

    // A recursive search discovers both the immediate source Answer and its
    // nested observation; the word "source" does not limit the traversal depth.
    assert_eq!(pangine.linked_answer_value(&object), Some(original));
}

fn observations() -> Pangine {
    let mut pangine = Pangine::new();
    pangine.parse_script_text(OBSERVATIONS).expect("observation corpus");
    pangine
}

fn assert_value(pangine: &mut Pangine, input: &str, expected: &str) {
    assert_eq!(must_ref(pangine, input), must_ref(pangine, expected), "{input}");
}

fn ask(pangine: &mut Pangine, input: &str) -> Option<ConceptId> {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    ask(pangine, input).unwrap_or_else(|| panic!("expected nonempty Concept for {input:?}"))
}
