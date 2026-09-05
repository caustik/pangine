//! A bounded candidate for asking inside a directly bound source value.
//! It keeps the parent rows and original sources, and compares recording the
//! selected view as another ordinary value. It is not a public operation.

use super::super::super::concept_answer::ConceptAnswer;
use super::{bound_values, complete_view, complete_whole, must_ref, CompletionResult, ConceptId, Pangine};
use std::collections::BTreeSet;

#[test]
#[ignore = "warning: this staged inquiry comparison retains direct source context but does not define a public scoped question"]
fn staged_questions_keep_record_pairings_and_the_same_original_sources() {
    let mut pangine = records(false);
    let memory = pangine.reference_percept("memory");
    let remembered = pangine.get_value(&memory);
    let parent_question = must_ref(&mut pangine, "{id}->{group}");
    let parent = pangine.complete_question(std::slice::from_ref(&memory), &parent_question).expect("record question");
    let group = pangine.reference_percept("group");
    let child_question = must_ref(&mut pangine, "([left]->{left})([right]->{right})");
    let staged = inquire_within(&mut pangine, &parent, &group, &child_question);
    let direct_question = must_ref(&mut pangine, "{id}->(([left]->{left})([right]->{right}))");
    let direct = pangine.complete_question(std::slice::from_ref(&memory), &direct_question).expect("direct nested question");
    let projection = must_ref(&mut pangine, "{id}->{left}->{right}");
    assert_eq!(staged.completions().len(), 2);
    assert_eq!(project(&mut pangine, &staged, &projection), project(&mut pangine, &direct, &projection));
    assert_eq!(project(&mut pangine, &staged, &projection), must_ref(&mut pangine, "([o-1]->[A]->[B])([o-2]->[B]->[A])"));
    assert!(staged.completions().iter().all(|row| row.evidence().len() == 2));
    assert_eq!(source_values(&staged), source_values(&parent));
    assert_eq!(pangine.get_value(&memory), remembered);

    // The ordinary codec can still expose the original complete records.
    let encoded = ConceptAnswer::from_result(&pangine, &staged).encode(&mut pangine);
    let question = must_ref(&mut pangine, "[pangine-answer-source]->([pangine-answer-percept-source]->{owner})->{record-source}");
    let inspection = pangine.complete_subject(&encoded, &question).expect("ordinary question about staged evidence");
    let output = pangine.reference_percept("record-source");
    let owner = pangine.reference_percept("owner");
    assert_eq!(bound_values(&inspection, &output), source_values(&parent));
    assert_eq!(bound_values(&inspection, &owner), BTreeSet::from([memory]));
}

#[test]
#[ignore = "warning: identical selected values can keep distinct parent record identities and their sources"]
fn equal_group_values_under_distinct_records_keep_their_parent_bindings() {
    let mut pangine = records(true);
    let memory = pangine.reference_percept("memory");
    let question = must_ref(&mut pangine, "{id}->{group}");
    let parent = pangine.complete_question(&[memory], &question).expect("record question");
    let group = pangine.reference_percept("group");
    let child = must_ref(&mut pangine, "([left]->{left})([right]->{right})");
    let staged = inquire_within(&mut pangine, &parent, &group, &child);
    let projection = must_ref(&mut pangine, "{id}->{left}->{right}");
    assert_eq!(project(&mut pangine, &staged, &projection), must_ref(&mut pangine, "([o-1]->[A]->[B])([o-2]->[A]->[B])"));
    assert_eq!(source_values(&staged), source_values(&parent));
    assert_eq!(source_values(&staged).len(), 2);
}

#[test]
#[ignore = "warning: reusing the original source keeps repeated staged inspection from introducing another source contribution"]
fn repeating_a_focused_question_keeps_the_same_source_support_in_this_probe() {
    let mut pangine = records(false);
    let memory = pangine.reference_percept("memory");
    let question = must_ref(&mut pangine, "{id}->{group}");
    let parent = pangine.complete_question(&[memory], &question).expect("record question");
    let group = pangine.reference_percept("group");
    let child = must_ref(&mut pangine, "([left]->{left})([right]->{right})");
    let once = inquire_within(&mut pangine, &parent, &group, &child);
    let twice = inquire_within(&mut pangine, &once, &group, &child);
    let projection = must_ref(&mut pangine, "{id}->{left}->{right}");
    assert_eq!(project(&mut pangine, &once, &projection), project(&mut pangine, &twice, &projection));
    assert_eq!(source_values(&once), source_values(&twice));
    assert!(once.completions() == twice.completions(), "the same source and local question add no distinct evidence fragment");
}

