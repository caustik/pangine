//! A small, deliberately lossy evidence view supplied through the Rust API.
//! Its field names have no engine meaning and are not a proposed Answer codec.

use pangine::{CompletionRemainderSide, CompletionResult, ConceptId, Pangine, Relevance};
use std::collections::BTreeSet;

#[test]
#[ignore = "warning: a consumer-supplied evidence view can expose context without establishing a public schema"]
fn a_question_can_relate_a_completed_result_to_observed_and_supplied_structure() {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{memory} ~= [o-1]->([red][round])");
    ask(&mut pangine, "{memory} ~= [o-2]->([red][round][heavy])");
    let memory = pangine.reference_percept("memory");
    let question = must_ref(&mut pangine, "{object}->([red][round][heavy])");
    let original = pangine.complete_question(&[memory], &question).expect("completed context");
    let reports = evidence_records(&mut pangine, &original);
    assert_eq!(reports.len(), 2);

    let inputs = reports
        .into_iter()
        .enumerate()
        .map(|(index, record)| {
            let input = pangine.reference_percept(&format!("report-{index}"));
            assert!(pangine.set_percept_value(&input, Some(record)));
            input
        })
        .collect::<Vec<_>>();
    let inspection = must_ref(&mut pangine, "([result]->{completed})([source]->{observed})([question-remainder]->{supplied})");
    let answer = pangine.complete_question(&inputs, &inspection).expect("evidence inspection");
    let completed = pangine.reference_percept("completed");
    let observed = pangine.reference_percept("observed");
    let supplied = pangine.reference_percept("supplied");
    assert_eq!(answer.completions().len(), 1);
    let row = &answer.completions()[0];
    assert_eq!(row.binding(&completed), Some(&must_ref(&mut pangine, "[o-1]->([red][round][heavy])")));
    assert_eq!(row.binding(&observed), Some(&must_ref(&mut pangine, "[o-1]->([red][round])")));
    assert_eq!(row.binding(&supplied), Some(&must_ref(&mut pangine, "[heavy]")));

    // The adapter records only selected fields. Routes, weights, ownership,
    // revisions, and adjustment history are intentionally not transported.
    assert_eq!(original.completions().len(), 2);
}

#[test]
#[ignore = "warning: explicit enclosing structure can select an immediate evidence field without a depth operator"]
fn evidence_about_evidence_supports_both_recursive_and_scoped_source_questions() {
    let mut pangine = Pangine::new();
    let observation = must_ref(&mut pangine, "[o-1]->([red][round])");
    let question = must_ref(&mut pangine, "{object}->([red][round][heavy])");
    let original = pangine.complete_subject(&observation, &question).expect("original question");
    let mut reports = evidence_records(&mut pangine, &original);
    assert_eq!(reports.len(), 1);
    let record = reports.pop().unwrap();

    let question = must_ref(&mut pangine, "[question-remainder]->{missing}");
    let inspection = pangine.complete_subject(&record, &question).expect("question about evidence");
    let mut reports = evidence_records(&mut pangine, &inspection);
    assert_eq!(reports.len(), 1);
    let inspection_record = reports.pop().unwrap();

    let question = must_ref(&mut pangine, "[source]->{any-source}");
    let recursive = pangine.complete_subject(&inspection_record, &question).expect("recursive source inspection");
    let output = pangine.reference_percept("any-source");
    let sources = recursive.completions().iter().filter_map(|row| row.binding(&output).cloned()).collect::<BTreeSet<_>>();
    assert_eq!(sources, BTreeSet::from([record.clone(), observation]));

    let wrapped = field(&mut pangine, "here", inspection_record);
    let question = must_ref(&mut pangine, "[here]->(([source]->{immediate}){other-fields})");
    let immediate = pangine.complete_subject(&wrapped, &question).expect("explicitly scoped source inspection");
    let output = pangine.reference_percept("immediate");
    assert_eq!(immediate.completions().len(), 1);
    assert_eq!(immediate.completions()[0].binding(&output), Some(&record));
}

