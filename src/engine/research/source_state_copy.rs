//! Test-only probes for copying retained Percept source state.
//!
//! The copy below operates on the engine's direct source map. It does not read
//! completion evidence, repeat experience calls, define snapshot syntax, or
//! establish that every Percept should be copyable this way.
//! Percept references inside copied Concepts remain live references.

use super::super::{ConceptId, ConceptKind, ConceptMap, Pangine};
use crate::Relevance;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
struct DecisionState {
    candidates: BTreeMap<String, Relevance>,
    selected: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct HistoryRecord {
    choice: String,
    role: String,
    source_percept: ConceptId,
    source_relevance: Relevance,
    coefficient: Option<Relevance>,
}

#[derive(Debug, PartialEq, Eq)]
struct ChoiceHistory {
    choice: String,
    source_percept: ConceptId,
    source_relevance: Relevance,
    coefficient: Option<Relevance>,
}

#[test]
#[ignore = "warning: direct Percept source-state copy is an engine experiment, not accepted snapshot syntax"]
fn direct_source_state_copy_preserves_decision_time_weights_and_histories() {
    let mut pangine = decision_fixture();
    run_stance_program(&mut pangine, "['archive']['stance']", "original");
    assert_eq!(decision_state(&mut pangine, "original-positive"), state(&[("[A]", 4), ("[B]", 2), ("[C]", 2)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "original-negative"), state(&[("[A]", 1), ("[B]", 1), ("[C]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "original-net"), state(&[("[A]", 3), ("[B]", 1), ("[C]", 1)], Some("[A]")));

    must_ref(&mut pangine, "['evaluated-archive'] = $['archive']");
    must_ref(&mut pangine, "['evaluated-stance'] = $['stance']");

    let archive = pangine.reference_percept("archive");
    let stance = pangine.reference_percept("stance");
    let decision_archive = pangine.reference_percept("decision-archive");
    let decision_stance = pangine.reference_percept("decision-stance");
    let archive_sources = pangine.get_relevance_map(&archive);
    let stance_sources = pangine.get_relevance_map(&stance);
    let archive_value = pangine.get_value(&archive);
    let stance_value = pangine.get_value(&stance);

    assert_eq!(copy_direct_source_state(&mut pangine, &archive, &decision_archive), archive_value);
    assert_eq!(copy_direct_source_state(&mut pangine, &stance, &decision_stance), stance_value);
    assert_eq!(pangine.get_relevance_map(&decision_archive), archive_sources);
    assert_eq!(pangine.get_relevance_map(&decision_stance), stance_sources);

    run_stance_program(&mut pangine, "['decision-archive']['decision-stance']", "copy");
    assert_eq!(decision_state(&mut pangine, "copy-positive"), state(&[("[A]", 4), ("[B]", 2), ("[C]", 2)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "copy-negative"), state(&[("[A]", 1), ("[B]", 1), ("[C]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "copy-net"), state(&[("[A]", 3), ("[B]", 1), ("[C]", 1)], Some("[A]")));

    let histories = history_completions(&mut pangine, &decision_archive, &decision_stance);
    assert_eq!(histories, expected_histories(&decision_archive));

    run_stance_program(&mut pangine, "['evaluated-archive']['evaluated-stance']", "evaluated");
    assert_eq!(decision_state(&mut pangine, "evaluated-positive"), state(&[("[A]", 1), ("[B]", 1), ("[C]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "evaluated-negative"), state(&[("[A]", 1), ("[B]", 1), ("[C]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "evaluated-net"), state(&[], None));

    must_ref(&mut pangine, "['copy-pointer-record'] = [decision]->[source]->['decision-archive']");
    must_ref(&mut pangine, "['copy-pointer-record'] @ [decision]->[source]->['record-source']");
    let record_source = pangine.reference_percept("record-source");
    assert_eq!(pangine.get_value(&record_source), Some(decision_archive.clone()));
    run_stance_program(&mut pangine, "['record-source']['decision-stance']", "indirect");
    assert_eq!(decision_state(&mut pangine, "indirect-net"), state(&[], None));
    run_stance_program(&mut pangine, "^['record-source']['decision-stance']", "selected-source");
    assert_eq!(decision_state(&mut pangine, "selected-source-net"), state(&[("[A]", 3), ("[B]", 1), ("[C]", 1)], Some("[A]")));

    experience(&mut pangine, "archive", "[repeat-a]->[amber]->[A]", 2);
    experience(&mut pangine, "archive", "[later-b]->[amber]->[B]", 6);
    run_stance_program(&mut pangine, "['archive']['stance']", "current");
    assert_eq!(decision_state(&mut pangine, "current-positive"), state(&[("[A]", 6), ("[B]", 8), ("[C]", 2)], Some("[B]")));
    assert_eq!(decision_state(&mut pangine, "current-negative"), state(&[("[A]", 1), ("[B]", 1), ("[C]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "current-net"), state(&[("[A]", 5), ("[B]", 7), ("[C]", 1)], Some("[B]")));

    assert_ne!(pangine.get_relevance_map(&archive), archive_sources);
    assert_eq!(pangine.get_relevance_map(&decision_archive), archive_sources);
    assert_eq!(pangine.get_value(&decision_archive), archive_value);
    assert_eq!(history_completions(&mut pangine, &decision_archive, &decision_stance), histories);
    run_stance_program(&mut pangine, "['decision-archive']['decision-stance']", "replay");
    assert_eq!(decision_state(&mut pangine, "replay-net"), state(&[("[A]", 3), ("[B]", 1), ("[C]", 1)], Some("[A]")));

    assert!(pangine.set_percept_value(&archive, None));
    assert!(pangine.get_relevance_map(&archive).is_empty());
    assert_eq!(pangine.get_relevance_map(&decision_archive), archive_sources);
    run_stance_program(&mut pangine, "['decision-archive']['decision-stance']", "after-source-clear");
    assert_eq!(decision_state(&mut pangine, "after-source-clear-net"), state(&[("[A]", 3), ("[B]", 1), ("[C]", 1)], Some("[A]")));

    experience(&mut pangine, "decision-archive", "[copy-only]->[amber]->[C]", 1);
    assert!(pangine.get_relevance_map(&archive).is_empty());
    assert_ne!(pangine.get_relevance_map(&decision_archive), archive_sources, "the copied Percept is independent but still mutable");
}

#[test]
#[ignore = "warning: equal evaluated values can hide different direct source histories"]
fn source_state_copy_distinguishes_histories_that_evaluated_assignment_collapses() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['whole'] = [A][B]");
    experience(&mut pangine, "split", "[A]", 1);
    experience(&mut pangine, "split", "[B]", 1);
    let whole = pangine.reference_percept("whole");
    let split = pangine.reference_percept("split");
    assert_eq!(pangine.get_value(&whole), pangine.get_value(&split));
    assert_ne!(pangine.get_relevance_map(&whole), pangine.get_relevance_map(&split));

    let whole_copy = pangine.reference_percept("whole-copy");
    let split_copy = pangine.reference_percept("split-copy");
    copy_direct_source_state(&mut pangine, &whole, &whole_copy);
    copy_direct_source_state(&mut pangine, &split, &split_copy);
    assert_eq!(pangine.get_relevance_map(&whole_copy), pangine.get_relevance_map(&whole));
    assert_eq!(pangine.get_relevance_map(&split_copy), pangine.get_relevance_map(&split));
    assert_ne!(pangine.get_relevance_map(&whole_copy), pangine.get_relevance_map(&split_copy));

    must_ref(&mut pangine, "['whole-evaluated'] = $['whole']");
    must_ref(&mut pangine, "['split-evaluated'] = $['split']");
    let whole_evaluated = pangine.reference_percept("whole-evaluated");
    let split_evaluated = pangine.reference_percept("split-evaluated");
    assert_eq!(pangine.get_relevance_map(&whole_evaluated), pangine.get_relevance_map(&split_evaluated));
    assert_eq!(pangine.get_relevance_map(&whole_evaluated).len(), 1);

    experience(&mut pangine, "repeated", "[C]", 2);
    experience(&mut pangine, "coefficient", "x2[C]", 1);
    let repeated = pangine.reference_percept("repeated");
    let coefficient = pangine.reference_percept("coefficient");
    assert_eq!(pangine.get_value(&repeated), pangine.get_value(&coefficient));
    assert_ne!(pangine.get_relevance_map(&repeated), pangine.get_relevance_map(&coefficient));

    let repeated_copy = pangine.reference_percept("repeated-copy");
    let coefficient_copy = pangine.reference_percept("coefficient-copy");
    copy_direct_source_state(&mut pangine, &repeated, &repeated_copy);
    copy_direct_source_state(&mut pangine, &coefficient, &coefficient_copy);
    assert_eq!(pangine.get_relevance_map(&repeated_copy), pangine.get_relevance_map(&repeated));
    assert_eq!(pangine.get_relevance_map(&coefficient_copy), pangine.get_relevance_map(&coefficient));
    assert_ne!(pangine.get_relevance_map(&repeated_copy), pangine.get_relevance_map(&coefficient_copy));

    must_ref(&mut pangine, "['repeated-evaluated'] = $['repeated']");
    must_ref(&mut pangine, "['coefficient-evaluated'] = $['coefficient']");
    let repeated_evaluated = pangine.reference_percept("repeated-evaluated");
    let coefficient_evaluated = pangine.reference_percept("coefficient-evaluated");
    assert_eq!(pangine.get_relevance_map(&repeated_evaluated), pangine.get_relevance_map(&coefficient_evaluated));
    assert_eq!(pangine.get_relevance_map(&repeated_evaluated).len(), 1);
}

#[test]
#[ignore = "warning: copying a record keeps references to mutable Percepts live"]
fn copying_a_record_does_not_freeze_the_mutable_percept_it_references() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "signal", "[A]", 2);
    experience(&mut pangine, "signal", "[B]", 1);
    must_ref(&mut pangine, "['live-record'] = [decision]->[source]->['signal']");

    let signal = pangine.reference_percept("signal");
    let live_record = pangine.reference_percept("live-record");
    let copied_record = pangine.reference_percept("copied-record");
    let signal_sources = pangine.get_relevance_map(&signal);
    let live_record_value = pangine.get_value(&live_record);
    assert_eq!(copy_direct_source_state(&mut pangine, &live_record, &copied_record), live_record_value);
    assert_eq!(pangine.get_relevance_map(&copied_record), pangine.get_relevance_map(&live_record));

    must_ref(&mut pangine, "['live-record'] @ [decision]->[source]->['live-source']");
    must_ref(&mut pangine, "['copied-record'] @ [decision]->[source]->['copied-source']");
    let live_source = pangine.reference_percept("live-source");
    let copied_source = pangine.reference_percept("copied-source");
    assert_eq!(pangine.get_value(&live_source), Some(signal.clone()));
    assert_eq!(pangine.get_value(&copied_source), Some(signal.clone()));
    run_selected_source_decision(&mut pangine, "live-source", "live-before");
    run_selected_source_decision(&mut pangine, "copied-source", "copied-before");
    assert_eq!(decision_state(&mut pangine, "live-before-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "copied-before-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));
    let evaluated_record_before = must_ref(&mut pangine, "$['copied-record']");

    let versioned_signal = pangine.reference_percept("versioned-signal");
    assert_eq!(copy_direct_source_state(&mut pangine, &signal, &versioned_signal), pangine.get_value(&signal));
    must_ref(&mut pangine, "['versioned-record'] = [decision]->[source]->['versioned-signal']");
    must_ref(&mut pangine, "['versioned-record'] @ [decision]->[source]->['versioned-source']");

    experience(&mut pangine, "signal", "[B]", 2);
    assert_ne!(pangine.get_relevance_map(&signal), signal_sources);
    assert_eq!(pangine.get_relevance_map(&versioned_signal), signal_sources);
    assert_eq!(pangine.get_value(&copied_record), live_record_value);
    assert_ne!(must_ref(&mut pangine, "$['copied-record']"), evaluated_record_before);

    run_selected_source_decision(&mut pangine, "live-source", "live-after");
    run_selected_source_decision(&mut pangine, "copied-source", "copied-after");
    run_selected_source_decision(&mut pangine, "versioned-source", "versioned-after");
    assert_eq!(decision_state(&mut pangine, "live-after-candidate"), state(&[("[B]", 3), ("[A]", 2)], Some("[B]")));
    assert_eq!(decision_state(&mut pangine, "copied-after-candidate"), state(&[("[B]", 3), ("[A]", 2)], Some("[B]")));
    assert_eq!(decision_state(&mut pangine, "versioned-after-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));
}

#[test]
#[ignore = "warning: direct source-state copy clears empty targets but does not rewire reference cycles"]
fn direct_source_copy_handles_empty_state_but_does_not_rewire_reference_cycles() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['empty-copy'] = [old]");
    let empty = pangine.reference_percept("empty");
    let empty_copy = pangine.reference_percept("empty-copy");
    assert_eq!(copy_direct_source_state(&mut pangine, &empty, &empty_copy), None);
    assert!(pangine.get_relevance_map(&empty_copy).is_empty());
    experience(&mut pangine, "empty", "[later]", 1);
    assert!(pangine.get_relevance_map(&empty_copy).is_empty());

    must_ref(&mut pangine, "['left'] = ['right']; ['right'] = ['left']");
    let left = pangine.reference_percept("left");
    let right = pangine.reference_percept("right");
    let left_copy = pangine.reference_percept("left-copy");
    let right_copy = pangine.reference_percept("right-copy");
    assert_eq!(copy_direct_source_state(&mut pangine, &left, &left_copy), Some(right.clone()));
    assert_eq!(copy_direct_source_state(&mut pangine, &right, &right_copy), Some(left.clone()));
    assert_eq!(pangine.get_relevance_map(&left_copy), vec![(Relevance::DEFAULT, right.clone())]);
    assert_eq!(pangine.get_relevance_map(&right_copy), vec![(Relevance::DEFAULT, left.clone())]);

    must_ref(&mut pangine, "['right'] = [resolved]");
    let resolved = must_ref(&mut pangine, "[resolved]");
    assert_eq!(must_ref(&mut pangine, "$['left-copy']"), resolved);
    assert_eq!(must_ref(&mut pangine, "$['right-copy']"), resolved);
    assert_eq!(pangine.get_relevance_map(&left_copy), vec![(Relevance::DEFAULT, right)]);
    assert_eq!(pangine.get_relevance_map(&right_copy), vec![(Relevance::DEFAULT, left)]);
}

#[test]
#[ignore = "warning: represented capture scope is an engine experiment, not accepted versioning semantics"]
fn represented_capture_scope_can_version_selected_references_and_leave_others_live() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "signal", "[A]", 2);
    experience(&mut pangine, "signal", "[B]", 1);
    must_ref(&mut pangine, "['context'] = [north]");
    must_ref(&mut pangine, "['shared'] = [shared-old]");
    must_ref(&mut pangine, "['left'] = ['right']; ['right'] = ['left']");
    must_ref(
        &mut pangine,
        "['decision-record'] =
           ([decision]->[source]->['signal'])
           ([decision]->[context]->['context'])
           ([decision]->[shared-one]->['shared'])
           ([decision]->[shared-two]->['shared'])
           ([decision]->[cycle]->['left'])",
    );
    must_ref(
        &mut pangine,
        "['capture-scope'] =
           (([decision]->[capture]->[fixed])([decision]->[source]->['decision-record'])([decision]->[target]->['decision-record-v1']))
           (([decision]->[capture]->[fixed])([decision]->[source]->['signal'])([decision]->[target]->['signal-v1']))
           (([decision]->[capture]->[fixed])([decision]->[source]->['shared'])([decision]->[target]->['shared-v1']))
           (([decision]->[capture]->[fixed])([decision]->[source]->['left'])([decision]->[target]->['left-v1']))
           (([decision]->[capture]->[fixed])([decision]->[source]->['right'])([decision]->[target]->['right-v1']))
           (([decision]->[capture]->[live])([decision]->[source]->['context'])([decision]->[target]->['unused-context-v1']))",
    );
    let mapping_question_text = "([decision]->[capture]->[fixed])([decision]->[source]->['version-source'])([decision]->[target]->['version-target'])";
    must_ref(&mut pangine, &format!("['capture-rows'] = ['capture-scope'] @ {mapping_question_text}"));

    let capture_rows = pangine.reference_percept("capture-rows");
    let mapping_question = must_ref(&mut pangine, mapping_question_text);
    let version_source = pangine.reference_percept("version-source");
    let version_target = pangine.reference_percept("version-target");
    let versions = copy_represented_source_graph(&mut pangine, &capture_rows, &mapping_question, &version_source, &version_target)
        .expect("valid represented source-version mapping");
    assert_eq!(versions.len(), 5);

    let decision_record = pangine.reference_percept("decision-record");
    let decision_record_v1 = pangine.reference_percept("decision-record-v1");
    let signal = pangine.reference_percept("signal");
    let signal_v1 = pangine.reference_percept("signal-v1");
    let context = pangine.reference_percept("context");
    let unused_context_v1 = pangine.reference_percept("unused-context-v1");
    let shared = pangine.reference_percept("shared");
    let shared_v1 = pangine.reference_percept("shared-v1");
    let left = pangine.reference_percept("left");
    let right = pangine.reference_percept("right");
    let left_v1 = pangine.reference_percept("left-v1");
    let right_v1 = pangine.reference_percept("right-v1");
    assert_eq!(versions.get(&decision_record), Some(&decision_record_v1));
    assert_eq!(versions.get(&signal), Some(&signal_v1));
    assert_eq!(versions.get(&shared), Some(&shared_v1));
    assert_eq!(versions.get(&left), Some(&left_v1));
    assert_eq!(versions.get(&right), Some(&right_v1));
    assert!(!versions.contains_key(&context));
    assert!(pangine.get_relevance_map(&unused_context_v1).is_empty());
    assert_eq!(pangine.get_relevance_map(&left_v1), vec![(Relevance::DEFAULT, right_v1.clone())]);
    assert_eq!(pangine.get_relevance_map(&right_v1), vec![(Relevance::DEFAULT, left_v1.clone())]);

    for (relation, holder) in [
        ("source", "versioned-record-source"),
        ("context", "versioned-record-context"),
        ("shared-one", "versioned-record-shared-one"),
        ("shared-two", "versioned-record-shared-two"),
        ("cycle", "versioned-record-cycle"),
    ] {
        must_ref(&mut pangine, &format!("['decision-record-v1'] @ [decision]->[{relation}]->['{holder}']"));
    }
    let versioned_record_source = pangine.reference_percept("versioned-record-source");
    let versioned_record_context = pangine.reference_percept("versioned-record-context");
    let versioned_record_shared_one = pangine.reference_percept("versioned-record-shared-one");
    let versioned_record_shared_two = pangine.reference_percept("versioned-record-shared-two");
    let versioned_record_cycle = pangine.reference_percept("versioned-record-cycle");
    assert_eq!(pangine.get_value(&versioned_record_source), Some(signal_v1.clone()));
    assert_eq!(pangine.get_value(&versioned_record_context), Some(context.clone()));
    assert_eq!(pangine.get_value(&versioned_record_shared_one), Some(shared_v1.clone()));
    assert_eq!(pangine.get_value(&versioned_record_shared_two), Some(shared_v1.clone()));
    assert_eq!(pangine.get_value(&versioned_record_cycle), Some(left_v1.clone()));

    must_ref(&mut pangine, "['decision-record'] @ [decision]->[source]->['live-record-source']");
    run_selected_source_decision(&mut pangine, "live-record-source", "live-scope-before");
    run_selected_source_decision(&mut pangine, "versioned-record-source", "versioned-scope-before");
    assert_eq!(decision_state(&mut pangine, "live-scope-before-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "versioned-scope-before-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));

    experience(&mut pangine, "signal", "[B]", 2);
    must_ref(&mut pangine, "['context'] = [south]");
    must_ref(&mut pangine, "['shared'] = [shared-new]");
    must_ref(&mut pangine, "['right'] = [resolved-original]");
    run_selected_source_decision(&mut pangine, "live-record-source", "live-scope-after");
    run_selected_source_decision(&mut pangine, "versioned-record-source", "versioned-scope-after");
    assert_eq!(decision_state(&mut pangine, "live-scope-after-candidate"), state(&[("[B]", 3), ("[A]", 2)], Some("[B]")));
    assert_eq!(decision_state(&mut pangine, "versioned-scope-after-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));
    assert_eq!(must_ref(&mut pangine, "$['versioned-record-context']"), must_ref(&mut pangine, "[south]"));
    assert_eq!(must_ref(&mut pangine, "$['versioned-record-shared-one']"), must_ref(&mut pangine, "[shared-old]"));
    assert_eq!(must_ref(&mut pangine, "$['versioned-record-shared-two']"), must_ref(&mut pangine, "[shared-old]"));
    assert_eq!(must_ref(&mut pangine, "$['left']"), must_ref(&mut pangine, "[resolved-original]"));
    assert_eq!(must_ref(&mut pangine, "$['versioned-record-cycle']"), left_v1);
}

#[test]
#[ignore = "warning: grounding a represented capture scope fixes its pairs, not the later source-state copy time"]
fn grounded_capture_scope_fixes_selected_pairs_but_not_source_state_time() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "alpha-signal", "[A]", 2);
    experience(&mut pangine, "alpha-signal", "[B]", 1);
    experience(&mut pangine, "beta-signal", "[B]", 2);
    experience(&mut pangine, "beta-signal", "[A]", 1);
    must_ref(&mut pangine, "['alpha-record'] = [bundle]->[needle]->['alpha-signal']");
    must_ref(&mut pangine, "['beta-record'] = [bundle]->[needle]->['beta-signal']");
    must_ref(
        &mut pangine,
        "['opaque-scope'] =
           (([cedar]->[quartz]->[amber])([cedar]->[opal]->['alpha-record'])([cedar]->[basalt]->['alpha-record-v1']))
           (([cedar]->[quartz]->[amber])([cedar]->[opal]->['alpha-signal'])([cedar]->[basalt]->['alpha-signal-v1']))
           (([cedar]->[quartz]->[violet])([cedar]->[opal]->['beta-record'])([cedar]->[basalt]->['unused-beta-record-v1']))
           (([cedar]->[quartz]->[violet])([cedar]->[opal]->['beta-signal'])([cedar]->[basalt]->['unused-beta-signal-v1']))",
    );

    let mapping_question_text = "([cedar]->[quartz]->[amber])([cedar]->[opal]->['quill'])([cedar]->[basalt]->['lantern'])";
    let mapping_question = must_ref(&mut pangine, mapping_question_text);
    let source_binding = pangine.reference_percept("quill");
    let target_binding = pangine.reference_percept("lantern");
    must_ref(&mut pangine, &format!("['scope-at-t0'] = ['opaque-scope'] @ {mapping_question_text}"));

    let opaque_scope = pangine.reference_percept("opaque-scope");
    let scope_at_t0 = pangine.reference_percept("scope-at-t0");
    let alpha_record = pangine.reference_percept("alpha-record");
    let alpha_record_v1 = pangine.reference_percept("alpha-record-v1");
    let alpha_signal = pangine.reference_percept("alpha-signal");
    let alpha_signal_v1 = pangine.reference_percept("alpha-signal-v1");
    let alpha_state_at_t0 = pangine.get_relevance_map(&alpha_signal);

    let direct_t0_versions = copy_represented_source_graph(&mut pangine, &opaque_scope, &mapping_question, &source_binding, &target_binding)
        .expect("opaque live scope should select the alpha pair at t0");
    assert_eq!(direct_t0_versions.len(), 2);
    assert_eq!(direct_t0_versions.get(&alpha_record), Some(&alpha_record_v1));
    assert_eq!(direct_t0_versions.get(&alpha_signal), Some(&alpha_signal_v1));
    assert_eq!(pangine.get_relevance_map(&alpha_signal_v1), alpha_state_at_t0);
    must_ref(&mut pangine, "['alpha-record-v1'] @ [bundle]->[needle]->['alpha-version-source']");
    let alpha_version_source = pangine.reference_percept("alpha-version-source");
    assert_eq!(pangine.get_value(&alpha_version_source), Some(alpha_signal_v1.clone()));
    run_selected_source_decision(&mut pangine, "alpha-version-source", "alpha-at-t0");
    assert_eq!(decision_state(&mut pangine, "alpha-at-t0-candidate"), state(&[("[A]", 2), ("[B]", 1)], Some("[A]")));

    experience(&mut pangine, "alpha-signal", "[B]", 2);
    experience(&mut pangine, "beta-signal", "[A]", 2);
    must_ref(
        &mut pangine,
        "['opaque-scope'] =
           (([cedar]->[quartz]->[amber])([cedar]->[opal]->['beta-record'])([cedar]->[basalt]->['beta-record-v1']))
           (([cedar]->[quartz]->[amber])([cedar]->[opal]->['beta-signal'])([cedar]->[basalt]->['beta-signal-v1']))
           (([cedar]->[quartz]->[violet])([cedar]->[opal]->['alpha-record'])([cedar]->[basalt]->['unused-alpha-record-v1']))
           (([cedar]->[quartz]->[violet])([cedar]->[opal]->['alpha-signal'])([cedar]->[basalt]->['unused-alpha-signal-v1']))",
    );
    let alpha_state_at_t1 = pangine.get_relevance_map(&alpha_signal);
    assert_ne!(alpha_state_at_t1, alpha_state_at_t0);

    let grounded_t0_versions = copy_represented_source_graph(&mut pangine, &scope_at_t0, &mapping_question, &source_binding, &target_binding)
        .expect("the grounded t0 scope should still select the alpha pair");
    assert_eq!(grounded_t0_versions, direct_t0_versions);
    assert_eq!(pangine.get_relevance_map(&alpha_signal_v1), alpha_state_at_t1);
    run_selected_source_decision(&mut pangine, "alpha-version-source", "alpha-copied-at-t1");
    assert_eq!(decision_state(&mut pangine, "alpha-copied-at-t1-candidate"), state(&[("[A]", 2), ("[B]", 3)], Some("[B]")));

    let beta_record = pangine.reference_percept("beta-record");
    let beta_record_v1 = pangine.reference_percept("beta-record-v1");
    let beta_signal = pangine.reference_percept("beta-signal");
    let beta_signal_v1 = pangine.reference_percept("beta-signal-v1");
    let beta_state_at_t1 = pangine.get_relevance_map(&beta_signal);
    let direct_t1_versions = copy_represented_source_graph(&mut pangine, &opaque_scope, &mapping_question, &source_binding, &target_binding)
        .expect("opaque live scope should select the beta pair at t1");
    assert_eq!(direct_t1_versions.len(), 2);
    assert_eq!(direct_t1_versions.get(&beta_record), Some(&beta_record_v1));
    assert_eq!(direct_t1_versions.get(&beta_signal), Some(&beta_signal_v1));
    assert!(!direct_t1_versions.contains_key(&alpha_signal));
    assert_eq!(pangine.get_relevance_map(&beta_signal_v1), beta_state_at_t1);
    must_ref(&mut pangine, "['beta-record-v1'] @ [bundle]->[needle]->['beta-version-source']");
    let beta_version_source = pangine.reference_percept("beta-version-source");
    assert_eq!(pangine.get_value(&beta_version_source), Some(beta_signal_v1));
    run_selected_source_decision(&mut pangine, "beta-version-source", "beta-at-t1");
    assert_eq!(decision_state(&mut pangine, "beta-at-t1-candidate"), state(&[("[A]", 3), ("[B]", 2)], Some("[A]")));
}

#[test]
#[ignore = "warning: equal projected support can retain different captured histories without defining how those histories should rank"]
fn one_call_capture_preserves_equal_support_with_different_experience_histories() {
    let mut pangine = Pangine::new();
    let weighted_experience = "x3(([event-weighted]->[signal]->[mark])([event-weighted]->[answer]->[C]))";
    experience(&mut pangine, "history-body", weighted_experience, 1);
    run_history_decision(&mut pangine, "['history-body']", "coefficient-alone");
    assert_eq!(decision_state(&mut pangine, "coefficient-alone-candidate"), state(&[("[C]", 1)], Some("[C]")));

    experience(&mut pangine, "history-body", weighted_experience, 2);
    experience(&mut pangine, "history-body", &choice_experience("event-repeat", "A"), 3);
    experience(&mut pangine, "history-body", &choice_experience("event-distinct-one", "B"), 1);
    experience(&mut pangine, "history-body", &choice_experience("event-distinct-two", "B"), 1);
    experience(&mut pangine, "history-body", &choice_experience("event-distinct-three", "B"), 1);

    let history_body = pangine.reference_percept("history-body");
    let source_state_at_capture = pangine.get_relevance_map(&history_body);
    let live_histories_at_capture = choice_histories(&mut pangine, &history_body);
    assert_eq!(live_histories_at_capture, equal_support_histories(&history_body));
    run_history_decision(&mut pangine, "['history-body']", "live-at-capture");
    assert_eq!(decision_state(&mut pangine, "live-at-capture-candidate"), state(&[("[A]", 3), ("[B]", 3), ("[C]", 3)], Some("[A]")));

    must_ref(&mut pangine, "['history-record'] = [packet]->[needle]->['history-body']");
    must_ref(
        &mut pangine,
        "['opaque-history-scope'] =
           (([cedar]->[quartz]->[amber])([cedar]->[opal]->['history-record'])([cedar]->[basalt]->['history-record-v1']))
           (([cedar]->[quartz]->[amber])([cedar]->[opal]->['history-body'])([cedar]->[basalt]->['history-body-v1']))",
    );
    let mapping_question =
        must_ref(&mut pangine, "([cedar]->[quartz]->[amber])([cedar]->[opal]->['history-map-source'])([cedar]->[basalt]->['history-map-target'])");
    let source_binding = pangine.reference_percept("history-map-source");
    let target_binding = pangine.reference_percept("history-map-target");
    let opaque_history_scope = pangine.reference_percept("opaque-history-scope");
    let versions = copy_represented_source_graph(&mut pangine, &opaque_history_scope, &mapping_question, &source_binding, &target_binding)
        .expect("the opaque live scope should capture the record and all of its selected experience");

    let history_record = pangine.reference_percept("history-record");
    let history_record_v1 = pangine.reference_percept("history-record-v1");
    let history_body_v1 = pangine.reference_percept("history-body-v1");
    assert_eq!(versions, BTreeMap::from([(history_body.clone(), history_body_v1.clone()), (history_record, history_record_v1)]));
    assert_eq!(pangine.get_relevance_map(&history_body_v1), source_state_at_capture);
    assert_eq!(choice_histories(&mut pangine, &history_body_v1), equal_support_histories(&history_body_v1));

    must_ref(&mut pangine, "['history-record-v1'] @ [packet]->[needle]->['captured-history-source']");
    let captured_history_source = pangine.reference_percept("captured-history-source");
    assert_eq!(pangine.get_value(&captured_history_source), Some(history_body_v1.clone()));
    run_history_decision(&mut pangine, "^['captured-history-source']", "captured-at-copy");
    assert_eq!(decision_state(&mut pangine, "captured-at-copy-candidate"), state(&[("[A]", 3), ("[B]", 3), ("[C]", 3)], Some("[A]")));
    must_ref(
        &mut pangine,
        "^['captured-history-source'] @ x3((['captured-weighted-event']->[signal]->[mark])(['captured-weighted-event']->[answer]->['captured-weighted-choice']))",
    );
    assert_eq!(decision_state(&mut pangine, "captured-weighted-choice"), state(&[("[C]", 3)], Some("[C]")));

    experience(&mut pangine, "history-body", &choice_experience("event-distinct-four", "B"), 1);
    experience(&mut pangine, "history-body", &choice_experience("event-distinct-five", "B"), 1);
    assert_ne!(pangine.get_relevance_map(&history_body), source_state_at_capture);
    run_history_decision(&mut pangine, "['history-body']", "live-after-copy");
    assert_eq!(decision_state(&mut pangine, "live-after-copy-candidate"), state(&[("[A]", 3), ("[B]", 5), ("[C]", 3)], Some("[B]")));

    assert_eq!(pangine.get_relevance_map(&history_body_v1), source_state_at_capture);
    assert_eq!(choice_histories(&mut pangine, &history_body_v1), equal_support_histories(&history_body_v1));
    run_history_decision(&mut pangine, "^['captured-history-source']", "captured-after-live-change");
    assert_eq!(decision_state(&mut pangine, "captured-after-live-change-candidate"), state(&[("[A]", 3), ("[B]", 3), ("[C]", 3)], Some("[A]")));
}

#[test]
#[ignore = "warning: represented source-version mapping validation is an experimental operation contract"]
fn represented_source_version_pairs_deduplicate_equal_proofs_and_reject_ambiguous_identity() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['source-one'] = [one]; ['source-two'] = [two]");
    let mapping_question_text = "(['directive']->[source]->['map-source'])(['directive']->[target]->['map-target'])";
    let mapping_question = must_ref(&mut pangine, mapping_question_text);
    let map_source = pangine.reference_percept("map-source");
    let map_target = pangine.reference_percept("map-target");

    must_ref(
        &mut pangine,
        "['equal-pairs'] =
           (([first]->[source]->['source-one'])([first]->[target]->['target-one']))
           (([second]->[source]->['source-one'])([second]->[target]->['target-one']))",
    );
    let equal_pairs = pangine.reference_percept("equal-pairs");
    let equal_versions = copy_represented_source_graph(&mut pangine, &equal_pairs, &mapping_question, &map_source, &map_target)
        .expect("equal source-version proofs should describe one mapping");
    assert_eq!(equal_versions.len(), 1);
    let source_one = pangine.reference_percept("source-one");
    let target_one = pangine.reference_percept("target-one");
    assert_eq!(equal_versions.get(&source_one), Some(&target_one));
    assert_eq!(pangine.get_relevance_map(&target_one), pangine.get_relevance_map(&source_one));

    must_ref(&mut pangine, "['conflict-target-a'] = [untouched-a]; ['conflict-target-b'] = [untouched-b]");
    must_ref(
        &mut pangine,
        "['one-source-two-targets'] =
           (([first]->[source]->['source-one'])([first]->[target]->['conflict-target-a']))
           (([second]->[source]->['source-one'])([second]->[target]->['conflict-target-b']))",
    );
    let one_source_two_targets = pangine.reference_percept("one-source-two-targets");
    let conflict_target_a = pangine.reference_percept("conflict-target-a");
    let conflict_target_b = pangine.reference_percept("conflict-target-b");
    let conflict_target_a_state = pangine.get_relevance_map(&conflict_target_a);
    let conflict_target_b_state = pangine.get_relevance_map(&conflict_target_b);
    assert_eq!(
        copy_represented_source_graph(&mut pangine, &one_source_two_targets, &mapping_question, &map_source, &map_target),
        Err("one source cannot map to several version targets")
    );
    assert_eq!(pangine.get_relevance_map(&conflict_target_a), conflict_target_a_state);
    assert_eq!(pangine.get_relevance_map(&conflict_target_b), conflict_target_b_state);

    must_ref(&mut pangine, "['shared-conflict-target'] = [untouched-shared]");
    must_ref(
        &mut pangine,
        "['two-sources-one-target'] =
           (([first]->[source]->['source-one'])([first]->[target]->['shared-conflict-target']))
           (([second]->[source]->['source-two'])([second]->[target]->['shared-conflict-target']))",
    );
    let two_sources_one_target = pangine.reference_percept("two-sources-one-target");
    let shared_conflict_target = pangine.reference_percept("shared-conflict-target");
    let shared_conflict_target_state = pangine.get_relevance_map(&shared_conflict_target);
    assert_eq!(
        copy_represented_source_graph(&mut pangine, &two_sources_one_target, &mapping_question, &map_source, &map_target),
        Err("one version target cannot represent several sources")
    );
    assert_eq!(pangine.get_relevance_map(&shared_conflict_target), shared_conflict_target_state);

    must_ref(&mut pangine, "['self-pair'] = ([only]->[source]->['source-one'])([only]->[target]->['source-one'])");
    let self_pair = pangine.reference_percept("self-pair");
    assert_eq!(
        copy_represented_source_graph(&mut pangine, &self_pair, &mapping_question, &map_source, &map_target),
        Err("source-version pairs must contain two different mutable Percepts")
    );
}

fn copy_represented_source_graph(
    pangine: &mut Pangine,
    rows: &ConceptId,
    mapping_question: &ConceptId,
    source_binding: &ConceptId,
    target_binding: &ConceptId,
) -> Result<BTreeMap<ConceptId, ConceptId>, &'static str> {
    let result = pangine.complete_question(std::slice::from_ref(rows), mapping_question).ok_or("invalid represented source-version question")?;
    let mut versions = BTreeMap::new();
    let mut sources_by_target = BTreeMap::new();
    for completion in result.completions() {
        let source = completion.binding(source_binding).ok_or("missing source binding")?.clone();
        let target = completion.binding(target_binding).ok_or("missing target binding")?.clone();
        if !pangine.is_mutable_percept(&source) || !pangine.is_mutable_percept(&target) || source == target {
            return Err("source-version pairs must contain two different mutable Percepts");
        }
        if versions.get(&source).is_some_and(|existing| existing != &target) {
            return Err("one source cannot map to several version targets");
        }
        if sources_by_target.get(&target).is_some_and(|existing| existing != &source) {
            return Err("one version target cannot represent several sources");
        }
        versions.insert(source.clone(), target.clone());
        sources_by_target.insert(target, source);
    }
    copy_source_graph(pangine, &versions)?;
    Ok(versions)
}

fn copy_source_graph(pangine: &mut Pangine, versions: &BTreeMap<ConceptId, ConceptId>) -> Result<(), &'static str> {
    let source_states = versions
        .keys()
        .map(|source| (source.clone(), pangine.percept_subconcepts.get(&source.index()).cloned().unwrap_or_default()))
        .collect::<BTreeMap<_, _>>();
    let mut rewritten_states = BTreeMap::new();
    let mut rewritten_concepts = BTreeMap::new();
    for (source, state) in source_states {
        let target = versions.get(&source).ok_or("missing target")?.clone();
        let mut rewritten_state = ConceptMap::new();
        for (concept, relevance) in state {
            let rewritten = rewrite_versioned_references(pangine, &concept, versions, &mut rewritten_concepts).ok_or("source graph rewrite failed")?;
            pangine.add_relevance(&mut rewritten_state, rewritten, false, relevance).ok_or("source graph relevance overflow")?;
        }
        rewritten_states.insert(target, rewritten_state);
    }
    for (target, state) in rewritten_states {
        pangine.set_percept_subconcepts(&target, state);
    }
    Ok(())
}

fn rewrite_versioned_references(
    pangine: &mut Pangine,
    concept: &ConceptId,
    versions: &BTreeMap<ConceptId, ConceptId>,
    rewritten: &mut BTreeMap<ConceptId, ConceptId>,
) -> Option<ConceptId> {
    if let Some(target) = versions.get(concept) {
        return Some(target.clone());
    }
    if let Some(cached) = rewritten.get(concept) {
        return Some(cached.clone());
    }
    let result = match &concept.0.kind {
        ConceptKind::Named(_) | ConceptKind::Percept { .. } => concept.clone(),
        ConceptKind::Ordered { components } => {
            let components =
                components.iter().map(|component| rewrite_versioned_references(pangine, component, versions, rewritten)).collect::<Option<Vec<_>>>()?;
            pangine.reference_ordered(components)
        }
        ConceptKind::Unordered => {
            let mut map = ConceptMap::new();
            for (child, relevance) in concept.0.subconcepts.clone() {
                let child = rewrite_versioned_references(pangine, &child, versions, rewritten)?;
                pangine.add_relevance(&mut map, child, false, relevance)?;
            }
            pangine.reference_map(&map)?
        }
    };
    rewritten.insert(concept.clone(), result.clone());
    Some(result)
}

fn copy_direct_source_state(pangine: &mut Pangine, source: &ConceptId, target: &ConceptId) -> Option<ConceptId> {
    if !pangine.is_mutable_percept(source) || !pangine.is_mutable_percept(target) {
        return None;
    }

    let sources = pangine.percept_subconcepts.get(&source.index()).cloned().unwrap_or_default();
    pangine.set_percept_subconcepts(target, sources)
}

fn decision_fixture() -> Pangine {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive", "[repeat-a]->[amber]->[A]", 4);
    experience(&mut pangine, "archive", "[distinct-b-one]->[amber]->[B]", 1);
    experience(&mut pangine, "archive", "[distinct-b-two]->[amber]->[B]", 1);
    experience(&mut pangine, "archive", "x3([weighted-c]->[amber]->[C])", 2);
    experience(&mut pangine, "archive", "[counter-a]->[violet]->[A]", 1);
    experience(&mut pangine, "archive", "[counter-b]->[violet]->[B]", 1);
    experience(&mut pangine, "archive", "[counter-c]->[violet]->[C]", 1);
    must_ref(&mut pangine, "['stance'] = ([amber]->[role]->[positive])([violet]->[role]->[negative])");
    pangine
}

fn run_stance_program(pangine: &mut Pangine, selector: &str, prefix: &str) {
    let input = format!(
        "{selector} @ (['{prefix}-positive-event']->['{prefix}-positive-relation']->['{prefix}-positive'])(['{prefix}-positive-relation']->[role]->[positive]);
         {selector} @ (['{prefix}-negative-event']->['{prefix}-negative-relation']->['{prefix}-negative'])(['{prefix}-negative-relation']->[role]->[negative]);
         ['{prefix}-net'] = $['{prefix}-positive'];
         ['{prefix}-net'] /= $['{prefix}-negative']"
    );
    pangine.reference_concept(&input).unwrap_or_else(|error| panic!("failed to run represented stance program: {error}"));
}

fn run_selected_source_decision(pangine: &mut Pangine, source_holder: &str, prefix: &str) {
    must_ref(pangine, &format!("^['{source_holder}'] @ ['{prefix}-candidate']"));
}

fn run_history_decision(pangine: &mut Pangine, selector: &str, prefix: &str) {
    must_ref(pangine, &format!("{selector} @ (['{prefix}-event']->[signal]->[mark])(['{prefix}-event']->[answer]->['{prefix}-candidate'])"));
}

fn choice_experience(event: &str, choice: &str) -> String {
    format!("([{event}]->[signal]->[mark])([{event}]->[answer]->[{choice}])")
}

fn choice_histories(pangine: &mut Pangine, source: &ConceptId) -> BTreeMap<String, ChoiceHistory> {
    let event = pangine.reference_percept("captured-history-event");
    let choice = pangine.reference_percept("captured-history-choice");
    let question = must_ref(pangine, "(['captured-history-event']->[signal]->[mark])(['captured-history-event']->[answer]->['captured-history-choice'])");
    pangine
        .complete_question(std::slice::from_ref(source), &question)
        .expect("valid captured-history question")
        .completions()
        .iter()
        .map(|completion| {
            let evidence = completion
                .evidence()
                .iter()
                .find(|evidence| evidence.binding(&event).is_some() && evidence.binding(&choice).is_some())
                .expect("captured choice-history evidence");
            let coefficient_ancestors = evidence.coefficient_ancestors().collect::<Vec<_>>();
            let coefficient = match coefficient_ancestors.as_slice() {
                [] => None,
                [ancestor] => {
                    let entries = pangine.get_relevance_map(ancestor);
                    let [(coefficient, _)] = entries.as_slice() else {
                        panic!("coefficient ancestor should contain one weighted operand");
                    };
                    Some(*coefficient)
                }
                _ => panic!("one captured choice row should cross at most one coefficient boundary"),
            };
            (
                pangine.format_concept(completion.binding(&event).expect("captured history event"), false),
                ChoiceHistory {
                    choice: pangine.format_concept(completion.binding(&choice).expect("captured history choice"), false),
                    source_percept: evidence.source_percept().expect("captured history source Percept").clone(),
                    source_relevance: evidence.source_relevance(),
                    coefficient,
                },
            )
        })
        .collect()
}

fn equal_support_histories(source: &ConceptId) -> BTreeMap<String, ChoiceHistory> {
    BTreeMap::from([
        ("[event-distinct-one]".to_owned(), choice_history("[B]", source, 1, None)),
        ("[event-distinct-three]".to_owned(), choice_history("[B]", source, 1, None)),
        ("[event-distinct-two]".to_owned(), choice_history("[B]", source, 1, None)),
        ("[event-repeat]".to_owned(), choice_history("[A]", source, 3, None)),
        ("[event-weighted]".to_owned(), choice_history("[C]", source, 3, Some(3))),
    ])
}

fn choice_history(choice: &str, source: &ConceptId, source_relevance: i64, coefficient: Option<i64>) -> ChoiceHistory {
    ChoiceHistory {
        choice: choice.to_owned(),
        source_percept: source.clone(),
        source_relevance: Relevance::new(source_relevance),
        coefficient: coefficient.map(Relevance::new),
    }
}

fn history_completions(pangine: &mut Pangine, archive: &ConceptId, stance: &ConceptId) -> BTreeMap<String, HistoryRecord> {
    let event = pangine.reference_percept("copy-history-event");
    let choice = pangine.reference_percept("copy-history-choice");
    let role = pangine.reference_percept("copy-history-role");
    let question = must_ref(
        pangine,
        "(['copy-history-event']->['copy-history-relation']->['copy-history-choice'])
         (['copy-history-relation']->[role]->['copy-history-role'])",
    );
    pangine
        .complete_question(&[archive.clone(), stance.clone()], &question)
        .expect("valid source-state copy history question")
        .completions()
        .iter()
        .map(|completion| {
            let source_evidence = completion
                .evidence()
                .iter()
                .find(|evidence| evidence.binding(&event).is_some() && evidence.binding(&choice).is_some())
                .expect("copy history source evidence");
            let coefficient_ancestors = source_evidence.coefficient_ancestors().collect::<Vec<_>>();
            let coefficient = match coefficient_ancestors.as_slice() {
                [] => None,
                [ancestor] => {
                    let entries = pangine.get_relevance_map(ancestor);
                    let [(coefficient, _)] = entries.as_slice() else {
                        panic!("coefficient ancestor should contain one weighted operand");
                    };
                    Some(*coefficient)
                }
                _ => panic!("one source row should cross at most one coefficient boundary"),
            };
            (
                pangine.format_concept(completion.binding(&event).expect("copy history event"), false),
                HistoryRecord {
                    choice: pangine.format_concept(completion.binding(&choice).expect("copy history choice"), false),
                    role: pangine.format_concept(completion.binding(&role).expect("copy history role"), false),
                    source_percept: source_evidence.source_percept().expect("copy history source Percept").clone(),
                    source_relevance: source_evidence.source_relevance(),
                    coefficient,
                },
            )
        })
        .collect()
}

fn expected_histories(source: &ConceptId) -> BTreeMap<String, HistoryRecord> {
    BTreeMap::from([
        ("[repeat-a]".to_owned(), history("[A]", "[positive]", source, 4, None)),
        ("[distinct-b-one]".to_owned(), history("[B]", "[positive]", source, 1, None)),
        ("[distinct-b-two]".to_owned(), history("[B]", "[positive]", source, 1, None)),
        ("[weighted-c]".to_owned(), history("[C]", "[positive]", source, 2, Some(3))),
        ("[counter-a]".to_owned(), history("[A]", "[negative]", source, 1, None)),
        ("[counter-b]".to_owned(), history("[B]", "[negative]", source, 1, None)),
        ("[counter-c]".to_owned(), history("[C]", "[negative]", source, 1, None)),
    ])
}

fn history(choice: &str, role: &str, source: &ConceptId, source_relevance: i64, coefficient: Option<i64>) -> HistoryRecord {
    HistoryRecord {
        choice: choice.to_owned(),
        role: role.to_owned(),
        source_percept: source.clone(),
        source_relevance: Relevance::new(source_relevance),
        coefficient: coefficient.map(Relevance::new),
    }
}

fn decision_state(pangine: &mut Pangine, name: &str) -> DecisionState {
    let percept = pangine.reference_percept(name);
    let candidates = pangine
        .get_value(&percept)
        .into_iter()
        .flat_map(|value| pangine.get_relevance_map(&value))
        .map(|(relevance, candidate)| (pangine.format_concept(&candidate, false), relevance))
        .collect();
    let selected =
        pangine.reference_concept(&format!("^['{name}']")).expect("valid source-state copy choice").map(|candidate| pangine.format_concept(&candidate, false));
    DecisionState { candidates, selected }
}

fn state(entries: &[(&str, i64)], selected: Option<&str>) -> DecisionState {
    DecisionState {
        candidates: entries.iter().map(|(candidate, relevance)| ((*candidate).to_owned(), Relevance::new(*relevance))).collect(),
        selected: selected.map(str::to_owned),
    }
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        let input = format!("['{percept}'] ~= {concept}");
        pangine
            .reference_concept(&input)
            .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
            .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