#[test]
#[ignore = "warning: this candidate loses which equal-valued binding was selected; its result shape and evidence are insufficient to recover the request"]
fn equal_values_at_distinct_positions_expose_the_missing_focus_record() {
    let mut pangine = Pangine::new();
    let source = must_ref(&mut pangine, "([tag]->[A])->([tag]->[A])");
    let question = must_ref(&mut pangine, "{first}->{second}");
    let parent = complete_whole(&mut pangine, &source, &question);
    let first = pangine.reference_percept("first");
    let second = pangine.reference_percept("second");
    let child = must_ref(&mut pangine, "[tag]->{inside}");
    let from_first = inquire_within(&mut pangine, &parent, &first, &child);
    let from_second = inquire_within(&mut pangine, &parent, &second, &child);
    assert_ne!(first, second);
    assert_eq!(from_first.completions().len(), 1);
    assert_eq!(from_first.question(), from_second.question());
    assert!(from_first.completions() == from_second.completions());
}

#[test]
#[ignore = "warning: selecting one bound Percept is not a general Answer projection; this candidate is not closed over ordinary composite views"]
fn a_composite_projection_exposes_the_limit_of_selecting_only_one_binding() {
    let mut pangine = records(false);
    let memory = pangine.reference_percept("memory");
    let question = must_ref(&mut pangine, "{id}->{group}");
    let parent = pangine.complete_question(&[memory], &question).expect("record question");
    let projection = must_ref(&mut pangine, "{id}->{group}");
    let child = must_ref(&mut pangine, "{record}->(([left]->{left})([right]->{right}))");
    assert!(inquire_within(&mut pangine, &parent, &projection, &child).completions().is_empty());
    let mut completed_records = 0;
    for row in parent.completions() {
        let value = pangine.instantiate_completion(&projection, row).expect("ordinary composite projection");
        completed_records += complete_whole(&mut pangine, &value, &child).completions().len();
    }
    assert_eq!(completed_records, 2);
    // Applying the question to detached projected values would work, but would
    // not by itself establish preservation of the parent context and sources.
}

#[test]
#[ignore = "warning: a complete inquiry record preserves the selected view while its tuple representation remains a test-only convention"]
fn recording_subject_question_and_answer_keeps_equal_valued_selections_distinct() {
    let mut pangine = Pangine::new();
    let source = must_ref(&mut pangine, "([tag]->[A])->([tag]->[A])");
    let question = must_ref(&mut pangine, "{first}->{second}");
    let answer = complete_whole(&mut pangine, &source, &question);
    let parent = inquiry_record(&mut pangine, source, question, &answer);
    let first = pangine.reference_percept("first");
    let second = pangine.reference_percept("second");
    let child = must_ref(&mut pangine, "[tag]->{inside}");
    let from_first = inquire_record(&mut pangine, &parent, &first, &child);
    let from_second = inquire_record(&mut pangine, &parent, &second, &child);
    assert_ne!(from_first, from_second);

    let [first_subject, first_question, first_answer] = record_parts(&mut pangine, &from_first);
    let [second_subject, second_question, second_answer] = record_parts(&mut pangine, &from_second);
    assert_eq!(first_question, second_question);
    assert_eq!(first_answer, second_answer);
    let [first_parent, first_selected] = view_parts(&mut pangine, &first_subject);
    let [second_parent, second_selected] = view_parts(&mut pangine, &second_subject);
    assert_eq!(first_parent, parent);
    assert_eq!(second_parent, parent);
    assert_eq!(first_selected, first);
    assert_eq!(second_selected, second);
}