#[test]
#[ignore = "warning: a represented identity and shared blank connect an enclosing value to its nested fields"]
fn an_explicit_record_identity_keeps_whole_records_with_their_nested_parts() {
    let mut pangine = Pangine::new();
    ask(&mut pangine, "{records} = ([row-1]->(([left]->[A])([right]->[B]))) ([row-2]->(([left]->[B])([right]->[A])))");
    ask(&mut pangine, "{records} @ ({id}->{whole})({id}->(([left]->{left})([right]->{right})))");
    assert_eq!(
        must_ref(&mut pangine, "$({id}->{whole}->{left}->{right})"),
        must_ref(&mut pangine, "([row-1]->(([left]->[A])([right]->[B]))->[A]->[B])([row-2]->(([left]->[B])([right]->[A]))->[B]->[A])")
    );
}

#[test]
#[ignore = "warning: an open record pattern requires a nonempty remainder and therefore has a singleton boundary"]
fn a_scoped_field_pattern_does_not_yet_cover_both_singleton_and_larger_records() {
    let mut pangine = Pangine::new();
    let open = must_ref(&mut pangine, "[here]->(([source]->{open-source}){rest})");
    let singleton = must_ref(&mut pangine, "[here]->([source]->{only-source})");
    let subjects = [
        "[here]->([source]->[observation])",
        "[here]->(([source]->[observation])([result]->[completed]))",
        "[here]->(([source]->[observation])([result]->[completed])([context]->[extra]))",
    ];
    for (index, subject) in subjects.into_iter().enumerate() {
        let subject = must_ref(&mut pangine, subject);
        let open_matches = pangine.complete_subject(&subject, &open).expect("open record question");
        let singleton_matches = pangine.complete_subject(&subject, &singleton).expect("singleton record question");
        assert_eq!(open_matches.completions().len(), usize::from(index != 0));
        assert_eq!(singleton_matches.completions().len(), usize::from(index == 0));
    }
}

#[test]
#[ignore = "warning: a repeated enclosing label remains searchable at every depth and is not a root-only matching operation"]
fn an_enclosing_relationship_is_only_as_specific_as_the_structure_it_names() {
    let mut pangine = Pangine::new();
    let subject = must_ref(&mut pangine, "[here]->(([source]->[outer])([nested]->([here]->(([source]->[inner])([other]->[field])))))");
    let question = must_ref(&mut pangine, "[here]->(([source]->{source}){rest})");
    let result = pangine.complete_subject(&subject, &question).expect("nested enclosing relationships");
    let output = pangine.reference_percept("source");
    let sources = result.completions().iter().filter_map(|row| row.binding(&output).cloned()).collect::<BTreeSet<_>>();
    assert_eq!(sources, BTreeSet::from([must_ref(&mut pangine, "[outer]"), must_ref(&mut pangine, "[inner]")]));
}

// This is an ordinary consumer's view of a result, not a lossless proof format.
// One record relates one complete result to one of its evidence fragments.
// Keeping those fields in the same record gives later questions their context.
fn evidence_records(pangine: &mut Pangine, result: &CompletionResult) -> Vec<ConceptId> {
    let mut records = Vec::new();
    for row in result.completions() {
        let completed = pangine.instantiate_completion(result.question(), row).expect("complete result");
        for evidence in row.evidence() {
            let mut fields = vec![
                field(pangine, "result", completed.clone()),
                field(pangine, "source", evidence.source_concept().clone()),
                field(pangine, "matched", evidence.matched().clone()),
            ];
            for remainder in evidence.remainders() {
                let label = match remainder.side() {
                    CompletionRemainderSide::Source => "source-remainder",
                    CompletionRemainderSide::Question => "question-remainder",
                };
                fields.push(field(pangine, label, remainder.concept().clone()));
            }
            let fields = fields.into_iter().map(|field| (Relevance::DEFAULT, field)).collect::<Vec<_>>();
            records.push(pangine.compose_union(&fields).expect("owned fields").expect("nonempty record"));
        }
    }
    records
}

fn field(pangine: &mut Pangine, name: &str, value: ConceptId) -> ConceptId {
    let label = pangine.reference_name(name);
    pangine.compose_ordered(&[label, value]).expect("owned field").expect("nonempty field")
}

fn ask(pangine: &mut Pangine, input: &str) -> Option<ConceptId> {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    ask(pangine, input).unwrap_or_else(|| panic!("expected a nonempty Concept for {input:?}"))
}