#[test]
#[ignore = "warning: a staged inquiry can be replayed from its captured parent Answer after transport; this does not replay arbitrary mutable source state"]
fn an_inquiry_about_an_inquiry_keeps_its_history_and_replays_in_another_engine() {
    let mut pangine = records(false);
    let memory = pangine.reference_percept("memory");
    let question = must_ref(&mut pangine, "{id}->{group}");
    let answer = pangine.complete_question(std::slice::from_ref(&memory), &question).expect("initial record query");
    let root = inquiry_record(&mut pangine, memory, question, &answer);
    let group = pangine.reference_percept("group");
    let fields = must_ref(&mut pangine, "([left]->{left})([right]->{right})");
    let first = inquire_record(&mut pangine, &root, &group, &fields);
    let left = pangine.reference_percept("left");
    let leaf = pangine.reference_percept("leaf");
    let second = inquire_record(&mut pangine, &first, &left, &leaf);
    let text = pangine.format_concept(&second, false);

    let mut other = Pangine::new();
    let copied = must_ref(&mut other, &text);
    let [subject, question, encoded_answer] = record_parts(&mut other, &copied);
    let [parent, selected] = view_parts(&mut other, &subject);
    let replayed = inquire_record(&mut other, &parent, &selected, &question);
    assert_eq!(replayed, copied);
    let [earlier_subject, _, _] = record_parts(&mut other, &parent);
    let [earliest, _] = view_parts(&mut other, &earlier_subject);
    assert_eq!(other.format_concept(&earliest, false), pangine.format_concept(&root, false));

    let answer = ConceptAnswer::decode(&other, &encoded_answer).expect("transported Answer").to_result(&mut other).expect("transported result");
    let projection = must_ref(&mut other, "{id}->{leaf}");
    assert_eq!(project(&mut other, &answer, &projection), must_ref(&mut other, "([o-1]->[A])([o-2]->[B])"));
    let memory = other.reference_percept("memory");
    assert_eq!(other.get_value(&memory), None, "replay used the captured Answer, not live source state");
}

#[test]
#[ignore = "warning: bounded recursive inquiry records retain history and replay but their eager text size is measured, not an efficient persistence design"]
fn repeated_inquiry_records_preserve_history_and_report_transport_growth() {
    let mut pangine = records(false);
    let memory = pangine.reference_percept("memory");
    let question = must_ref(&mut pangine, "{id}->{group}");
    let answer = pangine.complete_question(std::slice::from_ref(&memory), &question).expect("initial record query");
    let root = inquiry_record(&mut pangine, memory, question, &answer);
    let group = pangine.reference_percept("group");
    let fields = must_ref(&mut pangine, "([left]->{left})([right]->{right})");
    let mut prior = inquire_record(&mut pangine, &root, &group, &fields);
    let mut selected = pangine.reference_percept("left");
    for depth in 1..=8 {
        let leaf = pangine.reference_percept(&format!("leaf-{depth}"));
        let record = inquire_record(&mut pangine, &prior, &selected, &leaf);
        if [1, 2, 4, 8].contains(&depth) {
            let text = pangine.format_concept(&record, false);
            let mut other = Pangine::new();
            let copied = must_ref(&mut other, &text);
            let [subject, question, encoded] = record_parts(&mut other, &copied);
            let [parent, focus] = view_parts(&mut other, &subject);
            assert_eq!(inquire_record(&mut other, &parent, &focus, &question), copied);
            let result = ConceptAnswer::decode(&other, &encoded).expect("record Answer").to_result(&mut other).expect("record result");
            let projection = must_ref(&mut other, &format!("{{id}}->{{leaf-{depth}}}"));
            assert_eq!(project(&mut other, &result, &projection), must_ref(&mut other, "([o-1]->[A])([o-2]->[B])"));
            assert_eq!(source_values(&result).len(), 2);

            let mut earlier = copied;
            for _ in 0..=depth {
                let [subject, _, _] = record_parts(&mut other, &earlier);
                [earlier, _] = view_parts(&mut other, &subject);
            }
            assert_eq!(other.format_concept(&earlier, false), pangine.format_concept(&root, false));
            println!(
                "staged inquiry: leaf depth={depth}, text bytes={}, rows={}, sources={}",
                text.len(),
                result.completions().len(),
                source_values(&result).len()
            );
        }
        prior = record;
        selected = leaf;
    }
}

// An inquiry is represented here by its subject, pattern, and encoded Answer.
// A staged subject contains the preceding inquiry and the selected projection.
// The tuples are a consumer convention for this experiment, not parser syntax
// or a proposed public codec. The existing whole matcher inspects their fields.
fn inquiry_record(pangine: &mut Pangine, subject: ConceptId, question: ConceptId, answer: &CompletionResult) -> ConceptId {
    let encoded = ConceptAnswer::from_result(pangine, answer).encode(pangine);
    pangine.compose_ordered(&[subject, question, encoded]).expect("owned inquiry fields").expect("complete inquiry")
}

fn inquire_record(pangine: &mut Pangine, parent: &ConceptId, selected: &ConceptId, question: &ConceptId) -> ConceptId {
    let [_, _, encoded] = record_parts(pangine, parent);
    let prior = ConceptAnswer::decode(pangine, &encoded).expect("captured parent Answer").to_result(pangine).expect("captured parent result");
    let answer = inquire_within(pangine, &prior, selected, question);
    let subject = pangine.compose_ordered(&[parent.clone(), selected.clone()]).expect("owned view").expect("complete view");
    inquiry_record(pangine, subject, question.clone(), &answer)
}

fn record_parts(pangine: &mut Pangine, record: &ConceptId) -> [ConceptId; 3] {
    let question = must_ref(pangine, "{record-subject}->{record-question}->{record-answer}");
    let fields = complete_whole(pangine, record, &question);
    assert_eq!(fields.completions().len(), 1);
    ["record-subject", "record-question", "record-answer"].map(|name| {
        let field = pangine.reference_percept(name);
        fields.completions()[0].binding(&field).expect("whole inquiry field").clone()
    })
}

fn view_parts(pangine: &mut Pangine, view: &ConceptId) -> [ConceptId; 2] {
    let question = must_ref(pangine, "{view-parent}->{view-selected}");
    let fields = complete_whole(pangine, view, &question);
    assert_eq!(fields.completions().len(), 1);
    ["view-parent", "view-selected"].map(|name| {
        let field = pangine.reference_percept(name);
        fields.completions()[0].binding(&field).expect("whole view field").clone()
    })
}

// This comparison is limited to focus bindings supplied directly by evidence
// fragments. It does not infer origins for adjusted or reconstructed values.
// Each child question uses the containing source and is joined only to the
// parent row from which its focus came. This Answer alone does not retain the
// selected binding; the record comparison carries that request context outside it.
fn inquire_within(pangine: &mut Pangine, parent: &CompletionResult, focus: &ConceptId, question: &ConceptId) -> CompletionResult {
    let shape = pangine.answer_shape(&BTreeSet::from([parent.question().clone(), question.clone()])).expect("combined question shape");
    let mut parent_outputs = BTreeSet::new();
    let mut child_outputs = BTreeSet::new();
    pangine.collect_output_percepts(parent.question(), &mut parent_outputs);
    pangine.collect_output_percepts(question, &mut child_outputs);
    let mut rows = BTreeSet::new();
    for row in parent.completions() {
        let Some(value) = row.binding(focus) else {
            continue;
        };
        let parent_row = CompletionResult::from_parts(parent.question().clone(), vec![row.clone()]);
        for fragment in row.evidence().iter().filter(|fragment| fragment.binding(focus) == Some(value)) {
            let source_view = &fragment.source.source_view;
            let child = complete_view(pangine, &source_view.source, value, question, &source_view.routes);
            let extended = pangine.join_completion_results(&parent_row, &parent_outputs, &child, &child_outputs, &shape);
            rows.extend(extended.completions);
        }
    }
    CompletionResult::from_parts(shape, rows.into_iter().collect())
}

fn source_values(result: &CompletionResult) -> BTreeSet<ConceptId> {
    result.completions().iter().flat_map(|row| row.evidence().iter().map(|fragment| fragment.source_concept().clone())).collect()
}

fn project(pangine: &mut Pangine, result: &CompletionResult, projection: &ConceptId) -> ConceptId {
    pangine.materialize_completion_projection(result, projection).expect("nonempty projection")
}

fn records(equal: bool) -> Pangine {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{memory} ~= [o-1]->(([left]->[A])([right]->[B]))");
    let second = if equal { "[o-2]->(([left]->[A])([right]->[B]))" } else { "[o-2]->(([left]->[B])([right]->[A]))" };
    must_ref(&mut pangine, &format!("{{memory}} ~= {second}"));
    pangine
}
