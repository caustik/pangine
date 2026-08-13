//! Warning checks for decision knowledge represented in Pangine state.
//!
//! The host-side choice adapters are comparison cases, not a settled
//! architecture. These fixtures explore storing source-context eligibility as
//! ordinary relationships and letting Pangine perform the join, support
//! projection, and current deterministic choice.

use pangine::{Completion, ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
struct DecisionResult {
    candidates: BTreeMap<String, Relevance>,
    selected: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ExperienceHistory {
    candidate: String,
    source_relevance: Relevance,
    coefficient_bearing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionOrderEpisode {
    episode: String,
    context: String,
    order: String,
    consequence: String,
    relevance: Relevance,
}

#[test]
#[ignore = "warning: represented eligibility is promising, but additive support and deterministic choice remain provisional"]
fn represented_source_context_policy_filters_and_selects_inside_pangine() {
    let rows = [
        ("source-one", "north", "A", 5),
        ("source-one", "south", "A", 1),
        ("source-one", "south", "B", 4),
        ("source-two", "north", "A", 1),
        ("source-two", "north", "B", 4),
        ("source-two", "south", "A", 5),
    ];

    let first = decide(&rows, &[("source-one", "north"), ("source-two", "south")]);
    assert_eq!(first, DecisionResult { candidates: candidate_map(&[("[A]", 10)]), selected: Some("[A]".to_owned()) });

    let second = decide(&rows, &[("source-one", "south"), ("source-two", "north")]);
    assert_eq!(second, DecisionResult { candidates: candidate_map(&[("[A]", 2), ("[B]", 8)]), selected: Some("[B]".to_owned()) });
}

#[test]
#[ignore = "warning: absence of a represented eligible row is only one possible reason to abstain"]
fn represented_policy_can_leave_pangine_without_a_decision_candidate() {
    let result = decide(&[("source-one", "south", "A", 10)], &[("source-one", "north")]);
    assert_eq!(result, DecisionResult { candidates: BTreeMap::new(), selected: None });
}

#[test]
#[ignore = "warning: additive accumulation of complete experiences is promising but remains provisional"]
fn repeated_and_distinct_experiences_shape_pangines_own_decision() {
    let mut pangine = Pangine::new();
    sensor_experience(&mut pangine, "sensor-one", "hand", "withdraw", 2);
    sensor_experience(&mut pangine, "sensor-two", "hand", "continue", 1);
    sensor_experience(&mut pangine, "sensor-noise", "foot", "continue", 20);

    ask_sensor_decision(&mut pangine);
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[continue]", 1), ("[withdraw]", 2)]), selected: Some("[withdraw]".to_owned()) }
    );

    sensor_experience(&mut pangine, "sensor-three", "hand", "continue", 1);
    sensor_experience(&mut pangine, "sensor-four", "hand", "continue", 1);

    ask_sensor_decision(&mut pangine);
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[continue]", 3), ("[withdraw]", 2)]), selected: Some("[continue]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: accumulated events and replaceable current state are two application-provided representations, not universal sensor semantics"]
fn accumulated_events_and_replaceable_current_state_shape_different_answers() {
    let mut pangine = Pangine::new();
    let repeated_a = opaque_firing("rune-one", "A");
    let distinct_b = [opaque_firing("rune-two", "B"), opaque_firing("rune-three", "B"), opaque_firing("rune-four", "B")];

    experience(&mut pangine, "event-memory", &repeated_a, 3);
    for row in &distinct_b {
        experience(&mut pangine, "event-memory", row, 1);
    }

    replace_state(&mut pangine, "current-one", &repeated_a);
    let current_one = pangine.reference_percept("current-one");
    let current_one_state = pangine.get_relevance_map(&current_one);
    replace_state(&mut pangine, "current-one", &repeated_a);
    replace_state(&mut pangine, "current-one", &repeated_a);
    assert_eq!(pangine.get_relevance_map(&current_one), current_one_state, "replacing one sensor's state does not accumulate its prior updates");
    for (percept, row) in ["current-two", "current-three", "current-four"].into_iter().zip(&distinct_b) {
        replace_state(&mut pangine, percept, row);
    }

    let question_text = "(['retained-event']->[amber]->[quill])(['retained-event']->[violet]->['retained-choice'])";
    let question = must_ref(&mut pangine, question_text);
    let retained_event = pangine.reference_percept("retained-event");
    let retained_choice = pangine.reference_percept("retained-choice");
    let event_memory = pangine.reference_percept("event-memory");
    let current_sources =
        [current_one.clone(), pangine.reference_percept("current-two"), pangine.reference_percept("current-three"), pangine.reference_percept("current-four")];
    assert_eq!(
        firing_histories(&mut pangine, std::slice::from_ref(&event_memory), &question, &retained_event, &retained_choice),
        BTreeMap::from([
            ("[rune-four]".to_owned(), firing_history("[B]", "['event-memory']", 1)),
            ("[rune-one]".to_owned(), firing_history("[A]", "['event-memory']", 3)),
            ("[rune-three]".to_owned(), firing_history("[B]", "['event-memory']", 1)),
            ("[rune-two]".to_owned(), firing_history("[B]", "['event-memory']", 1)),
        ])
    );
    let current_histories = BTreeMap::from([
        ("[rune-four]".to_owned(), firing_history("[B]", "['current-four']", 1)),
        ("[rune-one]".to_owned(), firing_history("[A]", "['current-one']", 1)),
        ("[rune-three]".to_owned(), firing_history("[B]", "['current-three']", 1)),
        ("[rune-two]".to_owned(), firing_history("[B]", "['current-two']", 1)),
    ]);
    assert_eq!(firing_histories(&mut pangine, &current_sources, &question, &retained_event, &retained_choice), current_histories);

    pangine.reference_concept(&format!("['event-memory'] @ {question_text}")).expect("valid accumulated-event decision");
    assert_eq!(
        read_named_decision(&mut pangine, "retained-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 3)]), selected: Some("[A]".to_owned()) },
        "the accumulated representation reaches a tie and current ^ uses its canonical fallback"
    );
    let current_selector = "['current-one']['current-two']['current-three']['current-four']";
    pangine.reference_concept(&format!("{current_selector} @ {question_text}")).expect("valid replaceable-current-state decision");
    assert_eq!(
        read_named_decision(&mut pangine, "retained-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "event-memory", &repeated_a, 1);
    replace_state(&mut pangine, "current-one", &repeated_a);
    assert_eq!(pangine.get_relevance_map(&current_one), current_one_state);
    assert_eq!(
        firing_histories(&mut pangine, std::slice::from_ref(&event_memory), &question, &retained_event, &retained_choice).get("[rune-one]").cloned(),
        Some(firing_history("[A]", "['event-memory']", 4))
    );
    assert_eq!(firing_histories(&mut pangine, &current_sources, &question, &retained_event, &retained_choice), current_histories);

    pangine.reference_concept(&format!("['event-memory'] @ {question_text}")).expect("valid accumulated-event decision after another firing");
    assert_eq!(
        read_named_decision(&mut pangine, "retained-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );
    pangine.reference_concept(&format!("{current_selector} @ {question_text}")).expect("valid current-state decision after another update");
    assert_eq!(
        read_named_decision(&mut pangine, "retained-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: a detached basis marker can cross-join retained views that share record identities"]
fn detached_represented_basis_does_not_select_between_both_retained_views() {
    let mut pangine = Pangine::new();
    let accumulated_a = opaque_detached_basis_firing("rune-one", "A", "cedar");
    let accumulated_b = [
        opaque_detached_basis_firing("rune-two", "B", "cedar"),
        opaque_detached_basis_firing("rune-three", "B", "cedar"),
        opaque_detached_basis_firing("rune-four", "B", "cedar"),
    ];
    experience(&mut pangine, "event-memory", &accumulated_a, 4);
    for row in &accumulated_b {
        experience(&mut pangine, "event-memory", row, 1);
    }

    let current_a = opaque_detached_basis_firing("rune-one", "A", "quartz");
    let current_b = [
        opaque_detached_basis_firing("rune-two", "B", "quartz"),
        opaque_detached_basis_firing("rune-three", "B", "quartz"),
        opaque_detached_basis_firing("rune-four", "B", "quartz"),
    ];
    replace_state(&mut pangine, "current-one", &current_a);
    for (percept, row) in ["current-two", "current-three", "current-four"].into_iter().zip(&current_b) {
        replace_state(&mut pangine, percept, row);
    }

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['view']";
    let question_text = "(['joined-event']->[amber]->[quill])(['joined-event']->[violet]->['joined-choice'])(['joined-event']->[cobalt]->['joined-basis'])([lantern]->[cobalt]->['joined-basis'])";
    let question = must_ref(&mut pangine, question_text);
    let sources = [
        pangine.reference_percept("event-memory"),
        pangine.reference_percept("current-one"),
        pangine.reference_percept("current-two"),
        pangine.reference_percept("current-three"),
        pangine.reference_percept("current-four"),
        pangine.reference_percept("view"),
    ];
    let joined_choice = pangine.reference_percept("joined-choice");

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]");
    let accumulated = pangine.complete_question(&sources, &question).expect("valid represented-basis question");
    assert_eq!(
        accumulated
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&joined_choice).expect("bound accumulated choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid accumulated-basis decision");
    assert_eq!(
        read_named_decision(&mut pangine, "joined-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) },
        "the selected marker can join answer paths from both views because they share event identities"
    );

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[quartz]");
    let current = pangine.complete_question(&sources, &question).expect("valid represented-basis question after changing view");
    assert_eq!(
        current
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&joined_choice).expect("bound current choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid current-basis decision");
    assert_eq!(
        read_named_decision(&mut pangine, "joined-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) },
        "changing only the detached marker does not change which answer paths contribute"
    );
}

#[test]
#[ignore = "warning: putting a represented basis inside each evidence path is one possible retention shape, not universal decision semantics"]
fn represented_basis_inside_each_evidence_path_switches_one_fixed_question() {
    let mut pangine = Pangine::new();
    populate_embedded_retained_views(&mut pangine);

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['view']";
    let question_text = "(['joined-event']->[amber]->['joined-basis']->[quill])(['joined-event']->[violet]->['joined-basis']->['joined-choice'])([lantern]->[cobalt]->['joined-basis'])";
    let question = must_ref(&mut pangine, question_text);
    let sources = [
        pangine.reference_percept("event-memory"),
        pangine.reference_percept("current-one"),
        pangine.reference_percept("current-two"),
        pangine.reference_percept("current-three"),
        pangine.reference_percept("current-four"),
        pangine.reference_percept("view"),
    ];
    let joined_choice = pangine.reference_percept("joined-choice");
    let joined_basis = pangine.reference_percept("joined-basis");
    let cedar = must_ref(&mut pangine, "[cedar]");
    let quartz = must_ref(&mut pangine, "[quartz]");

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]");
    let accumulated = pangine.complete_question(&sources, &question).expect("valid embedded-basis question");
    assert!(accumulated.completions().iter().all(|completion| completion.binding(&joined_basis) == Some(&cedar)));
    assert_eq!(
        accumulated
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&joined_choice).expect("bound accumulated choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid embedded accumulated-basis decision");
    assert_eq!(
        read_named_decision(&mut pangine, "joined-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[quartz]");
    let current = pangine.complete_question(&sources, &question).expect("valid embedded-basis question after changing view");
    assert!(current.completions().iter().all(|completion| completion.binding(&joined_basis) == Some(&quartz)));
    assert_eq!(
        current
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&joined_choice).expect("bound current choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid embedded current-basis decision");
    assert_eq!(
        read_named_decision(&mut pangine, "joined-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    clear_percept(&mut pangine, "view");
    experience(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]", 1);
    experience(&mut pangine, "view", "[lantern]->[cobalt]->[quartz]", 1);
    let both = pangine.complete_question(&sources, &question).expect("valid embedded-basis question with both views present");
    assert_eq!(
        both.completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&joined_basis).expect("bound basis with both views present"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[cedar]".to_owned(), "[quartz]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid decision with both represented bases");
    assert_eq!(
        read_named_decision(&mut pangine, "joined-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]", 19);
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid decision after repeating one represented basis");
    assert_eq!(
        read_named_decision(&mut pangine, "joined-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) },
        "basis state admits evidence paths; its own repetition is not answer evidence in this question"
    );
}

#[test]
#[ignore = "warning: deriving a retained basis from represented condition links is promising but remains one explicit question shape"]
fn represented_condition_derives_the_retained_basis_inside_the_fixed_question() {
    let mut pangine = Pangine::new();
    populate_embedded_retained_views(&mut pangine);
    experience(&mut pangine, "links", "[opal]->[cobalt]->[cedar]", 1);
    experience(&mut pangine, "links", "[basalt]->[cobalt]->[quartz]", 1);

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['links']['condition']";
    let question_text = "(['joined-event']->[amber]->['joined-basis']->[quill])(['joined-event']->[violet]->['joined-basis']->['condition-choice'])(['joined-condition']->[cobalt]->['joined-basis'])([lantern]->[topaz]->['joined-condition'])";

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[opal]");
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid opal-condition decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[basalt]");
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid basalt-condition decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    clear_percept(&mut pangine, "condition");
    experience(&mut pangine, "condition", "[lantern]->[topaz]->[opal]", 1);
    experience(&mut pangine, "condition", "[lantern]->[topaz]->[basalt]", 1);
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid two-condition decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: an enclosing record can correlate a represented view with nested evidence only when the whole shape is asked together"]
fn one_enclosing_record_keeps_view_and_nested_evidence_correlated() {
    let mut pangine = Pangine::new();
    populate_enclosed_retained_views(&mut pangine);

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['view']";
    let question_text = "(['record-basis']->[ochre]->((['record-event']->[amber]->[quill])(['record-event']->[violet]->['record-choice'])))([lantern]->[cobalt]->['record-basis'])";
    let question = must_ref(&mut pangine, question_text);
    let sources = [
        pangine.reference_percept("event-memory"),
        pangine.reference_percept("current-one"),
        pangine.reference_percept("current-two"),
        pangine.reference_percept("current-three"),
        pangine.reference_percept("current-four"),
        pangine.reference_percept("view"),
    ];
    let record_basis = pangine.reference_percept("record-basis");
    let record_choice = pangine.reference_percept("record-choice");
    let cedar = must_ref(&mut pangine, "[cedar]");
    let quartz = must_ref(&mut pangine, "[quartz]");

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]");
    let accumulated = pangine.complete_question(&sources, &question).expect("valid enclosing-record accumulated question");
    assert!(accumulated.completions().iter().all(|completion| completion.binding(&record_basis) == Some(&cedar)));
    assert_eq!(
        accumulated
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&record_choice).expect("bound enclosing-record accumulated choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid enclosing-record accumulated decision");
    assert_eq!(
        read_named_decision(&mut pangine, "record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[quartz]");
    let current = pangine.complete_question(&sources, &question).expect("valid enclosing-record current question");
    assert!(current.completions().iter().all(|completion| completion.binding(&record_basis) == Some(&quartz)));
    assert_eq!(
        current
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&record_choice).expect("bound enclosing-record current choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid enclosing-record current decision");
    assert_eq!(
        read_named_decision(&mut pangine, "record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    clear_percept(&mut pangine, "view");
    experience(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]", 1);
    experience(&mut pangine, "view", "[lantern]->[cobalt]->[quartz]", 1);
    let both = pangine.complete_question(&sources, &question).expect("valid enclosing-record question with both views");
    assert_eq!(
        both.completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&record_basis).expect("bound enclosing-record basis"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[cedar]".to_owned(), "[quartz]".to_owned()])
    );
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid enclosing-record decision with both views");
    assert_eq!(
        read_named_decision(&mut pangine, "record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) }
    );

    let split_question_text = "(['split-basis']->[ochre]->['split-payload'])(['split-event']->[amber]->[quill])(['split-event']->[violet]->['split-choice'])([lantern]->[cobalt]->['split-basis'])";
    let split_question = must_ref(&mut pangine, split_question_text);
    let split_choice = pangine.reference_percept("split-choice");

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[cedar]");
    let split_accumulated = pangine.complete_question(&sources, &split_question).expect("valid split enclosing-record accumulated question");
    assert_eq!(choice_source_names(&mut pangine, &split_accumulated, &split_choice), retained_view_source_names());
    pangine.reference_concept(&format!("{selector} @ {split_question_text}")).expect("valid split enclosing-record accumulated decision");
    assert_eq!(
        read_named_decision(&mut pangine, "split-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) },
        "separate outer and descendant clauses admit answer evidence from both retained views"
    );

    replace_state(&mut pangine, "view", "[lantern]->[cobalt]->[quartz]");
    let split_current = pangine.complete_question(&sources, &split_question).expect("valid split enclosing-record current question");
    assert_eq!(choice_source_names(&mut pangine, &split_current, &split_choice), retained_view_source_names());
    pangine.reference_concept(&format!("{selector} @ {split_question_text}")).expect("valid split enclosing-record current decision");
    assert_eq!(
        read_named_decision(&mut pangine, "split-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) },
        "changing the selected outer record does not constrain separately matched descendants"
    );
}

#[test]
#[ignore = "warning: condition-derived selection of enclosing evidence records remains one explicit Pangine question, not a universal retention policy"]
fn represented_condition_selects_whole_enclosing_evidence_records() {
    let mut pangine = Pangine::new();
    populate_enclosed_retained_views(&mut pangine);
    experience(&mut pangine, "links", "[opal]->[cobalt]->[cedar]", 1);
    experience(&mut pangine, "links", "[basalt]->[cobalt]->[quartz]", 1);

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['links']['condition']";
    let question_text = "(['record-basis']->[ochre]->((['record-event']->[amber]->[quill])(['record-event']->[violet]->['condition-record-choice'])))(['record-condition']->[cobalt]->['record-basis'])([lantern]->[topaz]->['record-condition'])";

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[opal]");
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid condition-selected accumulated record decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[basalt]");
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid condition-selected current record decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    clear_percept(&mut pangine, "condition");
    experience(&mut pangine, "condition", "[lantern]->[topaz]->[opal]", 1);
    experience(&mut pangine, "condition", "[lantern]->[topaz]->[basalt]", 1);
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid two-condition enclosing-record decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "condition", "[lantern]->[topaz]->[opal]", 19);
    pangine.reference_concept(&format!("{selector} @ {question_text}")).expect("valid repeated-condition enclosing-record decision");
    assert_eq!(
        read_named_decision(&mut pangine, "condition-record-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) },
        "the current condition admits records but does not bind the answer in this question"
    );
}

#[test]
#[ignore = "warning: answer-linked condition experience adds support only because this explicit question binds it to the same candidate"]
fn condition_repetition_changes_the_answer_only_when_linked_to_the_candidate() {
    let mut pangine = Pangine::new();
    populate_enclosed_retained_views(&mut pangine);
    experience(&mut pangine, "links", "[opal]->[cobalt]->[cedar]", 1);
    experience(&mut pangine, "links", "[basalt]->[cobalt]->[quartz]", 1);
    experience(&mut pangine, "condition-evidence", "[opal]->[indigo]->[A]", 1);
    experience(&mut pangine, "condition-evidence", "[opal]->[indigo]->[B]", 3);
    experience(&mut pangine, "condition-evidence", "[basalt]->[indigo]->[A]", 4);
    experience(&mut pangine, "condition-evidence", "[basalt]->[indigo]->[B]", 1);
    experience(&mut pangine, "condition-evidence", "[obsidian]->[indigo]->[B]", 20);

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['links']['condition']['condition-evidence']";
    let filter_question = "(['filter-basis']->[ochre]->((['filter-event']->[amber]->[quill])(['filter-event']->[violet]->['filter-choice'])))(['filter-condition']->[cobalt]->['filter-basis'])([lantern]->[topaz]->['filter-condition'])";
    let linked_question = "(['linked-basis']->[ochre]->((['linked-event']->[amber]->[quill])(['linked-event']->[violet]->['linked-choice'])))(['linked-condition']->[cobalt]->['linked-basis'])([lantern]->[topaz]->['linked-condition'])(['linked-condition']->[indigo]->['linked-choice'])";

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[opal]");
    pangine.reference_concept(&format!("{selector} @ {filter_question}")).expect("valid filter-only opal decision");
    assert_eq!(
        read_named_decision(&mut pangine, "filter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid answer-linked opal decision");
    assert_eq!(
        read_named_decision(&mut pangine, "linked-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) }
    );

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[basalt]");
    pangine.reference_concept(&format!("{selector} @ {filter_question}")).expect("valid filter-only basalt decision");
    assert_eq!(
        read_named_decision(&mut pangine, "filter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid answer-linked basalt decision");
    assert_eq!(
        read_named_decision(&mut pangine, "linked-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 4)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[opal]");
    experience(&mut pangine, "condition-evidence", "[opal]->[indigo]->[A]", 2);
    pangine.reference_concept(&format!("{selector} @ {filter_question}")).expect("valid filter-only decision after more answer-linked experience");
    assert_eq!(
        read_named_decision(&mut pangine, "filter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) },
        "answer-linked experience stays irrelevant to the filter-only question"
    );
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid linked decision after more answer-linked experience");
    assert_eq!(
        read_named_decision(&mut pangine, "linked-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 7), ("[B]", 6)]), selected: Some("[A]".to_owned()) },
        "two more opal-to-A experiences change Pangine's linked answer"
    );

    experience(&mut pangine, "condition", "[lantern]->[topaz]->[opal]", 19);
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid linked decision after repeating filter-only condition");
    assert_eq!(
        read_named_decision(&mut pangine, "linked-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 7), ("[B]", 6)]), selected: Some("[A]".to_owned()) },
        "repeating the current-condition fact does not add support when it still does not bind the candidate"
    );
}

#[test]
#[ignore = "warning: condition-event-outcome observations are one explicit evidence shape, not universal causal or decision semantics"]
fn complete_observations_derive_answer_support_without_precollapsed_condition_votes() {
    let mut pangine = Pangine::new();
    populate_enclosed_retained_views(&mut pangine);
    experience(&mut pangine, "links", "[opal]->[cobalt]->[cedar]", 1);
    experience(&mut pangine, "links", "[basalt]->[cobalt]->[quartz]", 1);

    let observation = |condition: &str, event: &str, candidate: &str| format!("([{condition}]->[indigo]->[{event}])([{event}]->[saffron]->[{candidate}])");
    let repeated_opal_a = observation("opal", "opal-a-repeat", "A");
    experience(&mut pangine, "observations", &repeated_opal_a, 1);
    for event in ["opal-b-one", "opal-b-two", "opal-b-three"] {
        experience(&mut pangine, "observations", &observation("opal", event, "B"), 1);
    }
    for event in ["basalt-a-one", "basalt-a-two", "basalt-a-three", "basalt-a-four"] {
        experience(&mut pangine, "observations", &observation("basalt", event, "A"), 1);
    }
    experience(&mut pangine, "observations", &observation("basalt", "basalt-b-one", "B"), 1);
    experience(&mut pangine, "observations", &observation("obsidian", "obsidian-b-repeat", "B"), 20);

    let selector = "['event-memory']['current-one']['current-two']['current-three']['current-four']['links']['condition']['observations']";
    let filter_question = "(['observation-filter-basis']->[ochre]->((['observation-filter-event']->[amber]->[quill])(['observation-filter-event']->[violet]->['observation-filter-choice'])))(['observation-filter-condition']->[cobalt]->['observation-filter-basis'])([lantern]->[topaz]->['observation-filter-condition'])";
    let linked_question = "(['observation-basis']->[ochre]->((['record-event']->[amber]->[quill])(['record-event']->[violet]->['observation-choice'])))(['observation-condition']->[cobalt]->['observation-basis'])([lantern]->[topaz]->['observation-condition'])(['observation-condition']->[indigo]->['observation-event'])(['observation-event']->[saffron]->['observation-choice'])";

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[opal]");
    pangine.reference_concept(&format!("{selector} @ {filter_question}")).expect("valid filter-only opal observation decision");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-filter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid observation-linked opal decision");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 6)]), selected: Some("[B]".to_owned()) }
    );

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[basalt]");
    pangine.reference_concept(&format!("{selector} @ {filter_question}")).expect("valid filter-only basalt observation decision");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-filter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid observation-linked basalt decision");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 4)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "condition", "[lantern]->[topaz]->[opal]");
    experience(&mut pangine, "observations", &repeated_opal_a, 2);
    pangine.reference_concept(&format!("{selector} @ {filter_question}")).expect("valid filter-only decision after repeated observation");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-filter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) },
        "observations stay irrelevant when the question only uses condition to admit records"
    );
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid linked decision after repeated observation");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 7), ("[B]", 6)]), selected: Some("[A]".to_owned()) },
        "two more complete opal-A observations change Pangine's linked answer"
    );

    experience(&mut pangine, "condition", "[lantern]->[topaz]->[opal]", 19);
    pangine.reference_concept(&format!("{selector} @ {linked_question}")).expect("valid linked decision after repeated filter condition");
    assert_eq!(
        read_named_decision(&mut pangine, "observation-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 7), ("[B]", 6)]), selected: Some("[A]".to_owned()) },
        "repeating the current condition does not multiply the observation support"
    );

    let observation_source = pangine.reference_percept("observations");
    let observation_event = pangine.reference_percept("observation-event");
    let observation_choice = pangine.reference_percept("observation-choice");
    let sources = ["event-memory", "current-one", "current-two", "current-three", "current-four", "links", "condition", "observations"]
        .map(|name| pangine.reference_percept(name));
    let question = must_ref(&mut pangine, linked_question);
    let result = pangine.complete_question(&sources, &question).expect("valid detailed observation-history question");
    let histories = result
        .completions()
        .iter()
        .map(|completion| {
            let evidence = completion
                .evidence()
                .iter()
                .find(|evidence| evidence.source_percept() == Some(&observation_source) && evidence.binding(&observation_choice).is_some())
                .expect("outcome evidence from the complete observation");
            (
                pangine.format_concept(completion.binding(&observation_event).expect("bound observation event"), false),
                pangine.format_concept(completion.binding(&observation_choice).expect("bound observation choice"), false),
                evidence.source_relevance(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        histories,
        BTreeSet::from([
            ("[opal-a-repeat]".to_owned(), "[A]".to_owned(), Relevance::new(3)),
            ("[opal-b-one]".to_owned(), "[B]".to_owned(), Relevance::DEFAULT),
            ("[opal-b-three]".to_owned(), "[B]".to_owned(), Relevance::DEFAULT),
            ("[opal-b-two]".to_owned(), "[B]".to_owned(), Relevance::DEFAULT),
        ]),
        "equal observation subtotals retain one repeated event and three distinct event identities before answer projection"
    );
}

#[test]
#[ignore = "warning: current report-additive choice does not decide whether correlated reports should count once or many times"]
fn reporting_source_and_event_identity_remain_visible_in_the_same_decision() {
    let mut pangine = Pangine::new();
    populate_source_event_reports(&mut pangine);

    let question_text = "(['report']->[amber]->['report-source'])(['report']->[topaz]->['report-condition'])([lantern]->[cobalt]->['report-condition'])(['report']->[indigo]->['reported-event'])(['reported-event']->[saffron]->['reported-choice'])";
    let reports = pangine.reference_percept("reports");
    let condition = pangine.reference_percept("condition");
    let report_id = pangine.reference_percept("report");
    let report_source = pangine.reference_percept("report-source");
    let reported_event = pangine.reference_percept("reported-event");
    let reported_choice = pangine.reference_percept("reported-choice");
    let question = must_ref(&mut pangine, question_text);
    let result = pangine.complete_question(&[reports.clone(), condition], &question).expect("valid source-and-event report question");
    let histories = result
        .completions()
        .iter()
        .map(|completion| {
            let outcome_evidence = completion
                .evidence()
                .iter()
                .find(|evidence| evidence.source_percept() == Some(&reports) && evidence.binding(&reported_choice).is_some())
                .expect("outcome evidence from one complete report");
            (
                pangine.format_concept(completion.binding(&report_id).expect("bound report ID"), false),
                pangine.format_concept(completion.binding(&report_source).expect("bound report source"), false),
                pangine.format_concept(completion.binding(&reported_event).expect("bound reported event"), false),
                pangine.format_concept(completion.binding(&reported_choice).expect("bound reported choice"), false),
                outcome_evidence.source_relevance(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        histories,
        BTreeSet::from([
            ("[report-a-one]".to_owned(), "[source-one]".to_owned(), "[shared-a-event]".to_owned(), "[A]".to_owned(), Relevance::DEFAULT,),
            ("[report-a-three]".to_owned(), "[source-three]".to_owned(), "[shared-a-event]".to_owned(), "[A]".to_owned(), Relevance::DEFAULT,),
            ("[report-a-two]".to_owned(), "[source-two]".to_owned(), "[shared-a-event]".to_owned(), "[A]".to_owned(), Relevance::DEFAULT,),
            ("[report-b-one]".to_owned(), "[source-four]".to_owned(), "[distinct-b-event-one]".to_owned(), "[B]".to_owned(), Relevance::DEFAULT,),
            ("[report-b-three]".to_owned(), "[source-six]".to_owned(), "[distinct-b-event-three]".to_owned(), "[B]".to_owned(), Relevance::DEFAULT,),
            ("[report-b-two]".to_owned(), "[source-five]".to_owned(), "[distinct-b-event-two]".to_owned(), "[B]".to_owned(), Relevance::DEFAULT,),
        ]),
        "the equal answer totals retain three sources sharing one event and three sources reporting distinct events"
    );

    pangine.reference_concept(&format!("['reports']['condition'] @ {question_text}")).expect("valid materialized source-and-event report question");
    assert_eq!(
        read_named_decision(&mut pangine, "reported-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 3)]), selected: Some("[A]".to_owned()) },
        "the current placeholder sees six reports and uses its canonical fallback for the equal answer totals"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "reported-event"),
        candidate_map(&[("[distinct-b-event-one]", 1), ("[distinct-b-event-three]", 1), ("[distinct-b-event-two]", 1), ("[shared-a-event]", 3),]),
        "Pangine keeps the shared event identity while retaining all three matching reports"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "report-source"),
        candidate_map(&[("[source-five]", 1), ("[source-four]", 1), ("[source-one]", 1), ("[source-six]", 1), ("[source-three]", 1), ("[source-two]", 1),]),
        "Pangine also keeps every reporting source separate"
    );

    experience(&mut pangine, "reports", &source_event_report("report-a-four", "source-seven", "opal", "shared-a-event", "A"), 1);
    pangine.reference_concept(&format!("['reports']['condition'] @ {question_text}")).expect("valid decision after one more source reports the shared event");
    assert_eq!(
        read_named_decision(&mut pangine, "reported-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) },
        "a fourth source reporting the shared event changes the current report-additive result"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "reported-event").get("[shared-a-event]").copied(),
        Some(Relevance::new(4)),
        "the shared event stays one identity with four retained reports"
    );

    experience(&mut pangine, "condition", "[lantern]->[cobalt]->[opal]", 19);
    pangine.reference_concept(&format!("['reports']['condition'] @ {question_text}")).expect("valid decision after repeating the filter condition");
    assert_eq!(
        read_named_decision(&mut pangine, "reported-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) },
        "repeating the condition still filters reports without adding answer support"
    );
}

#[test]
#[ignore = "warning: report-first and event-first Pangine programs can disagree without either becoming a universal decision policy"]
fn question_order_can_choose_total_reports_or_most_reported_event_inside_pangine() {
    let mut pangine = Pangine::new();
    populate_source_event_reports(&mut pangine);
    experience(&mut pangine, "reports", &source_event_report("report-b-four", "source-seven", "opal", "distinct-b-event-four", "B"), 1);

    let report_question = "(['order-report']->[amber]->['order-source'])(['order-report']->[topaz]->['order-condition'])([lantern]->[cobalt]->['order-condition'])(['order-report']->[indigo]->['order-event'])(['order-event']->[saffron]->['outcome-first-choice'])";
    must_ref(&mut pangine, &format!("['reports']['condition'] @ {report_question}"));
    assert_eq!(
        read_named_decision(&mut pangine, "outcome-first-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) },
        "settling the outcome after all eligible reports lets four B reports outweigh three A reports"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "order-event"),
        candidate_map(&[
            ("[distinct-b-event-four]", 1),
            ("[distinct-b-event-one]", 1),
            ("[distinct-b-event-three]", 1),
            ("[distinct-b-event-two]", 1),
            ("[shared-a-event]", 3),
        ]),
        "the same report question retains one A event reported three times and four B events reported once each"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "order-source"),
        candidate_map(&[
            ("[source-five]", 1),
            ("[source-four]", 1),
            ("[source-one]", 1),
            ("[source-seven]", 1),
            ("[source-six]", 1),
            ("[source-three]", 1),
            ("[source-two]", 1),
        ]),
        "all seven reporting sources remain visible before either Pangine choice"
    );

    must_ref(&mut pangine, "['selected-order-event'] = ^['order-event']");
    let selected_event = must_ref(&mut pangine, "$['selected-order-event']");
    assert_eq!(pangine.format_concept(&selected_event, false), "[shared-a-event]");
    must_ref(&mut pangine, "['selected-event-link'] = [selected]->$['selected-order-event']");

    let event_first_question = "([selected]->['event-first-event'])(['event-first-report']->[topaz]->['event-first-condition'])([lantern]->[cobalt]->['event-first-condition'])(['event-first-report']->[indigo]->['event-first-event'])(['event-first-event']->[saffron]->['event-first-choice'])";
    must_ref(&mut pangine, &format!("['selected-event-link']['reports']['condition'] @ {event_first_question}"));
    assert_eq!(
        read_named_decision(&mut pangine, "event-first-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) },
        "settling the most-reported event first selects the shared A event, then Pangine derives A from its three reports"
    );

    experience(&mut pangine, "condition", "[lantern]->[cobalt]->[opal]", 19);
    must_ref(&mut pangine, &format!("['reports']['condition'] @ {report_question}"));
    must_ref(&mut pangine, "['selected-order-event'] = ^['order-event']");
    must_ref(&mut pangine, "['selected-event-link'] = [selected]->$['selected-order-event']");
    must_ref(&mut pangine, &format!("['selected-event-link']['reports']['condition'] @ {event_first_question}"));
    assert_eq!(
        read_named_decision(&mut pangine, "outcome-first-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "event-first-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) },
        "repeating a filtering condition changes neither Pangine program's answer"
    );
}

#[test]
#[ignore = "warning: represented experience can select between preserved question-order conclusions, but the supplied guidance is not a universal reasoning policy"]
fn represented_experience_selects_between_preserved_question_order_conclusions() {
    let mut pangine = Pangine::new();
    populate_question_order_record(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "outcome-first-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "event-first-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );

    let eligible_reports = must_ref(&mut pangine, "$['eligible-order-reports']");
    let reporting_sources = must_ref(&mut pangine, "$['record-source']");
    let reported_events = must_ref(&mut pangine, "$['record-event']");
    let outcome_first_state = must_ref(&mut pangine, "$['outcome-first-state']");
    let outcome_first_conclusion = must_ref(&mut pangine, "$['outcome-first-conclusion']");
    let selected_event = must_ref(&mut pangine, "$['selected-record-event']");
    let event_first_state = must_ref(&mut pangine, "$['event-first-state']");
    let event_first_conclusion = must_ref(&mut pangine, "$['event-first-conclusion']");
    clear_question_order_live_state(&mut pangine);

    let question_order_record = pangine.reference_percept("question-order-record");
    let recorded_order = pangine.reference_percept("recorded-order");
    let recorded_conclusion = pangine.reference_percept("recorded-conclusion");
    let recorded_conclusion_question = must_ref(&mut pangine, "['recorded-order']->[conclusion]->['recorded-conclusion']");
    let preserved_conclusions = pangine
        .complete_question(std::slice::from_ref(&question_order_record), &recorded_conclusion_question)
        .expect("valid preserved question-order conclusions")
        .completions()
        .iter()
        .map(|completion| {
            (
                pangine.format_concept(completion.binding(&recorded_order).expect("recorded reasoning order"), false),
                pangine.format_concept(completion.binding(&recorded_conclusion).expect("recorded conclusion"), false),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        preserved_conclusions,
        BTreeSet::from([("[event-first]".to_owned(), "[A]".to_owned()), ("[outcome-first]".to_owned(), "[B]".to_owned())]),
        "both disagreeing conclusions remain ordinary relationships in one Pangine record"
    );

    for (path, output) in [
        ("[reasoning]->[reports]", "recorded-order-reports"),
        ("[reasoning]->[sources]", "recorded-order-sources"),
        ("[reasoning]->[events]", "recorded-order-events"),
        ("[outcome-first]->[candidates]", "recorded-outcome-first-state"),
        ("[outcome-first]->[conclusion]", "recorded-outcome-first-conclusion"),
        ("[event-first]->[selected-event]", "recorded-selected-event"),
        ("[event-first]->[candidates]", "recorded-event-first-state"),
        ("[event-first]->[conclusion]", "recorded-event-first-conclusion"),
    ] {
        must_ref(&mut pangine, &format!("['question-order-record'] @ {path}->['{output}']"));
    }
    assert_eq!(must_ref(&mut pangine, "$['recorded-order-reports']"), eligible_reports);
    assert_eq!(must_ref(&mut pangine, "$['recorded-order-sources']"), reporting_sources);
    assert_eq!(must_ref(&mut pangine, "$['recorded-order-events']"), reported_events);
    assert_eq!(must_ref(&mut pangine, "$['recorded-outcome-first-state']"), outcome_first_state);
    assert_eq!(must_ref(&mut pangine, "$['recorded-outcome-first-conclusion']"), outcome_first_conclusion);
    assert_eq!(must_ref(&mut pangine, "$['recorded-selected-event']"), selected_event);
    assert_eq!(must_ref(&mut pangine, "$['recorded-event-first-state']"), event_first_state);
    assert_eq!(must_ref(&mut pangine, "$['recorded-event-first-conclusion']"), event_first_conclusion);

    experience(&mut pangine, "order-guidance", "[cedar]->[guides]->[outcome-first]", 2);
    experience(&mut pangine, "order-guidance", "[cedar]->[guides]->[event-first]", 1);
    experience(&mut pangine, "order-guidance", "[quartz]->[guides]->[outcome-first]", 1);
    experience(&mut pangine, "order-guidance", "[quartz]->[guides]->[event-first]", 2);
    experience(&mut pangine, "order-guidance", "[obsidian]->[guides]->[event-first]", 20);

    replace_state(&mut pangine, "reasoning-context", "[request]->[context]->[cedar]");
    run_represented_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "relevant-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) },
        "the selected outcome-first state keeps its original alternatives and amounts"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    replace_state(&mut pangine, "reasoning-context", "[request]->[context]->[quartz]");
    run_represented_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "relevant-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) },
        "the selected event-first state keeps the support derived from the chosen event's reports"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    clear_percept(&mut pangine, "reasoning-context");
    experience(&mut pangine, "reasoning-context", "[request]->[context]->[cedar]", 20);
    run_represented_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "relevant-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) },
        "repeating the current context admits guidance without multiplying it"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "order-guidance", "[cedar]->[guides]->[event-first]", 2);
    run_represented_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "relevant-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) },
        "more matching guidance experience can change which preserved Pangine conclusion is currently relevant"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(must_ref(&mut pangine, "$['recorded-outcome-first-conclusion']"), outcome_first_conclusion);
    assert_eq!(must_ref(&mut pangine, "$['recorded-event-first-conclusion']"), event_first_conclusion);
}

#[test]
#[ignore = "warning: complete past episodes shape reasoning-order relevance only because this explicit question joins context and requested consequence"]
fn complete_past_episodes_derive_question_order_without_direct_guidance() {
    let mut pangine = Pangine::new();
    populate_question_order_record(&mut pangine);
    clear_question_order_live_state(&mut pangine);

    for (episode, context, order, consequence) in [
        ("episode-one", "cedar", "outcome-first", "opal"),
        ("episode-two", "cedar", "outcome-first", "opal"),
        ("episode-three", "cedar", "event-first", "opal"),
        ("episode-four", "quartz", "outcome-first", "opal"),
        ("episode-five", "quartz", "event-first", "opal"),
        ("episode-six", "quartz", "event-first", "opal"),
        ("episode-seven", "cedar", "outcome-first", "basalt"),
        ("episode-eight", "cedar", "event-first", "basalt"),
        ("episode-nine", "cedar", "event-first", "basalt"),
    ] {
        experience(&mut pangine, "order-episodes", &question_order_episode(episode, context, order, consequence), 1);
    }
    experience(&mut pangine, "order-episodes", &question_order_episode("noise-episode", "obsidian", "event-first", "opal"), 20);
    assert!(read_named_weights(&mut pangine, "order-guidance").is_empty(), "the episode fixture contains no direct context-to-order guidance state");

    replace_state(&mut pangine, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut pangine, "requested-consequence", "[request]->[saffron]->[opal]");
    run_episode_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) },
        "two complete cedar/opal episodes support outcome-first and one supports event-first"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "past-episode"),
        candidate_map(&[("[episode-one]", 1), ("[episode-three]", 1), ("[episode-two]", 1)]),
        "the matching episode identities remain visible instead of becoming a direct context-to-order summary"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    replace_state(&mut pangine, "episode-context", "[request]->[cobalt]->[quartz]");
    run_episode_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "past-episode"), candidate_map(&[("[episode-five]", 1), ("[episode-four]", 1), ("[episode-six]", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    replace_state(&mut pangine, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut pangine, "requested-consequence", "[request]->[saffron]->[basalt]");
    run_episode_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) },
        "changing only the requested consequence selects a different set of complete cedar episodes"
    );
    assert_eq!(read_named_weights(&mut pangine, "past-episode"), candidate_map(&[("[episode-eight]", 1), ("[episode-nine]", 1), ("[episode-seven]", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    clear_percept(&mut pangine, "episode-context");
    clear_percept(&mut pangine, "requested-consequence");
    experience(&mut pangine, "episode-context", "[request]->[cobalt]->[cedar]", 20);
    experience(&mut pangine, "requested-consequence", "[request]->[saffron]->[opal]", 20);
    run_episode_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) },
        "repeating the current situation and request filters episodes without multiplying their order support"
    );
    assert_eq!(read_named_weights(&mut pangine, "past-episode"), candidate_map(&[("[episode-one]", 1), ("[episode-three]", 1), ("[episode-two]", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    for episode in ["episode-ten", "episode-eleven"] {
        experience(&mut pangine, "order-episodes", &question_order_episode(episode, "cedar", "event-first", "opal"), 1);
    }
    run_episode_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) },
        "two additional complete matching episodes change the derived reasoning order without a summary update"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "past-episode"),
        candidate_map(&[("[episode-eleven]", 1), ("[episode-one]", 1), ("[episode-ten]", 1), ("[episode-three]", 1), ("[episode-two]", 1),])
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    must_ref(&mut pangine, "['question-order-record'] @ [outcome-first]->[conclusion]->['kept-outcome-conclusion']");
    must_ref(&mut pangine, "['question-order-record'] @ [event-first]->[conclusion]->['kept-event-conclusion']");
    let kept_outcome_conclusion = must_ref(&mut pangine, "$['kept-outcome-conclusion']");
    let kept_event_conclusion = must_ref(&mut pangine, "$['kept-event-conclusion']");
    assert_eq!(pangine.format_concept(&kept_outcome_conclusion, false), "[B]");
    assert_eq!(pangine.format_concept(&kept_event_conclusion, false), "[A]");
}

#[test]
#[ignore = "warning: equal pairwise episode totals can hide the complete correlation used by this explicit Pangine question"]
fn complete_episode_pairing_changes_choice_when_every_pairwise_total_matches() {
    let first_episodes = [
        ("episode-one", "cedar", "outcome-first", "opal"),
        ("episode-two", "cedar", "event-first", "basalt"),
        ("episode-three", "quartz", "outcome-first", "basalt"),
        ("episode-four", "quartz", "event-first", "opal"),
    ];
    let second_episodes = [
        ("episode-one", "cedar", "outcome-first", "basalt"),
        ("episode-two", "cedar", "event-first", "opal"),
        ("episode-three", "quartz", "outcome-first", "opal"),
        ("episode-four", "quartz", "event-first", "basalt"),
    ];

    let mut first = Pangine::new();
    populate_question_order_record(&mut first);
    clear_question_order_live_state(&mut first);
    remember_question_order_episodes(&mut first, &first_episodes);

    let mut second = Pangine::new();
    populate_question_order_record(&mut second);
    clear_question_order_live_state(&mut second);
    remember_question_order_episodes(&mut second, &second_episodes);

    let first_rows = complete_question_order_episode_rows(&mut first);
    let second_rows = complete_question_order_episode_rows(&mut second);
    assert_eq!(first_rows.len(), 4);
    assert_eq!(second_rows.len(), 4);
    assert_ne!(first_rows, second_rows, "the complete context-order-consequence rows must remain different");

    let context_order = episode_pair_map(&[
        ("[cedar]", "[event-first]", 1),
        ("[cedar]", "[outcome-first]", 1),
        ("[quartz]", "[event-first]", 1),
        ("[quartz]", "[outcome-first]", 1),
    ]);
    let first_context_order = episode_pair_totals(&first_rows, |row| (row.context.clone(), row.order.clone()));
    let second_context_order = episode_pair_totals(&second_rows, |row| (row.context.clone(), row.order.clone()));
    assert_eq!(first_context_order, context_order);
    assert_eq!(second_context_order, first_context_order);

    let context_consequence = episode_pair_map(&[("[cedar]", "[basalt]", 1), ("[cedar]", "[opal]", 1), ("[quartz]", "[basalt]", 1), ("[quartz]", "[opal]", 1)]);
    let first_context_consequence = episode_pair_totals(&first_rows, |row| (row.context.clone(), row.consequence.clone()));
    let second_context_consequence = episode_pair_totals(&second_rows, |row| (row.context.clone(), row.consequence.clone()));
    assert_eq!(first_context_consequence, context_consequence);
    assert_eq!(second_context_consequence, first_context_consequence);

    let order_consequence = episode_pair_map(&[
        ("[event-first]", "[basalt]", 1),
        ("[event-first]", "[opal]", 1),
        ("[outcome-first]", "[basalt]", 1),
        ("[outcome-first]", "[opal]", 1),
    ]);
    let first_order_consequence = episode_pair_totals(&first_rows, |row| (row.order.clone(), row.consequence.clone()));
    let second_order_consequence = episode_pair_totals(&second_rows, |row| (row.order.clone(), row.consequence.clone()));
    assert_eq!(first_order_consequence, order_consequence);
    assert_eq!(second_order_consequence, first_order_consequence);

    assert!(read_named_weights(&mut first, "order-guidance").is_empty());
    assert!(read_named_weights(&mut second, "order-guidance").is_empty());
    replace_state(&mut first, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut first, "requested-consequence", "[request]->[saffron]->[opal]");
    replace_state(&mut second, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut second, "requested-consequence", "[request]->[saffron]->[opal]");

    run_episode_question_order_program(&mut first);
    assert_eq!(read_named_weights(&mut first, "past-episode"), candidate_map(&[("[episode-one]", 1)]));
    assert_eq!(
        read_named_decision(&mut first, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[outcome-first]", 1)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    run_episode_question_order_program(&mut second);
    assert_eq!(read_named_weights(&mut second, "past-episode"), candidate_map(&[("[episode-two]", 1)]));
    assert_eq!(
        read_named_decision(&mut second, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: pairwise-equal repeated complete episodes choose different orders only under the current additive question and decision placeholder"]
fn weighted_complete_episode_pairing_changes_choice_when_every_pairwise_total_matches() {
    let mut first = Pangine::new();
    populate_question_order_record(&mut first);
    clear_question_order_live_state(&mut first);

    let mut second = Pangine::new();
    populate_question_order_record(&mut second);
    clear_question_order_live_state(&mut second);
    remember_pairwise_equal_weighted_question_order_episodes(&mut first, &mut second);

    let first_rows = complete_question_order_episode_rows(&mut first);
    let second_rows = complete_question_order_episode_rows(&mut second);
    assert_eq!(first_rows.len(), 8);
    assert_eq!(second_rows.len(), 8);
    assert_ne!(first_rows, second_rows, "the same complete episodes must retain their different experience amounts");

    let first_shapes =
        first_rows.iter().map(|row| (row.episode.clone(), row.context.clone(), row.order.clone(), row.consequence.clone())).collect::<BTreeSet<_>>();
    let second_shapes =
        second_rows.iter().map(|row| (row.episode.clone(), row.context.clone(), row.order.clone(), row.consequence.clone())).collect::<BTreeSet<_>>();
    assert_eq!(second_shapes, first_shapes, "both memories contain the same named complete episodes");
    assert_eq!(
        first_rows.iter().map(|row| (row.context.clone(), row.order.clone(), row.consequence.clone())).collect::<BTreeSet<_>>().len(),
        8,
        "every context-order-consequence combination is present"
    );

    assert_eq!(
        first_rows.iter().map(|row| (row.episode.clone(), row.relevance)).collect::<BTreeMap<_, _>>(),
        candidate_map(&[
            ("[episode-eight]", 1),
            ("[episode-five]", 1),
            ("[episode-four]", 2),
            ("[episode-one]", 2),
            ("[episode-seven]", 2),
            ("[episode-six]", 2),
            ("[episode-three]", 1),
            ("[episode-two]", 1),
        ])
    );
    assert_eq!(
        second_rows.iter().map(|row| (row.episode.clone(), row.relevance)).collect::<BTreeMap<_, _>>(),
        candidate_map(&[
            ("[episode-eight]", 2),
            ("[episode-five]", 2),
            ("[episode-four]", 1),
            ("[episode-one]", 1),
            ("[episode-seven]", 1),
            ("[episode-six]", 1),
            ("[episode-three]", 2),
            ("[episode-two]", 2),
        ])
    );

    let context_order = episode_pair_map(&[
        ("[cedar]", "[event-first]", 3),
        ("[cedar]", "[outcome-first]", 3),
        ("[quartz]", "[event-first]", 3),
        ("[quartz]", "[outcome-first]", 3),
    ]);
    let first_context_order = episode_pair_totals(&first_rows, |row| (row.context.clone(), row.order.clone()));
    let second_context_order = episode_pair_totals(&second_rows, |row| (row.context.clone(), row.order.clone()));
    assert_eq!(first_context_order, context_order);
    assert_eq!(second_context_order, first_context_order);

    let context_consequence = episode_pair_map(&[("[cedar]", "[basalt]", 3), ("[cedar]", "[opal]", 3), ("[quartz]", "[basalt]", 3), ("[quartz]", "[opal]", 3)]);
    let first_context_consequence = episode_pair_totals(&first_rows, |row| (row.context.clone(), row.consequence.clone()));
    let second_context_consequence = episode_pair_totals(&second_rows, |row| (row.context.clone(), row.consequence.clone()));
    assert_eq!(first_context_consequence, context_consequence);
    assert_eq!(second_context_consequence, first_context_consequence);

    let order_consequence = episode_pair_map(&[
        ("[event-first]", "[basalt]", 3),
        ("[event-first]", "[opal]", 3),
        ("[outcome-first]", "[basalt]", 3),
        ("[outcome-first]", "[opal]", 3),
    ]);
    let first_order_consequence = episode_pair_totals(&first_rows, |row| (row.order.clone(), row.consequence.clone()));
    let second_order_consequence = episode_pair_totals(&second_rows, |row| (row.order.clone(), row.consequence.clone()));
    assert_eq!(first_order_consequence, order_consequence);
    assert_eq!(second_order_consequence, first_order_consequence);

    assert!(read_named_weights(&mut first, "order-guidance").is_empty());
    assert!(read_named_weights(&mut second, "order-guidance").is_empty());
    replace_state(&mut first, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut first, "requested-consequence", "[request]->[saffron]->[opal]");
    replace_state(&mut second, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut second, "requested-consequence", "[request]->[saffron]->[opal]");

    run_episode_question_order_program(&mut first);
    assert_eq!(read_named_weights(&mut first, "past-episode"), candidate_map(&[("[episode-one]", 2), ("[episode-three]", 1)]));
    assert_eq!(
        read_named_decision(&mut first, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    run_episode_question_order_program(&mut second);
    assert_eq!(read_named_weights(&mut second, "past-episode"), candidate_map(&[("[episode-one]", 1), ("[episode-three]", 2)]));
    assert_eq!(
        read_named_decision(&mut second, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: represented consequence roles drive one explicit positive-minus-negative program, not a universal value or relevance rule"]
fn represented_consequence_stance_lets_every_current_context_outcome_shape_question_order() {
    let mut first = Pangine::new();
    populate_question_order_record(&mut first);
    clear_question_order_live_state(&mut first);

    let mut second = Pangine::new();
    populate_question_order_record(&mut second);
    clear_question_order_live_state(&mut second);
    remember_pairwise_equal_weighted_question_order_episodes(&mut first, &mut second);

    let first_rows = complete_question_order_episode_rows(&mut first);
    let second_rows = complete_question_order_episode_rows(&mut second);
    replace_state(&mut first, "episode-context", "[request]->[cobalt]->[cedar]");
    replace_state(&mut second, "episode-context", "[request]->[cobalt]->[cedar]");

    replace_state(&mut first, "requested-consequence", "[request]->[saffron]->[opal]");
    replace_state(&mut second, "requested-consequence", "[request]->[saffron]->[opal]");
    run_episode_question_order_program(&mut first);
    run_episode_question_order_program(&mut second);
    assert_eq!(
        read_named_decision(&mut first, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    clear_percept(&mut first, "requested-consequence");
    clear_percept(&mut second, "requested-consequence");
    set_consequence_stance(&mut first, "opal", "basalt");
    set_consequence_stance(&mut second, "opal", "basalt");
    run_stance_question_order_program(&mut first);
    run_stance_question_order_program(&mut second);

    assert!(read_named_weights(&mut first, "requested-consequence").is_empty());
    assert!(read_named_weights(&mut second, "requested-consequence").is_empty());
    assert_eq!(read_named_weights(&mut first, "positive-episode"), candidate_map(&[("[episode-one]", 2), ("[episode-three]", 1)]));
    assert_eq!(read_named_weights(&mut first, "negative-episode"), candidate_map(&[("[episode-four]", 2), ("[episode-two]", 1)]));
    assert_eq!(
        read_named_decision(&mut first, "positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", -1), ("[outcome-first]", 1)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    assert_eq!(read_named_weights(&mut second, "positive-episode"), candidate_map(&[("[episode-one]", 1), ("[episode-three]", 2)]));
    assert_eq!(read_named_weights(&mut second, "negative-episode"), candidate_map(&[("[episode-four]", 1), ("[episode-two]", 2)]));
    assert_eq!(
        read_named_decision(&mut second, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", -1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut first, "consequence-stance", "[opal]->[role]->[positive]", 19);
    run_stance_question_order_program(&mut first);
    assert_eq!(
        read_named_weights(&mut first, "consequence-stance"),
        candidate_map(&[("{[basalt]->[role]->[negative]}", 1), ("{[opal]->[role]->[positive]}", 20)])
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", -1), ("[outcome-first]", 1)]), selected: Some("[outcome-first]".to_owned()) }
    );

    set_consequence_stance(&mut first, "basalt", "opal");
    set_consequence_stance(&mut second, "basalt", "opal");
    run_stance_question_order_program(&mut first);
    run_stance_question_order_program(&mut second);

    assert_eq!(read_named_weights(&mut first, "positive-episode"), candidate_map(&[("[episode-four]", 2), ("[episode-two]", 1)]));
    assert_eq!(read_named_weights(&mut first, "negative-episode"), candidate_map(&[("[episode-one]", 2), ("[episode-three]", 1)]));
    assert_eq!(
        read_named_decision(&mut first, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", -1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut first, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    assert_eq!(read_named_weights(&mut second, "positive-episode"), candidate_map(&[("[episode-four]", 1), ("[episode-two]", 2)]));
    assert_eq!(read_named_weights(&mut second, "negative-episode"), candidate_map(&[("[episode-one]", 1), ("[episode-three]", 2)]));
    assert_eq!(
        read_named_decision(&mut second, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", -1), ("[outcome-first]", 1)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut second, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    assert!(read_named_weights(&mut first, "order-guidance").is_empty());
    assert!(read_named_weights(&mut second, "order-guidance").is_empty());
    assert_eq!(complete_question_order_episode_rows(&mut first), first_rows);
    assert_eq!(complete_question_order_episode_rows(&mut second), second_rows);
}

#[test]
#[ignore = "warning: source-identified valuation reports remain filters in this question, not a chosen sensor aggregation or relevance rule"]
fn source_identified_valuation_reports_preserve_identity_without_multiplying_order_evidence() {
    let mut repeated_report = Pangine::new();
    let mut distinct_reports_one_source = Pangine::new();
    let mut distinct_reports_distinct_sources = Pangine::new();
    let mut replaceable_report = Pangine::new();

    for pangine in [&mut repeated_report, &mut distinct_reports_one_source, &mut distinct_reports_distinct_sources, &mut replaceable_report] {
        prepare_first_weighted_question_order_memory(pangine);
        experience(pangine, "valuation-control", &valuation_report("negative-report-one", "negative-source-one", "basalt", "negative"), 1);
        experience(pangine, "valuation-control", &valuation_report("negative-report-two", "negative-source-two", "basalt", "negative"), 1);
    }

    let repeated_positive = valuation_report("positive-report", "positive-source", "opal", "positive");
    experience(&mut repeated_report, "valuation-reports", &repeated_positive, 3);

    for report in ["positive-report-one", "positive-report-two", "positive-report-three"] {
        experience(&mut distinct_reports_one_source, "valuation-reports", &valuation_report(report, "positive-source", "opal", "positive"), 1);
    }
    for (report, source) in
        [("positive-report-one", "positive-source-one"), ("positive-report-two", "positive-source-two"), ("positive-report-three", "positive-source-three")]
    {
        experience(&mut distinct_reports_distinct_sources, "valuation-reports", &valuation_report(report, source, "opal", "positive"), 1);
    }

    replace_state(&mut replaceable_report, "valuation-reports", &repeated_positive);
    let replaceable_source = replaceable_report.reference_percept("valuation-reports");
    let replaceable_state = replaceable_report.get_relevance_map(&replaceable_source);
    replace_state(&mut replaceable_report, "valuation-reports", &repeated_positive);
    replace_state(&mut replaceable_report, "valuation-reports", &repeated_positive);
    assert_eq!(replaceable_report.get_relevance_map(&replaceable_source), replaceable_state);

    let controls =
        [("negative-report-one", "negative-source-one", "basalt", "negative", 1), ("negative-report-two", "negative-source-two", "basalt", "negative", 1)];
    assert_eq!(
        complete_valuation_report_rows(&mut repeated_report),
        valuation_report_map(&[controls[0], controls[1], ("positive-report", "positive-source", "opal", "positive", 3),])
    );
    assert_eq!(
        complete_valuation_report_rows(&mut distinct_reports_one_source),
        valuation_report_map(&[
            controls[0],
            controls[1],
            ("positive-report-one", "positive-source", "opal", "positive", 1),
            ("positive-report-two", "positive-source", "opal", "positive", 1),
            ("positive-report-three", "positive-source", "opal", "positive", 1),
        ])
    );
    assert_eq!(
        complete_valuation_report_rows(&mut distinct_reports_distinct_sources),
        valuation_report_map(&[
            controls[0],
            controls[1],
            ("positive-report-one", "positive-source-one", "opal", "positive", 1),
            ("positive-report-two", "positive-source-two", "opal", "positive", 1),
            ("positive-report-three", "positive-source-three", "opal", "positive", 1),
        ])
    );
    assert_eq!(
        complete_valuation_report_rows(&mut replaceable_report),
        valuation_report_map(&[controls[0], controls[1], ("positive-report", "positive-source", "opal", "positive", 1),])
    );

    let episode_rows = complete_question_order_episode_rows(&mut repeated_report);
    assert_eq!(complete_question_order_episode_rows(&mut distinct_reports_one_source), episode_rows);
    assert_eq!(complete_question_order_episode_rows(&mut distinct_reports_distinct_sources), episode_rows);
    assert_eq!(complete_question_order_episode_rows(&mut replaceable_report), episode_rows);

    for pangine in [&mut repeated_report, &mut distinct_reports_one_source, &mut distinct_reports_distinct_sources, &mut replaceable_report] {
        run_valuation_report_question_order_program(pangine);
        assert!(read_named_weights(pangine, "requested-consequence").is_empty());
        assert!(read_named_weights(pangine, "order-guidance").is_empty());
        assert_eq!(
            read_named_decision(pangine, "negative-order"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
        );
    }

    assert_eq!(read_named_weights(&mut repeated_report, "positive-report"), candidate_map(&[("[positive-report]", 3)]));
    assert_eq!(
        read_named_weights(&mut distinct_reports_one_source, "positive-report"),
        candidate_map(&[("[positive-report-one]", 1), ("[positive-report-three]", 1), ("[positive-report-two]", 1)])
    );
    assert_eq!(
        read_named_weights(&mut distinct_reports_distinct_sources, "positive-report"),
        candidate_map(&[("[positive-report-one]", 1), ("[positive-report-three]", 1), ("[positive-report-two]", 1)])
    );
    assert_eq!(read_named_weights(&mut replaceable_report, "positive-report"), candidate_map(&[("[positive-report]", 1)]));

    assert_eq!(
        read_named_decision(&mut repeated_report, "positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_report, "positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    for pangine in [&mut distinct_reports_one_source, &mut distinct_reports_distinct_sources] {
        assert_eq!(
            read_named_decision(pangine, "positive-order"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
        );
    }

    for pangine in [&mut repeated_report, &mut distinct_reports_one_source, &mut distinct_reports_distinct_sources, &mut replaceable_report] {
        assert_eq!(
            read_named_decision(pangine, "episode-order-net"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", -1), ("[outcome-first]", 1)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-state"),
            DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-choice"),
            DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
        );
    }
}

#[test]
#[ignore = "warning: complete order-bearing valuation observations expose current additive behavior, not universal sensor semantics"]
fn complete_valuation_observations_make_repetition_and_distinct_reports_matter_while_replaceable_state_stays_current() {
    let mut repeated_observation = Pangine::new();
    let mut distinct_observations_one_source = Pangine::new();
    let mut distinct_observations_distinct_sources = Pangine::new();
    let mut simultaneous_current_observations = Pangine::new();
    let mut replaceable_observation = Pangine::new();

    for pangine in [
        &mut repeated_observation,
        &mut distinct_observations_one_source,
        &mut distinct_observations_distinct_sources,
        &mut simultaneous_current_observations,
        &mut replaceable_observation,
    ] {
        prepare_complete_valuation_observation_memory(pangine);
    }

    let repeated_positive =
        valuation_observation("outcome-positive-report", "outcome-positive-source", "episode-one", "cedar", "outcome-first", "opal", "positive");
    experience(&mut repeated_observation, "valuation-observations", &repeated_positive, 4);

    for report in ["outcome-positive-report-one", "outcome-positive-report-two", "outcome-positive-report-three", "outcome-positive-report-four"] {
        experience(
            &mut distinct_observations_one_source,
            "valuation-observations",
            &valuation_observation(report, "outcome-positive-source", "episode-one", "cedar", "outcome-first", "opal", "positive"),
            1,
        );
    }
    for (report, source) in [
        ("outcome-positive-report-one", "outcome-positive-source-one"),
        ("outcome-positive-report-two", "outcome-positive-source-two"),
        ("outcome-positive-report-three", "outcome-positive-source-three"),
        ("outcome-positive-report-four", "outcome-positive-source-four"),
    ] {
        experience(
            &mut distinct_observations_distinct_sources,
            "valuation-observations",
            &valuation_observation(report, source, "episode-one", "cedar", "outcome-first", "opal", "positive"),
            1,
        );
    }
    for (percept, report, source) in [
        ("valuation-current-one", "outcome-positive-report-one", "outcome-positive-source-one"),
        ("valuation-current-two", "outcome-positive-report-two", "outcome-positive-source-two"),
        ("valuation-current-three", "outcome-positive-report-three", "outcome-positive-source-three"),
        ("valuation-current-four", "outcome-positive-report-four", "outcome-positive-source-four"),
    ] {
        replace_state(
            &mut simultaneous_current_observations,
            percept,
            &valuation_observation(report, source, "episode-one", "cedar", "outcome-first", "opal", "positive"),
        );
    }

    replace_state(&mut replaceable_observation, "valuation-observations", &repeated_positive);
    let replaceable_source = replaceable_observation.reference_percept("valuation-observations");
    let replaceable_state = replaceable_observation.get_relevance_map(&replaceable_source);
    for _ in 0..3 {
        replace_state(&mut replaceable_observation, "valuation-observations", &repeated_positive);
    }
    assert_eq!(replaceable_observation.get_relevance_map(&replaceable_source), replaceable_state);

    let episode_rows = complete_question_order_episode_rows(&mut repeated_observation);
    assert_eq!(complete_question_order_episode_rows(&mut distinct_observations_one_source), episode_rows);
    assert_eq!(complete_question_order_episode_rows(&mut distinct_observations_distinct_sources), episode_rows);
    assert_eq!(complete_question_order_episode_rows(&mut simultaneous_current_observations), episode_rows);
    assert_eq!(complete_question_order_episode_rows(&mut replaceable_observation), episode_rows);

    for pangine in [
        &mut repeated_observation,
        &mut distinct_observations_one_source,
        &mut distinct_observations_distinct_sources,
        &mut simultaneous_current_observations,
        &mut replaceable_observation,
    ] {
        run_complete_valuation_observation_question_order_program(pangine);
        assert!(read_named_weights(pangine, "requested-consequence").is_empty());
        assert!(read_named_weights(pangine, "order-guidance").is_empty());
        assert_eq!(
            read_named_decision(pangine, "negative-order"),
            DecisionResult { candidates: candidate_map(&[("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
        );
    }

    assert_eq!(
        read_named_weights(&mut repeated_observation, "positive-observation"),
        candidate_map(&[("[outcome-positive-report]", 4), ("[event-positive-report]", 1)])
    );
    assert_eq!(
        read_named_weights(&mut replaceable_observation, "positive-observation"),
        candidate_map(&[("[outcome-positive-report]", 1), ("[event-positive-report]", 1)])
    );
    assert_eq!(
        read_named_weights(&mut distinct_observations_one_source, "positive-observation"),
        candidate_map(&[
            ("[event-positive-report]", 1),
            ("[outcome-positive-report-four]", 1),
            ("[outcome-positive-report-one]", 1),
            ("[outcome-positive-report-three]", 1),
            ("[outcome-positive-report-two]", 1),
        ])
    );
    assert_eq!(
        read_named_weights(&mut distinct_observations_distinct_sources, "positive-observation"),
        candidate_map(&[
            ("[event-positive-report]", 1),
            ("[outcome-positive-report-four]", 1),
            ("[outcome-positive-report-one]", 1),
            ("[outcome-positive-report-three]", 1),
            ("[outcome-positive-report-two]", 1),
        ])
    );
    assert_eq!(
        read_named_weights(&mut simultaneous_current_observations, "positive-observation"),
        candidate_map(&[
            ("[event-positive-report]", 1),
            ("[outcome-positive-report-four]", 1),
            ("[outcome-positive-report-one]", 1),
            ("[outcome-positive-report-three]", 1),
            ("[outcome-positive-report-two]", 1),
        ])
    );

    for pangine in
        [&mut repeated_observation, &mut distinct_observations_one_source, &mut distinct_observations_distinct_sources, &mut simultaneous_current_observations]
    {
        assert_eq!(
            read_named_decision(pangine, "positive-order"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 6)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-net"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-state"),
            DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-choice"),
            DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
        );
    }

    assert_eq!(
        read_named_decision(&mut replaceable_observation, "positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: staged validation can separate report support from episode support, but neither weighting treatment is universal"]
fn staged_validation_can_admit_complete_observations_without_adding_episode_relevance() {
    let mut repeated_observation = Pangine::new();
    let mut simultaneous_current_observations = Pangine::new();
    let mut replaceable_observation = Pangine::new();
    for pangine in [&mut repeated_observation, &mut simultaneous_current_observations, &mut replaceable_observation] {
        prepare_complete_valuation_observation_memory(pangine);
    }

    let repeated_positive =
        valuation_observation("outcome-positive-report", "outcome-positive-source", "episode-one", "cedar", "outcome-first", "opal", "positive");
    experience(&mut repeated_observation, "valuation-observations", &repeated_positive, 4);
    for (percept, report, source) in [
        ("valuation-current-one", "outcome-positive-report-one", "outcome-positive-source-one"),
        ("valuation-current-two", "outcome-positive-report-two", "outcome-positive-source-two"),
        ("valuation-current-three", "outcome-positive-report-three", "outcome-positive-source-three"),
        ("valuation-current-four", "outcome-positive-report-four", "outcome-positive-source-four"),
    ] {
        replace_state(
            &mut simultaneous_current_observations,
            percept,
            &valuation_observation(report, source, "episode-one", "cedar", "outcome-first", "opal", "positive"),
        );
    }
    for _ in 0..4 {
        replace_state(&mut replaceable_observation, "valuation-observations", &repeated_positive);
    }

    for pangine in [&mut repeated_observation, &mut simultaneous_current_observations] {
        run_complete_valuation_observation_question_order_program(pangine);
        assert_eq!(
            read_named_decision(pangine, "positive-order"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 6)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-net"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "episode-order-choice"),
            DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
        );
    }
    run_complete_valuation_observation_question_order_program(&mut replaceable_observation);
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    for pangine in [&mut repeated_observation, &mut simultaneous_current_observations, &mut replaceable_observation] {
        run_staged_valuation_observation_question_order_program(pangine);
        assert!(read_named_weights(pangine, "requested-consequence").is_empty());
        assert!(read_named_weights(pangine, "order-guidance").is_empty());
        assert!(!read_named_weights(pangine, "validation-key-observation").contains_key("[mismatched-report]"));
        assert_eq!(
            read_named_decision(pangine, "staged-negative-order"),
            DecisionResult { candidates: candidate_map(&[("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
        );
    }

    let repeated_validation_keys = read_named_weights(&mut repeated_observation, "validation-key-observation").into_keys().collect::<BTreeSet<_>>();
    let replaceable_validation_keys = read_named_weights(&mut replaceable_observation, "validation-key-observation").into_keys().collect::<BTreeSet<_>>();
    let single_source_keys = BTreeSet::from([
        "[event-positive-report]".to_owned(),
        "[outcome-negative-report-one]".to_owned(),
        "[outcome-negative-report-two]".to_owned(),
        "[outcome-positive-report]".to_owned(),
    ]);
    assert_eq!(repeated_validation_keys, single_source_keys);
    assert_eq!(replaceable_validation_keys, single_source_keys);
    assert_eq!(
        read_named_weights(&mut simultaneous_current_observations, "validation-key-observation").into_keys().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "[event-positive-report]".to_owned(),
            "[outcome-negative-report-one]".to_owned(),
            "[outcome-negative-report-two]".to_owned(),
            "[outcome-positive-report-four]".to_owned(),
            "[outcome-positive-report-one]".to_owned(),
            "[outcome-positive-report-three]".to_owned(),
            "[outcome-positive-report-two]".to_owned(),
        ])
    );

    for pangine in [&mut repeated_observation, &mut simultaneous_current_observations] {
        assert_eq!(
            read_named_decision(pangine, "staged-positive-order"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 4)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "staged-episode-order-net"),
            DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "staged-episode-order-state"),
            DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
        );
        assert_eq!(
            read_named_decision(pangine, "staged-episode-order-choice"),
            DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
        );
    }

    assert_eq!(
        read_named_decision(&mut replaceable_observation, "staged-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "staged-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", -1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "staged-episode-order-state"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut replaceable_observation, "staged-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: treating validation memory as support can change the answer, but this fixture does not choose either policy"]
fn validation_as_support_or_filter_can_choose_different_orders_from_the_same_observations() {
    let mut pangine = Pangine::new();
    prepare_balanced_valuation_observation_memory(&mut pangine);

    run_complete_valuation_observation_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 4), ("[outcome-first]", 4)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    run_staged_valuation_observation_question_order_program(&mut pangine);
    assert!(!read_named_weights(&mut pangine, "validation-key-observation").contains_key("[mismatched-report]"));
    assert_eq!(
        read_named_decision(&mut pangine, "staged-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "staged-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "staged-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "staged-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: represented origin identity and independence can route support inside one fixed program, but this remains one explicit policy"]
fn represented_report_provenance_routes_one_fixed_validation_program() {
    let mut same_occurrence = Pangine::new();
    let mut independent_corroboration = Pangine::new();
    for pangine in [&mut same_occurrence, &mut independent_corroboration] {
        prepare_balanced_valuation_observation_memory(pangine);
    }

    assert_eq!(read_named_weights(&mut same_occurrence, "order-episodes"), read_named_weights(&mut independent_corroboration, "order-episodes"));
    assert_eq!(
        read_named_weights(&mut same_occurrence, "valuation-observations"),
        read_named_weights(&mut independent_corroboration, "valuation-observations")
    );
    assert_eq!(read_named_weights(&mut same_occurrence, "episode-context"), read_named_weights(&mut independent_corroboration, "episode-context"));
    assert_eq!(read_named_weights(&mut same_occurrence, "question-order-record"), read_named_weights(&mut independent_corroboration, "question-order-record"));

    for pangine in [&mut same_occurrence, &mut independent_corroboration] {
        remember_common_balanced_valuation_origins(pangine);
    }
    remember_valuation_origin(&mut same_occurrence, "outcome-positive-report", "episode-one-origin");
    remember_valuation_origin(&mut independent_corroboration, "outcome-positive-report", "outcome-positive-report-origin");
    remember_origin_independence(&mut independent_corroboration, "outcome-positive-report-origin", "episode-one-origin");

    run_provenance_routed_valuation_observation_question_order_program(&mut same_occurrence);
    run_provenance_routed_valuation_observation_question_order_program(&mut independent_corroboration);

    for pangine in [&mut same_occurrence, &mut independent_corroboration] {
        assert!(!read_named_weights(pangine, "validation-key-observation").contains_key("[mismatched-report]"));
        assert!(!read_named_weights(pangine, "independent-positive-observation").contains_key("[mismatched-report]"));
    }
    assert_eq!(
        read_named_weights(&mut same_occurrence, "validation-key-observation"),
        read_named_weights(&mut independent_corroboration, "validation-key-observation")
    );

    assert!(read_named_weights(&mut same_occurrence, "independent-positive-order").is_empty());
    assert_eq!(read_named_weights(&mut same_occurrence, "same-occurrence-positive-order"), candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]));
    assert_eq!(read_named_weights(&mut independent_corroboration, "independent-positive-order"), candidate_map(&[("[outcome-first]", 4)]));
    assert_eq!(read_named_weights(&mut independent_corroboration, "same-occurrence-positive-order"), candidate_map(&[("[event-first]", 3)]));
    for pangine in [&mut same_occurrence, &mut independent_corroboration] {
        assert!(read_named_weights(pangine, "independent-negative-order").is_empty());
        assert_eq!(read_named_weights(pangine, "same-occurrence-negative-order"), candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]));
    }

    assert_eq!(
        read_named_decision(&mut same_occurrence, "provenance-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut same_occurrence, "provenance-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut same_occurrence, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut same_occurrence, "provenance-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    assert_eq!(
        read_named_decision(&mut independent_corroboration, "provenance-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 4)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut independent_corroboration, "provenance-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut independent_corroboration, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut independent_corroboration, "provenance-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: repeated routing facts stay neutral while multiple admissible origins add both routes, so partial correlation remains unresolved"]
fn repeated_and_ambiguous_provenance_expose_the_fixed_routing_boundary() {
    let mut same_occurrence = Pangine::new();
    let mut repeated_same_occurrence = Pangine::new();
    let mut mixed_origins = Pangine::new();
    for pangine in [&mut same_occurrence, &mut repeated_same_occurrence, &mut mixed_origins] {
        prepare_balanced_valuation_observation_memory(pangine);
        remember_common_balanced_valuation_origins(pangine);
    }

    remember_valuation_origin(&mut same_occurrence, "outcome-positive-report", "episode-one-origin");
    for _ in 0..20 {
        remember_valuation_origin(&mut repeated_same_occurrence, "outcome-positive-report", "episode-one-origin");
    }
    remember_valuation_origin(&mut mixed_origins, "outcome-positive-report", "episode-one-origin");
    remember_valuation_origin(&mut mixed_origins, "outcome-positive-report", "outcome-positive-report-origin");
    remember_origin_independence(&mut mixed_origins, "outcome-positive-report-origin", "episode-one-origin");

    for pangine in [&mut same_occurrence, &mut repeated_same_occurrence, &mut mixed_origins] {
        run_provenance_routed_valuation_observation_question_order_program(pangine);
        assert!(!read_named_weights(pangine, "validation-key-observation").contains_key("[mismatched-report]"));
    }

    assert_eq!(
        read_named_weights(&mut same_occurrence, "same-occurrence-positive-order"),
        read_named_weights(&mut repeated_same_occurrence, "same-occurrence-positive-order")
    );
    assert_eq!(
        read_named_decision(&mut same_occurrence, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut repeated_same_occurrence, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut repeated_same_occurrence, "provenance-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    assert_eq!(read_named_weights(&mut mixed_origins, "same-occurrence-positive-order"), candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]));
    assert_eq!(read_named_weights(&mut mixed_origins, "independent-positive-order"), candidate_map(&[("[outcome-first]", 4)]));
    assert_eq!(
        read_named_decision(&mut mixed_origins, "provenance-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 6)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut mixed_origins, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 5)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut mixed_origins, "provenance-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: report-first and origin-first provenance questions preserve different experience and neither is a universal correlation policy"]
fn report_first_and_origin_first_questions_disagree_on_one_overlapping_origin_corpus() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);

    run_provenance_routed_valuation_observation_question_order_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "provenance-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 6)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "provenance-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 4)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "provenance-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    run_origin_first_valuation_observation_question_order_program(&mut pangine);
    retrieve_origin_first_question_order_choice(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "origin-positive-origin"),
        DecisionResult {
            candidates: candidate_map(&[("[episode-one-origin]", 1), ("[episode-three-origin]", 3), ("[outcome-positive-report-origin]", 1)]),
            selected: Some("[episode-three-origin]".to_owned())
        }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "origin-negative-origin"),
        DecisionResult {
            candidates: candidate_map(&[("[episode-four-origin]", 1), ("[episode-two-origin]", 2)]),
            selected: Some("[episode-two-origin]".to_owned())
        }
    );
    assert_eq!(
        read_named_weights(&mut pangine, "origin-first-positive-observation"),
        candidate_map(&[("[event-positive-report-one]", 3), ("[event-positive-report-three]", 3), ("[event-positive-report-two]", 3)])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "origin-first-negative-observation"),
        candidate_map(&[("[outcome-negative-report-one]", 3), ("[outcome-negative-report-two]", 3)])
    );
    assert_eq!(
        read_named_decision(&mut pangine, "origin-first-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", -2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "origin-first-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    must_ref(
        &mut pangine,
        "['overlapping-origin-question-order-record'] =
           ([report-first]->[positive-order]->$['provenance-positive-order'])
           ([report-first]->[negative-order]->$['provenance-negative-order'])
           ([report-first]->[net-order]->$['provenance-episode-order-net'])
           ([report-first]->[conclusion]->$['provenance-episode-order-choice'])
           ([origin-first]->[positive-origins]->$['origin-positive-origin'])
           ([origin-first]->[negative-origins]->$['origin-negative-origin'])
           ([origin-first]->[positive-order]->$['origin-first-positive-order'])
           ([origin-first]->[negative-order]->$['origin-first-negative-order'])
           ([origin-first]->[net-order]->$['origin-first-episode-order-net'])
           ([origin-first]->[conclusion]->$['origin-first-episode-order-choice'])",
    );
    for percept in ["order-episodes", "episode-context", "valuation-observations", "valuation-provenance", "question-order-record"] {
        clear_percept(&mut pangine, percept);
    }
    must_ref(&mut pangine, "['overlapping-origin-question-order-record'] @ [report-first]->[conclusion]->['recorded-report-first-conclusion']");
    must_ref(&mut pangine, "['overlapping-origin-question-order-record'] @ [origin-first]->[conclusion]->['recorded-origin-first-conclusion']");
    must_ref(&mut pangine, "['overlapping-origin-question-order-record'] @ [report-first]->[net-order]->['recorded-report-first-net']");
    must_ref(&mut pangine, "['overlapping-origin-question-order-record'] @ [origin-first]->[positive-origins]->['recorded-origin-positive-origins']");
    must_ref(&mut pangine, "['overlapping-origin-question-order-record'] @ [origin-first]->[net-order]->['recorded-origin-first-net']");
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-report-first-conclusion"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-origin-first-conclusion"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-report-first-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 4)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-origin-positive-origins"),
        DecisionResult {
            candidates: candidate_map(&[("[episode-one-origin]", 1), ("[episode-three-origin]", 3), ("[outcome-positive-report-origin]", 1)]),
            selected: Some("[episode-three-origin]".to_owned())
        }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-origin-first-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", -2)]), selected: Some("[event-first]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: origin experience changes the origin-first question while remaining a filter in the report-first question"]
fn repeated_origin_experience_changes_only_the_question_that_selects_an_origin() {
    let mut baseline = Pangine::new();
    let mut repeated_origin = Pangine::new();
    for pangine in [&mut baseline, &mut repeated_origin] {
        prepare_overlapping_origin_question_order_memory(pangine);
    }
    for _ in 0..3 {
        remember_valuation_origin(&mut repeated_origin, "outcome-positive-report", "episode-one-origin");
    }

    for pangine in [&mut baseline, &mut repeated_origin] {
        run_provenance_routed_valuation_observation_question_order_program(pangine);
    }
    assert_eq!(read_named_decision(&mut baseline, "provenance-episode-order-net"), read_named_decision(&mut repeated_origin, "provenance-episode-order-net"));
    assert_eq!(
        read_named_decision(&mut repeated_origin, "provenance-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 4)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut repeated_origin, "provenance-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );

    run_origin_first_valuation_observation_question_order_program(&mut baseline);
    retrieve_origin_first_question_order_choice(&mut baseline);
    run_origin_first_valuation_observation_question_order_program(&mut repeated_origin);
    assert_eq!(
        read_named_decision(&mut baseline, "origin-positive-origin"),
        DecisionResult {
            candidates: candidate_map(&[("[episode-one-origin]", 1), ("[episode-three-origin]", 3), ("[outcome-positive-report-origin]", 1)]),
            selected: Some("[episode-three-origin]".to_owned())
        }
    );
    assert_eq!(
        read_named_decision(&mut baseline, "origin-first-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", -2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut baseline, "origin-first-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut repeated_origin, "origin-positive-origin"),
        DecisionResult {
            candidates: candidate_map(&[("[episode-one-origin]", 4), ("[episode-three-origin]", 3), ("[outcome-positive-report-origin]", 1)]),
            selected: Some("[episode-one-origin]".to_owned())
        }
    );
    assert_eq!(read_named_weights(&mut repeated_origin, "origin-first-positive-order"), candidate_map(&[("[outcome-first]", 2)]));
    assert_eq!(read_named_weights(&mut repeated_origin, "origin-first-negative-order"), candidate_map(&[("[outcome-first]", 2)]));
    assert_eq!(read_named_decision(&mut repeated_origin, "origin-first-episode-order-net"), DecisionResult { candidates: BTreeMap::new(), selected: None });
    assert_eq!(read_named_decision(&mut repeated_origin, "origin-first-episode-order-choice"), DecisionResult { candidates: BTreeMap::new(), selected: None });
}

#[test]
#[ignore = "warning: distinct-origin staging preserves original decision amounts but report bindings also contain join support and are not a general report record"]
fn all_origins_stage_keeps_each_origin_once_and_rejoins_original_order_amounts() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);
    let original_observations = must_ref(&mut pangine, "$['valuation-observations']");
    let original_provenance = must_ref(&mut pangine, "$['valuation-provenance']");

    run_all_origins_valuation_observation_question_order_program(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['valuation-observations']"), original_observations.clone());
    assert_eq!(must_ref(&mut pangine, "$['valuation-provenance']"), original_provenance.clone());
    assert_eq!(
        read_named_weights(&mut pangine, "all-origin"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "all-origin-positive-observation"),
        candidate_map(&[
            ("[event-positive-report-one]", 3),
            ("[event-positive-report-three]", 3),
            ("[event-positive-report-two]", 3),
            ("[outcome-positive-report]", 4),
        ]),
        "report bindings contain support from the join and are not the original report amounts"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "all-origin-negative-observation"),
        candidate_map(&[("[event-negative-report]", 3), ("[outcome-negative-report-one]", 3), ("[outcome-negative-report-two]", 3)]),
        "report bindings contain support from the join and are not the original report amounts"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "all-origin-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "all-origin-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "all-origin-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    retrieve_all_origins_question_order_choice(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "all-origin-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );

    must_ref(
        &mut pangine,
        "['all-origins-question-order-record'] =
           ([all-origins]->[origins]->$['all-origin'])
           ([all-origins]->[source-reports]->$['valuation-observations'])
           ([all-origins]->[source-provenance]->$['valuation-provenance'])
           ([all-origins]->[positive-report-bindings]->$['all-origin-positive-observation'])
           ([all-origins]->[negative-report-bindings]->$['all-origin-negative-observation'])
           ([all-origins]->[positive-order]->$['all-origin-positive-order'])
           ([all-origins]->[negative-order]->$['all-origin-negative-order'])
           ([all-origins]->[net-order]->$['all-origin-episode-order-net'])
           ([all-origins]->[conclusion]->$['all-origin-episode-order-choice'])",
    );
    for percept in [
        "order-episodes",
        "episode-context",
        "valuation-observations",
        "valuation-provenance",
        "question-order-record",
        "validated-observation-rows",
        "validated-observation-keys",
        "all-origin-report-rows",
        "all-origin-projection-rows",
        "all-origin",
        "all-origin-positive-observation",
        "all-origin-negative-observation",
        "all-origin-positive-order",
        "all-origin-negative-order",
        "all-origin-episode-order-net",
        "all-origin-episode-order-choice",
    ] {
        clear_percept(&mut pangine, percept);
    }
    must_ref(&mut pangine, "['all-origins-question-order-record'] @ [all-origins]->[origins]->['recorded-all-origins']");
    must_ref(&mut pangine, "['all-origins-question-order-record'] @ [all-origins]->[source-reports]->['recorded-all-origin-source-reports']");
    must_ref(&mut pangine, "['all-origins-question-order-record'] @ [all-origins]->[source-provenance]->['recorded-all-origin-source-provenance']");
    must_ref(
        &mut pangine,
        "['all-origins-question-order-record'] @ [all-origins]->[positive-report-bindings]->['recorded-all-origin-positive-report-bindings']",
    );
    must_ref(&mut pangine, "['all-origins-question-order-record'] @ [all-origins]->[net-order]->['recorded-all-origin-net']");
    must_ref(&mut pangine, "['all-origins-question-order-record'] @ [all-origins]->[conclusion]->['recorded-all-origin-conclusion']");
    assert_eq!(
        read_named_weights(&mut pangine, "recorded-all-origins"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "recorded-all-origin-positive-report-bindings"),
        candidate_map(&[
            ("[event-positive-report-one]", 3),
            ("[event-positive-report-three]", 3),
            ("[event-positive-report-two]", 3),
            ("[outcome-positive-report]", 4),
        ])
    );
    assert_eq!(must_ref(&mut pangine, "$['recorded-all-origin-source-reports']"), original_observations);
    assert_eq!(must_ref(&mut pangine, "$['recorded-all-origin-source-provenance']"), original_provenance);
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-all-origin-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "recorded-all-origin-conclusion"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: origin-link and report repetition differ only because this explicit staged question gives them different roles"]
fn all_origins_stage_ignores_origin_link_repetition_but_preserves_report_repetition() {
    let mut baseline = Pangine::new();
    let mut repeated_origin = Pangine::new();
    let mut repeated_report = Pangine::new();
    let mut episode_only_origin = Pangine::new();
    for pangine in [&mut baseline, &mut repeated_origin, &mut repeated_report, &mut episode_only_origin] {
        prepare_overlapping_origin_question_order_memory(pangine);
    }
    for _ in 0..3 {
        remember_valuation_origin(&mut repeated_origin, "outcome-positive-report", "episode-one-origin");
    }
    experience(
        &mut repeated_report,
        "valuation-observations",
        &valuation_observation("outcome-positive-report", "source-one", "episode-one", "cedar", "outcome-first", "opal", "positive"),
        3,
    );
    remember_valuation_origin(&mut episode_only_origin, "episode-one", "episode-only-origin");

    for pangine in [&mut baseline, &mut repeated_origin, &mut repeated_report, &mut episode_only_origin] {
        run_all_origins_valuation_observation_question_order_program(pangine);
    }
    assert_eq!(read_named_weights(&mut baseline, "all-origin"), read_named_weights(&mut repeated_origin, "all-origin"));
    assert_eq!(read_named_weights(&mut baseline, "all-origin"), read_named_weights(&mut repeated_report, "all-origin"));
    assert_eq!(read_named_weights(&mut baseline, "all-origin"), read_named_weights(&mut episode_only_origin, "all-origin"));
    assert_eq!(
        read_named_weights(&mut repeated_origin, "all-origin"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ])
    );
    assert_eq!(read_named_weights(&mut baseline, "all-origin-positive-order"), read_named_weights(&mut repeated_origin, "all-origin-positive-order"));
    assert_eq!(read_named_weights(&mut baseline, "all-origin-negative-order"), read_named_weights(&mut repeated_origin, "all-origin-negative-order"));
    assert_eq!(read_named_weights(&mut baseline, "all-origin-episode-order-net"), read_named_weights(&mut repeated_origin, "all-origin-episode-order-net"));
    assert_eq!(
        read_named_weights(&mut baseline, "all-origin-episode-order-choice"),
        read_named_weights(&mut repeated_origin, "all-origin-episode-order-choice")
    );
    assert_eq!(read_named_weights(&mut baseline, "all-origin-positive-order"), read_named_weights(&mut episode_only_origin, "all-origin-positive-order"));
    assert_eq!(read_named_weights(&mut baseline, "all-origin-negative-order"), read_named_weights(&mut episode_only_origin, "all-origin-negative-order"));
    assert_eq!(read_named_weights(&mut baseline, "all-origin-episode-order-net"), read_named_weights(&mut episode_only_origin, "all-origin-episode-order-net"));
    assert_eq!(
        read_named_weights(&mut repeated_report, "all-origin-positive-observation"),
        candidate_map(&[
            ("[event-positive-report-one]", 3),
            ("[event-positive-report-three]", 3),
            ("[event-positive-report-two]", 3),
            ("[outcome-positive-report]", 7),
        ]),
        "joined report bindings include path support while the order output retains the report amount"
    );
    assert_eq!(
        read_named_decision(&mut repeated_report, "all-origin-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 5)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut repeated_report, "all-origin-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
    );
    for pangine in [&mut baseline, &mut repeated_origin, &mut repeated_report, &mut episode_only_origin] {
        retrieve_all_origins_question_order_choice(pangine);
    }
    assert_eq!(
        read_named_weights(&mut baseline, "all-origin-episode-order-choice"),
        read_named_weights(&mut episode_only_origin, "all-origin-episode-order-choice")
    );
    assert_eq!(
        read_named_decision(&mut repeated_report, "all-origin-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: the one-each origin marginal is observable but current questions cannot turn it into one tagged link per member"]
fn distinct_origin_state_does_not_materialize_one_rejoinable_link_per_origin() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);
    run_all_origins_valuation_observation_question_order_program(&mut pangine);

    must_run(&mut pangine, "['constructed-origin-links'] = (['all-origin'] @ [eligible-origin]->['constructed-origin'])");
    assert!(read_named_weights(&mut pangine, "constructed-origin-links").is_empty());
    assert!(read_named_weights(&mut pangine, "constructed-origin").is_empty());

    let grouped = must_ref(&mut pangine, "['grouped-origin-link'] = [eligible-origin]->$['all-origin']");
    assert_eq!(
        pangine.format_concept(&grouped, false),
        "{[eligible-origin]->[episode-four-origin][episode-one-origin][episode-three-origin][episode-two-origin][outcome-positive-report-origin]}"
    );
    must_run(
        &mut pangine,
        "['grouped-origin-link']['all-origin-projection-rows'] @
           ([eligible-origin]->['rejoined-origin'])
           (['rejoined-report']->[origin]->['rejoined-origin'])",
    );
    assert!(read_named_weights(&mut pangine, "rejoined-origin").is_empty());
    assert!(read_named_weights(&mut pangine, "rejoined-report").is_empty());
}

#[test]
#[ignore = "warning: enclosing report-origin rows preserve correlations, but assigned rows do not retain original source amounts"]
fn complete_origin_report_rows_keep_correlations_but_not_source_amounts_through_a_second_question() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);
    remember_enclosed_overlapping_origin_observations(&mut pangine, 2, 1);
    let original_enclosed_observations = must_ref(&mut pangine, "$['enclosed-valuation-observations']");

    run_enclosed_origin_report_row_program(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['enclosed-valuation-observations']"), original_enclosed_observations);
    let expected_reports = candidate_map(&[
        ("[event-negative-report]", 1),
        ("[event-positive-report-one]", 1),
        ("[event-positive-report-three]", 1),
        ("[event-positive-report-two]", 1),
        ("[outcome-negative-report-one]", 1),
        ("[outcome-negative-report-two]", 1),
        ("[outcome-positive-report]", 2),
    ]);
    assert_eq!(read_named_weights(&mut pangine, "enclosed-stage-report"), expected_reports);
    assert_eq!(
        read_named_weights(&mut pangine, "enclosed-stage-origin-group"),
        candidate_map(&[
            ("{[event-negative-report]->[origin]->[episode-four-origin]}", 1),
            ("{[event-positive-report-one]->[origin]->[episode-three-origin]}", 1),
            ("{[event-positive-report-three]->[origin]->[episode-three-origin]}", 1),
            ("{[event-positive-report-two]->[origin]->[episode-three-origin]}", 1),
            ("{[outcome-negative-report-one]->[origin]->[episode-two-origin]}", 1),
            ("{[outcome-negative-report-two]->[origin]->[episode-two-origin]}", 1),
            ("{[outcome-positive-report]->[origin]->[episode-one-origin]}{[outcome-positive-report]->[origin]->[outcome-positive-report-origin]}", 2,),
        ])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "enclosed-stage-origin"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "enclosed-stage-order"),
        candidate_map(&[("[event-first]", 7), ("[outcome-first]", 7)]),
        "the shared order binding receives support from both the reports and the remembered episodes"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "enclosed-stage-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 7), ("[outcome-first]", 7)]), selected: Some("[event-first]".to_owned()) }
    );
    let one_each_reports = candidate_map(&[
        ("[event-negative-report]", 1),
        ("[event-positive-report-one]", 1),
        ("[event-positive-report-three]", 1),
        ("[event-positive-report-two]", 1),
        ("[outcome-negative-report-one]", 1),
        ("[outcome-negative-report-two]", 1),
        ("[outcome-positive-report]", 1),
    ]);
    assert_eq!(read_named_weights(&mut pangine, "enclosed-recalled-report"), one_each_reports);
    assert_eq!(
        read_named_weights(&mut pangine, "enclosed-recalled-origin-group"),
        candidate_map(&[
            ("{[event-negative-report]->[origin]->[episode-four-origin]}", 1),
            ("{[event-positive-report-one]->[origin]->[episode-three-origin]}", 1),
            ("{[event-positive-report-three]->[origin]->[episode-three-origin]}", 1),
            ("{[event-positive-report-two]->[origin]->[episode-three-origin]}", 1),
            ("{[outcome-negative-report-one]->[origin]->[episode-two-origin]}", 1),
            ("{[outcome-negative-report-two]->[origin]->[episode-two-origin]}", 1),
            ("{[outcome-positive-report]->[origin]->[episode-one-origin]}{[outcome-positive-report]->[origin]->[outcome-positive-report-origin]}", 1,),
        ])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "enclosed-recalled-origin"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ])
    );
    let recalled_origin_groups = pangine.reference_percept("enclosed-recalled-origin-group");
    let recalled_pair_report = pangine.reference_percept("enclosed-recalled-pair-report");
    let recalled_pair_origin = pangine.reference_percept("enclosed-recalled-pair-origin");
    let recalled_pair_question = must_ref(&mut pangine, "(['enclosed-recalled-pair-report']->[origin]->['enclosed-recalled-pair-origin'])");
    let recalled_pairs = pangine
        .complete_question(std::slice::from_ref(&recalled_origin_groups), &recalled_pair_question)
        .expect("valid recalled report-origin question")
        .completions()
        .iter()
        .map(|completion| {
            (
                pangine.format_concept(completion.binding(&recalled_pair_report).expect("bound recalled report"), false),
                pangine.format_concept(completion.binding(&recalled_pair_origin).expect("bound recalled origin"), false),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        recalled_pairs,
        BTreeSet::from([
            ("[event-negative-report]".to_owned(), "[episode-four-origin]".to_owned()),
            ("[event-positive-report-one]".to_owned(), "[episode-three-origin]".to_owned()),
            ("[event-positive-report-three]".to_owned(), "[episode-three-origin]".to_owned()),
            ("[event-positive-report-two]".to_owned(), "[episode-three-origin]".to_owned()),
            ("[outcome-negative-report-one]".to_owned(), "[episode-two-origin]".to_owned()),
            ("[outcome-negative-report-two]".to_owned(), "[episode-two-origin]".to_owned()),
            ("[outcome-positive-report]".to_owned(), "[episode-one-origin]".to_owned()),
            ("[outcome-positive-report]".to_owned(), "[outcome-positive-report-origin]".to_owned()),
        ]),
        "the second assigned row view retains every report-to-origin pairing even though its source amounts have become one"
    );
    assert_eq!(read_named_weights(&mut pangine, "enclosed-recalled-order"), candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "enclosed-recalled-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: repetition controls probe enclosing report and nested origin experience separately"]
fn enclosing_report_repetition_and_origin_link_repetition_remain_distinct() {
    let mut repeated_report = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut repeated_report);
    remember_enclosed_overlapping_origin_observations(&mut repeated_report, 5, 1);
    run_enclosed_origin_report_row_program(&mut repeated_report);

    let mut repeated_origin = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut repeated_origin);
    remember_enclosed_overlapping_origin_observations(&mut repeated_origin, 2, 4);
    run_enclosed_origin_report_row_program(&mut repeated_origin);

    assert_eq!(read_named_weights(&mut repeated_report, "enclosed-stage-report").get("[outcome-positive-report]"), Some(&Relevance::new(5)));
    assert_eq!(
        read_named_weights(&mut repeated_report, "enclosed-stage-origin-group")
            .get("{[outcome-positive-report]->[origin]->[episode-one-origin]}{[outcome-positive-report]->[origin]->[outcome-positive-report-origin]}"),
        Some(&Relevance::new(5))
    );
    assert_eq!(
        read_named_decision(&mut repeated_report, "enclosed-stage-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 7), ("[outcome-first]", 10)]), selected: Some("[outcome-first]".to_owned()) },
        "three more complete outcome-first reports change Pangine's first-stage answer"
    );
    assert_eq!(
        read_named_weights(&mut repeated_report, "enclosed-stage-origin"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ]),
        "asking the assigned origin-group Percept is already beyond the source-weight boundary"
    );
    assert_eq!(read_named_weights(&mut repeated_report, "enclosed-recalled-report").get("[outcome-positive-report]"), Some(&Relevance::DEFAULT));
    assert_eq!(
        read_named_decision(&mut repeated_report, "enclosed-recalled-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 1)]), selected: Some("[event-first]".to_owned()) },
        "the second question keeps the row but no longer sees the five original firings"
    );

    assert_eq!(read_named_weights(&mut repeated_origin, "enclosed-stage-report").get("[outcome-positive-report]"), Some(&Relevance::new(2)));
    assert_eq!(
        read_named_weights(&mut repeated_origin, "enclosed-stage-origin-group")
            .get("x4{[outcome-positive-report]->[origin]->[episode-one-origin]}{[outcome-positive-report]->[origin]->[outcome-positive-report-origin]}"),
        Some(&Relevance::new(2)),
        "the four origin links remain visible as structure inside the complete report"
    );
    assert_eq!(
        read_named_decision(&mut repeated_origin, "enclosed-stage-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 7), ("[outcome-first]", 7)]), selected: Some("[event-first]".to_owned()) },
        "repeating one nested origin link is not the same experience as repeating the complete outcome-first report"
    );
    assert_eq!(
        read_named_weights(&mut repeated_origin, "enclosed-stage-origin"),
        candidate_map(&[
            ("[episode-four-origin]", 1),
            ("[episode-one-origin]", 1),
            ("[episode-three-origin]", 1),
            ("[episode-two-origin]", 1),
            ("[outcome-positive-report-origin]", 1),
        ]),
        "the current later origin question does not interpret the nested coefficient as four independent firings"
    );
}

#[test]
#[ignore = "warning: a represented source reference can rejoin selected report-origin rows, but it remains a live reference rather than a weight snapshot"]
fn represented_source_reference_rejoins_origin_report_rows_to_original_amounts() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);
    let original_observations = must_ref(&mut pangine, "$['valuation-observations']");
    run_all_origins_valuation_observation_question_order_program(&mut pangine);

    prepare_referenced_origin_report_rows(&mut pangine);
    let observation_source = pangine.reference_percept("valuation-observations");
    let source_holder = pangine.reference_percept("referenced-original-source");
    assert_eq!(pangine.get_value(&source_holder), Some(observation_source));
    assert_eq!(must_ref(&mut pangine, "$['valuation-observations']"), original_observations);

    run_referenced_origin_report_row_rejoin_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2)]), selected: Some("[event-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_weights(&mut pangine, "referenced-positive-order"),
        read_named_weights(&mut pangine, "all-origin-positive-order"),
        "the represented reference recovers the same original report amounts as the direct source selector"
    );
    assert_eq!(read_named_weights(&mut pangine, "referenced-negative-order"), read_named_weights(&mut pangine, "all-origin-negative-order"));
}

#[test]
#[ignore = "warning: referenced row rejoin fixes selected report-origin identities, not the later state of their live source"]
fn referenced_origin_report_rows_use_later_amounts_only_for_already_selected_reports() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);
    run_all_origins_valuation_observation_question_order_program(&mut pangine);
    prepare_referenced_origin_report_rows(&mut pangine);
    let referenced_rows_percept = pangine.reference_percept("referenced-origin-report-rows");
    let referenced_rows = pangine.get_value(&referenced_rows_percept).expect("stored referenced report-origin rows");
    let originally_evaluated_rows = must_ref(&mut pangine, "$['referenced-origin-report-rows']");

    experience(
        &mut pangine,
        "valuation-observations",
        &valuation_observation("outcome-positive-report", "source-one", "episode-one", "cedar", "outcome-first", "opal", "positive"),
        3,
    );
    experience(
        &mut pangine,
        "valuation-observations",
        &valuation_observation("later-valid-report", "later-source", "episode-three", "cedar", "event-first", "opal", "positive"),
        20,
    );

    run_referenced_origin_report_row_rejoin_program(&mut pangine);
    assert_eq!(pangine.get_value(&referenced_rows_percept), Some(referenced_rows));
    assert_ne!(
        must_ref(&mut pangine, "$['referenced-origin-report-rows']"),
        originally_evaluated_rows,
        "evaluating a stored live reference sees later source state"
    );
    assert!(!read_named_weights(&mut pangine, "referenced-positive-report").contains_key("[later-valid-report]"));
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 5)]), selected: Some("[outcome-first]".to_owned()) },
        "later experience for an already selected report changes the amount supplied by the live source"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-episode-order-net"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 2), ("[outcome-first]", 3)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[B]", 1)]), selected: Some("[B]".to_owned()) },
        "the later report with a new identity stays out because the stored rows never selected it"
    );
}

#[test]
#[ignore = "warning: multiple represented source references make source selection another provisional decision"]
fn multiple_represented_source_references_are_an_explicit_source_choice() {
    let mut pangine = Pangine::new();
    prepare_overlapping_origin_question_order_memory(&mut pangine);
    run_all_origins_valuation_observation_question_order_program(&mut pangine);
    experience(&mut pangine, "origin-report-source-pointer", "[origin-report-question]->[source]->['valuation-observations']", 1);
    experience(&mut pangine, "origin-report-source-pointer", "[origin-report-question]->[source]->['alternate-valuation-observations']", 1);
    materialize_referenced_origin_report_rows(&mut pangine);

    let candidates = read_named_weights(&mut pangine, "referenced-original-source");
    let selected = must_ref(&mut pangine, "^['referenced-original-source']");
    assert_eq!(
        candidates.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["['alternate-valuation-observations']".to_owned(), "['valuation-observations']".to_owned()])
    );
    assert_eq!(candidates["['alternate-valuation-observations']"], candidates["['valuation-observations']"]);
    assert_eq!(selected, pangine.reference_percept("alternate-valuation-observations"));

    experience(&mut pangine, "origin-report-source-pointer", "[origin-report-question]->[source]->['valuation-observations']", 1);
    materialize_referenced_origin_report_rows(&mut pangine);
    must_ref(&mut pangine, "['referenced-origin-report-rows'] @ [origin-report-question]->[source]->['copied-source-candidate']");
    assert_eq!(
        read_named_weights(&mut pangine, "copied-source-candidate"),
        candidate_map(&[("['alternate-valuation-observations']", 1), ("['valuation-observations']", 1)]),
        "the completed rows retain both source identities but flatten their earlier 2-to-1 experience"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "referenced-original-source"),
        candidate_map(&[("['alternate-valuation-observations']", 1), ("['valuation-observations']", 2)])
    );
    assert_eq!(
        must_ref(&mut pangine, "^['referenced-original-source']"),
        pangine.reference_percept("valuation-observations"),
        "one more source-reference experience resolves the tie without a host-side choice"
    );

    run_referenced_origin_report_row_rejoin_program(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-positive-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 3), ("[outcome-first]", 2)]), selected: Some("[event-first]".to_owned()) },
        "the unselected pointer row does not multiply the selected source's report amounts"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-negative-order"),
        DecisionResult { candidates: candidate_map(&[("[event-first]", 1), ("[outcome-first]", 2)]), selected: Some("[outcome-first]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "referenced-episode-order-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: chained inventory rows preserve live source histories but currently turn repeated references into one membership row each"]
fn chained_reference_inventory_supplies_each_live_memory_without_host_enumeration() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "inventory-memory-a", "[mark]->[reports]->[A]", 3);
    experience(&mut pangine, "inventory-memory-a", "[mark]->[reports]->[B]", 1);
    experience(&mut pangine, "inventory-memory-b", "[mark]->[reports]->[A]", 1);
    experience(&mut pangine, "inventory-memory-b", "[mark]->[reports]->[B]", 4);
    experience(&mut pangine, "memory-inventory", "['inventory-memory-a']", 20);
    experience(&mut pangine, "memory-inventory", "['inventory-memory-b']", 1);

    let inventory_rows = must_ref(&mut pangine, "['memory-inventory'] @ ['listed-memory-rows']");
    assert_eq!(
        pangine
            .get_relevance_map(&inventory_rows)
            .into_iter()
            .map(|(relevance, memory)| (pangine.format_concept(&memory, false), relevance))
            .collect::<BTreeMap<_, _>>(),
        candidate_map(&[("['inventory-memory-a']", 1), ("['inventory-memory-b']", 1)]),
        "completion rows retain each referenced memory once even though the output Percept keeps the inventory's 20-to-1 experience"
    );
    must_run(&mut pangine, "(['memory-inventory'] @ ['listed-memory']) @ [mark]->[reports]->['inventory-answer']");
    assert_eq!(
        read_named_weights(&mut pangine, "listed-memory"),
        candidate_map(&[("['inventory-memory-a']", 20), ("['inventory-memory-b']", 1)]),
        "the inner question retains the inventory's reference experience"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "inventory-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 5)]), selected: Some("[B]".to_owned()) },
        "the returned reference rows become separate live sources, so their original report histories rather than flattened values shape the answer"
    );

    experience(&mut pangine, "inventory-memory-a", "[mark]->[reports]->[A]", 2);
    must_run(&mut pangine, "(['memory-inventory'] @ ['listed-memory-later']) @ [mark]->[reports]->['inventory-answer-later']");
    assert_eq!(
        read_named_decision(&mut pangine, "inventory-answer-later"),
        DecisionResult { candidates: candidate_map(&[("[A]", 6), ("[B]", 5)]), selected: Some("[A]".to_owned()) },
        "the inventory continues to follow later experience in every referenced memory"
    );

    must_run(&mut pangine, "($['memory-inventory']) @ [mark]->[reports]->['evaluated-inventory-answer']");
    assert_eq!(
        read_named_decision(&mut pangine, "evaluated-inventory-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 1)]), selected: Some("[A]".to_owned()) },
        "evaluating the inventory as one value still flattens the source histories that chaining preserves"
    );
}

#[test]
#[ignore = "warning: current inventory rows count repeated references as one membership while distinct live memories and repeated reports remain separate answer experience"]
fn repeated_reference_distinct_memories_and_repeated_reports_remain_different_inventory_experience() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "inventory-sensor-a-one", "[pulse]->[supports]->[A]", 1);
    experience(&mut pangine, "inventory-sensor-a-two", "[pulse]->[supports]->[A]", 1);
    experience(&mut pangine, "inventory-sensor-a-three", "[pulse]->[supports]->[A]", 1);
    experience(&mut pangine, "inventory-sensor-b", "[pulse]->[supports]->[B]", 2);

    experience(&mut pangine, "repeated-reference-inventory", "['inventory-sensor-a-one']", 3);
    experience(&mut pangine, "repeated-reference-inventory", "['inventory-sensor-b']", 1);
    for sensor in ["inventory-sensor-a-one", "inventory-sensor-a-two", "inventory-sensor-a-three", "inventory-sensor-b"] {
        experience(&mut pangine, "distinct-reference-inventory", &format!("['{sensor}']"), 1);
    }

    must_run(&mut pangine, "(['repeated-reference-inventory'] @ ['repeated-listed-sensor']) @ [pulse]->[supports]->['repeated-reference-answer']");
    assert_eq!(read_named_weights(&mut pangine, "repeated-listed-sensor"), candidate_map(&[("['inventory-sensor-a-one']", 3), ("['inventory-sensor-b']", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "repeated-reference-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 2)]), selected: Some("[B]".to_owned()) },
        "three references to one A memory still produce one active memory beside the B memory"
    );

    must_run(&mut pangine, "(['distinct-reference-inventory'] @ ['distinct-listed-sensor']) @ [pulse]->[supports]->['distinct-reference-answer']");
    assert_eq!(
        read_named_weights(&mut pangine, "distinct-listed-sensor"),
        candidate_map(&[
            ("['inventory-sensor-a-one']", 1),
            ("['inventory-sensor-a-three']", 1),
            ("['inventory-sensor-a-two']", 1),
            ("['inventory-sensor-b']", 1),
        ])
    );
    assert_eq!(
        read_named_decision(&mut pangine, "distinct-reference-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 2)]), selected: Some("[A]".to_owned()) },
        "three distinct A memories contribute their separate report experience"
    );

    experience(&mut pangine, "inventory-sensor-a-one", "[pulse]->[supports]->[A]", 2);
    must_run(&mut pangine, "(['repeated-reference-inventory'] @ ['repeated-listed-sensor-later']) @ [pulse]->[supports]->['repeated-report-answer']");
    assert_eq!(
        read_named_decision(&mut pangine, "repeated-report-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 2)]), selected: Some("[A]".to_owned()) },
        "three reports inside one referenced memory remain three units of answer experience even though three references to that memory were one membership"
    );
}

#[test]
#[ignore = "warning: complete activation observations are one provisional answer-bearing representation, not a universal meaning for repeated source references"]
fn complete_activation_observations_can_shape_an_answer_without_reinterpreting_pointer_repetition() {
    let mut pangine = prepare_explicit_activation_comparison();
    assert_eq!(
        read_named_weights(&mut pangine, "activation-pointer-state"),
        candidate_map(&[("['opal-sensor']", 3), ("['quartz-sensor']", 1)]),
        "the pure pointer history remains visible without becoming answer support"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "retained-report-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) },
        "following each referenced sensor once leaves the retained reports responsible for this answer"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "activation-event"),
        candidate_map(&[("[lantern]", 3), ("[topaz-one]", 1), ("[topaz-two]", 1)]),
        "the repeated activation and two distinct activations remain different event histories"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "activation-source"),
        candidate_map(&[("['opal-sensor']", 3), ("['quartz-sensor']", 2)]),
        "each complete activation binds the sensor that supplied the reading"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "activation-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "the represented activations rather than pointer repetition supply this answer's amounts"
    );

    let violet_activation_one = "([topaz-one]->[amber]->['quartz-sensor'])([topaz-one]->[cobalt]->[violet])";
    experience(&mut pangine, "activation-observations", violet_activation_one, 2);
    must_run(
        &mut pangine,
        "['activation-observations'] @ (['activation-event-later']->[amber]->['activation-source-later'])(['activation-event-later']->[cobalt]->['activation-answer-later'])",
    );
    assert_eq!(read_named_weights(&mut pangine, "activation-event-later"), candidate_map(&[("[lantern]", 3), ("[topaz-one]", 3), ("[topaz-two]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "activation-source-later"), candidate_map(&[("['opal-sensor']", 3), ("['quartz-sensor']", 4)]));
    assert_eq!(
        read_named_decision(&mut pangine, "activation-answer-later"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 4)]), selected: Some("[violet]".to_owned()) },
        "two later activation experiences change Pangine's answer without a host-side branch or total"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "activation-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "the earlier activation state and answer remain separately inspectable"
    );
}

#[test]
#[ignore = "warning: context-linked answer-view episodes select between preserved states only under this explicit provisional question, while an empty route cannot yet form the next link"]
fn represented_context_selects_between_preserved_report_and_activation_answers_without_host_branch() {
    let mut pangine = prepare_explicit_activation_comparison();
    experience(&mut pangine, "activation-answer-view-episodes", &answer_view_episode("silver-repeat", "silver", "fern"), 2);
    experience(&mut pangine, "activation-answer-view-episodes", &answer_view_episode("silver-other", "silver", "moss"), 1);
    experience(&mut pangine, "activation-answer-view-episodes", &answer_view_episode("ochre-other", "ochre", "fern"), 1);
    experience(&mut pangine, "activation-answer-view-episodes", &answer_view_episode("ochre-one", "ochre", "moss"), 1);
    experience(&mut pangine, "activation-answer-view-episodes", &answer_view_episode("ochre-two", "ochre", "moss"), 1);
    experience(&mut pangine, "activation-answer-view-episodes", &answer_view_episode("obsidian-noise", "obsidian", "fern"), 20);

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[silver]");
    must_ref(
        &mut pangine,
        "['activation-answer-view-record'] =
           ([fern]->[rose]->$['retained-report-answer'])
           ([moss]->[rose]->$['activation-answer'])",
    );
    run_context_selected_activation_answer(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "routing-episode"),
        candidate_map(&[("[silver-other]", 1), ("[silver-repeat]", 2)]),
        "only complete episodes matching the represented current context enter the view decision"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "routing-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 2), ("[moss]", 1)]), selected: Some("[fern]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "context-selected-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) },
        "the selected report view keeps its original candidate amounts"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "context-selected-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[violet]", 1)]), selected: Some("[violet]".to_owned()) }
    );

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[ochre]");
    run_context_selected_activation_answer(&mut pangine);
    assert_eq!(read_named_weights(&mut pangine, "routing-episode"), candidate_map(&[("[ochre-one]", 1), ("[ochre-other]", 1), ("[ochre-two]", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "routing-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 1), ("[moss]", 2)]), selected: Some("[moss]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "context-selected-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "the selected firing view keeps its original candidate amounts"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "context-selected-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1)]), selected: Some("[cedar]".to_owned()) }
    );

    assert_eq!(
        read_named_decision(&mut pangine, "retained-report-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "activation-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "both unselected source answers remain visible and unchanged"
    );

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[onyx]");
    derive_context_answer_view(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "routing-view"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "an unknown context has no represented reason to select either answer view"
    );
    assert!(
        pangine.reference_concept("['selected-answer-view'] = [selected-view]->^['routing-view']").is_err(),
        "the current grammar cannot place an empty decision inside the ordered selected-view link"
    );
}

#[test]
#[ignore = "warning: matching context and requested result selects a preserved answer view only under this explicit correlation question, not because Pangine knows that result is useful or caused by the view"]
fn represented_requested_result_can_select_a_less_common_answer_view_without_host_scoring() {
    let mut pangine = prepare_explicit_activation_comparison();
    for (event, context, view, result, repetitions) in [
        ("silver-fern-opal", "silver", "fern", "opal", 1),
        ("silver-fern-basalt", "silver", "fern", "basalt", 3),
        ("silver-moss-opal-one", "silver", "moss", "opal", 1),
        ("silver-moss-opal-two", "silver", "moss", "opal", 1),
        ("silver-moss-basalt", "silver", "moss", "basalt", 1),
        ("obsidian-noise", "obsidian", "fern", "opal", 20),
    ] {
        experience(&mut pangine, "activation-answer-view-result-episodes", &answer_view_result_episode(event, context, view, result), repetitions);
    }

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[silver]");
    must_run(
        &mut pangine,
        "['activation-answer-view-result-episodes']['activation-answer-context'] @
           (['frequency-episode']->[amber]->['frequency-context'])
           (['frequency-episode']->[violet]->['frequency-view'])
           ([request]->[cobalt]->['frequency-context'])",
    );
    assert_eq!(
        read_named_decision(&mut pangine, "frequency-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 4), ("[moss]", 3)]), selected: Some("[fern]".to_owned()) },
        "when the result is ignored, silver experience makes the report view more common"
    );

    must_ref(
        &mut pangine,
        "['activation-answer-view-record'] =
           ([fern]->[rose]->$['retained-report-answer'])
           ([moss]->[rose]->$['activation-answer'])",
    );
    replace_state(&mut pangine, "requested-answer-result", "[request]->[saffron]->[opal]");
    run_context_result_selected_activation_answer(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "result-routing-episode"),
        candidate_map(&[("[silver-fern-opal]", 1), ("[silver-moss-opal-one]", 1), ("[silver-moss-opal-two]", 1)]),
        "the complete join keeps only silver episodes with the requested opal result"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "result-routing-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 1), ("[moss]", 2)]), selected: Some("[moss]".to_owned()) },
        "the firing view wins among episodes with the requested result even though it is less common overall"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "result-selected-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "the result-selected firing view keeps its original candidate amounts"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "result-selected-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1)]), selected: Some("[cedar]".to_owned()) }
    );

    replace_state(&mut pangine, "requested-answer-result", "[request]->[saffron]->[basalt]");
    run_context_result_selected_activation_answer(&mut pangine);
    assert_eq!(read_named_weights(&mut pangine, "result-routing-episode"), candidate_map(&[("[silver-fern-basalt]", 3), ("[silver-moss-basalt]", 1)]));
    assert_eq!(
        read_named_decision(&mut pangine, "result-routing-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 3), ("[moss]", 1)]), selected: Some("[fern]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "result-selected-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) },
        "changing only the requested result selects the unchanged report answer"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "result-selected-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[violet]", 1)]), selected: Some("[violet]".to_owned()) }
    );

    assert_eq!(
        read_named_decision(&mut pangine, "retained-report-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "activation-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "matching a result does not alter or combine either source answer"
    );

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[onyx]");
    replace_state(&mut pangine, "requested-answer-result", "[request]->[saffron]->[opal]");
    derive_context_result_answer_view(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "result-routing-view"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "an unknown context remains unanswered even when the requested result exists elsewhere"
    );
    assert!(
        pangine.reference_concept("['result-selected-answer-view'] = [selected-view]->^['result-routing-view']").is_err(),
        "the current grammar still cannot place an empty decision inside the next ordered link"
    );
}

#[test]
#[ignore = "warning: choosing a matching observed answer before mapping it to a preserved view is one provisional question order; a direct all-row join keeps each view once, equal conclusions remain ambiguous, and no causal meaning is implied"]
fn observed_choice_and_result_derive_answer_view_without_episode_view_labels() {
    let mut pangine = prepare_explicit_activation_comparison();
    must_run(&mut pangine, "['retained-report-choice'] = ^['retained-report-answer']");
    must_run(&mut pangine, "['activation-answer-choice'] = ^['activation-answer']");
    must_ref(
        &mut pangine,
        "['activation-answer-observation-record'] =
           ([fern]->[rose]->$['retained-report-answer'])
           ([fern]->[topaz]->$['retained-report-choice'])
           ([moss]->[rose]->$['activation-answer'])
           ([moss]->[topaz]->$['activation-answer-choice'])",
    );

    for (event, context, choice, result, repetitions) in [
        ("silver-violet-opal", "silver", "violet", "opal", 1),
        ("silver-violet-basalt", "silver", "violet", "basalt", 3),
        ("silver-cedar-opal-one", "silver", "cedar", "opal", 1),
        ("silver-cedar-opal-two", "silver", "cedar", "opal", 1),
        ("silver-cedar-basalt", "silver", "cedar", "basalt", 1),
        ("obsidian-noise", "obsidian", "violet", "opal", 20),
    ] {
        experience(&mut pangine, "activation-answer-choice-result-episodes", &answer_choice_result_episode(event, context, choice, result), repetitions);
    }

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-answer-result", "[request]->[saffron]->[opal]");
    derive_all_choice_result_answer_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "direct-choice-routing-episode"),
        candidate_map(&[("[silver-cedar-opal-one]", 1), ("[silver-cedar-opal-two]", 1), ("[silver-violet-opal]", 1)]),
        "the direct question finds every matching episode"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "direct-choice-derived-answer-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 1), ("[moss]", 1)]), selected: Some("[fern]".to_owned()) },
        "the direct join keeps one proof from each stored view instead of transferring matching episode amounts"
    );

    run_choice_result_derived_activation_answer(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "choice-routing-episode"),
        candidate_map(&[("[silver-cedar-opal-one]", 1), ("[silver-cedar-opal-two]", 1), ("[silver-violet-opal]", 1)]),
        "the matching episodes contain choices and results but no answer-view names"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "observed-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 2), ("[violet]", 1)]), selected: Some("[cedar]".to_owned()) },
        "the first stage preserves the matching episode amounts and selects the more experienced past choice"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "choice-derived-answer-view"),
        DecisionResult { candidates: candidate_map(&[("[moss]", 1)]), selected: Some("[moss]".to_owned()) },
        "the second stage derives the view whose preserved conclusion matches Pangine's selected past choice"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "choice-derived-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "choice-derived-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1)]), selected: Some("[cedar]".to_owned()) }
    );

    replace_state(&mut pangine, "requested-answer-result", "[request]->[saffron]->[basalt]");
    run_choice_result_derived_activation_answer(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "observed-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 3)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "choice-derived-answer-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 1)]), selected: Some("[fern]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "choice-derived-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "choice-derived-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[violet]", 1)]), selected: Some("[violet]".to_owned()) }
    );

    must_ref(
        &mut pangine,
        "['ambiguous-answer-view-record'] =
           ([fern]->[topaz]->[violet])
           ([moss]->[topaz]->[violet])",
    );
    must_run(
        &mut pangine,
        "['ambiguous-answer-view-record']['selected-past-answer-choice'] @
           (['ambiguous-answer-view']->[topaz]->['ambiguous-recorded-answer-choice'])
           ([past-choice]->['ambiguous-recorded-answer-choice'])",
    );
    assert_eq!(
        read_named_decision(&mut pangine, "ambiguous-answer-view"),
        DecisionResult { candidates: candidate_map(&[("[fern]", 1), ("[moss]", 1)]), selected: Some("[fern]".to_owned()) },
        "one observed choice cannot distinguish two views that currently have the same conclusion"
    );

    assert_eq!(
        read_named_decision(&mut pangine, "retained-report-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "activation-answer"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) },
        "deriving the view from observed choices still leaves both source answers unchanged"
    );

    replace_state(&mut pangine, "activation-answer-context", "[request]->[cobalt]->[onyx]");
    replace_state(&mut pangine, "requested-answer-result", "[request]->[saffron]->[opal]");
    derive_observed_answer_choice(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "observed-answer-choice"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "an unknown context supplies no observed answer for the later view mapping"
    );
    assert!(
        pangine.reference_concept("['selected-past-answer-choice'] = [past-choice]->^['observed-answer-choice']").is_err(),
        "the staged program stops at the same current empty-decision boundary instead of using an application fallback"
    );
}

#[test]
#[ignore = "warning: record-linked label-free observations recover a view only when that saved decision distinguishes it; selecting a record before a choice is one provisional question order, not automatic time or causal semantics"]
fn saved_decision_records_prevent_current_answers_from_reinterpreting_label_free_experience() {
    let mut pangine = prepare_historical_answer_record_comparison();

    for (event, context, record, choice, result, repetitions) in [
        ("copper-violet", "silver", "copper", "violet", "opal", 2),
        ("copper-cedar", "silver", "copper", "cedar", "opal", 1),
        ("silver-violet", "silver", "silver-record", "violet", "basalt", 2),
        ("silver-cedar", "silver", "silver-record", "cedar", "basalt", 1),
        ("gold-violet", "silver", "gold", "violet", "garnet", 2),
        ("gold-cedar", "silver", "gold", "cedar", "garnet", 1),
        ("obsidian-noise", "obsidian", "copper", "violet", "opal", 20),
    ] {
        experience(&mut pangine, "historical-answer-choice-episodes", &historical_answer_choice_episode(event, context, record, choice, result), repetitions);
    }

    assert_eq!(
        read_named_decision(&mut pangine, "copper-fern-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "copper-moss-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "silver-fern-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "silver-moss-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 4)]), selected: Some("[violet]".to_owned()) },
        "the middle record preserves genuine agreement between the two views"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "gold-fern-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 4), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "gold-moss-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 4)]), selected: Some("[violet]".to_owned()) },
        "the later record reverses which view has violet as its conclusion"
    );

    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[basalt]");
    run_record_linked_choice_mapping(&mut pangine);
    assert_eq!(read_named_weights(&mut pangine, "historical-decision-record"), candidate_map(&[("[silver-record]", 3)]));
    assert_eq!(
        read_named_decision(&mut pangine, "historical-observed-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_weights(&mut pangine, "historical-derived-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "a saved decision where both views concluded violet keeps the label-free history ambiguous"
    );
    assert!(
        read_named_weights(&mut pangine, "historical-derived-answer-state").is_empty(),
        "the program does not retrieve or combine answer states after an ambiguous view mapping"
    );

    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[opal]");
    run_record_linked_choice_mapping(&mut pangine);
    assert_eq!(read_named_weights(&mut pangine, "historical-decision-record"), candidate_map(&[("[copper]", 3)]));
    assert_eq!(
        read_named_decision(&mut pangine, "historical-observed-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "historical-derived-view"), candidate_map(&[("[fern]", 1)]));

    must_ref(
        &mut pangine,
        "['latest-answer-view-conclusions'] =
           ([fern]->[topaz]->$['gold-fern-choice'])
           ([moss]->[topaz]->$['gold-moss-choice'])",
    );
    must_run(
        &mut pangine,
        "['latest-answer-view-conclusions']['selected-historical-choice'] @
           (['latest-matched-view']->[topaz]->['latest-matched-choice'])
           ([historical-choice]->['latest-matched-choice'])",
    );
    assert_eq!(
        read_named_weights(&mut pangine, "latest-matched-view"),
        candidate_map(&[("[moss]", 1)]),
        "matching copper's old violet choice against only the latest conclusions would incorrectly call it moss"
    );

    retrieve_record_linked_answer(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-derived-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) },
        "the record-linked program instead retrieves copper's preserved fern answer"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "historical-derived-answer-choice"),
        DecisionResult { candidates: candidate_map(&[("[violet]", 1)]), selected: Some("[violet]".to_owned()) }
    );

    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[garnet]");
    run_record_linked_choice_mapping(&mut pangine);
    assert_eq!(read_named_weights(&mut pangine, "historical-decision-record"), candidate_map(&[("[gold]", 3)]));
    assert_eq!(
        read_named_decision(&mut pangine, "historical-observed-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(
        read_named_weights(&mut pangine, "historical-derived-view"),
        candidate_map(&[("[moss]", 1)]),
        "the same observed violet maps to moss in the later saved decision"
    );
    retrieve_record_linked_answer(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-derived-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 3), ("[violet]", 4)]), selected: Some("[violet]".to_owned()) }
    );

    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[onyx]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[opal]");
    derive_historical_decision_record(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-decision-record"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "an unknown context does not borrow a matching result or saved decision from another experience"
    );
    assert!(
        pangine.reference_concept("['selected-historical-record'] = [historical-record]->^['historical-decision-record']").is_err(),
        "the empty route remains visible instead of invoking an application fallback"
    );
}

#[test]
#[ignore = "warning: selecting one matching decision record preserves episode amounts but discards other records, while an all-record join keeps every compatible path once and neither question order is universal"]
fn record_first_and_all_record_questions_preserve_different_parts_of_repeated_experience() {
    let mut pangine = prepare_historical_answer_record_comparison();
    for (event, record, choice, repetitions) in [
        ("multi-copper-violet", "copper", "violet", 3),
        ("multi-copper-cedar", "copper", "cedar", 1),
        ("multi-silver-violet", "silver-record", "violet", 3),
        ("multi-gold-cedar", "gold", "cedar", 4),
        ("multi-gold-violet", "gold", "violet", 1),
    ] {
        experience(&mut pangine, "historical-answer-choice-episodes", &historical_answer_choice_episode(event, "silver", record, choice, "pearl"), repetitions);
    }
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("multi-obsidian-noise", "obsidian", "copper", "violet", "pearl"),
        20,
    );
    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[pearl]");

    run_record_linked_choice_mapping(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-decision-record"),
        DecisionResult { candidates: candidate_map(&[("[copper]", 4), ("[gold]", 5), ("[silver-record]", 3)]), selected: Some("[gold]".to_owned()) },
        "the record-first program initially selects the record with the most matching episode experience"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "historical-observed-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 4), ("[violet]", 1)]), selected: Some("[cedar]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "historical-derived-view"), candidate_map(&[("[fern]", 1)]));
    retrieve_record_linked_answer(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-derived-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 4), ("[violet]", 2)]), selected: Some("[cedar]".to_owned()) }
    );

    derive_all_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-routing-episode"),
        candidate_map(&[
            ("[multi-copper-cedar]", 1),
            ("[multi-copper-violet]", 3),
            ("[multi-gold-cedar]", 4),
            ("[multi-gold-violet]", 1),
            ("[multi-silver-violet]", 3),
        ]),
        "the all-record question keeps every compatible episode and its original amount"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-derived-view"),
        candidate_map(&[("[fern]", 3), ("[moss]", 3)]),
        "the mapped view state counts each compatible saved record entry once rather than transferring episode amounts"
    );
    assert!(
        read_named_weights(&mut pangine, "all-record-derived-answer-state").is_empty(),
        "the all-record comparison stops before a spelling tie-break or answer-state combination"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("multi-copper-violet", "silver", "copper", "violet", "pearl"),
        2,
    );
    run_record_linked_choice_mapping(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-decision-record"),
        DecisionResult { candidates: candidate_map(&[("[copper]", 6), ("[gold]", 5), ("[silver-record]", 3)]), selected: Some("[copper]".to_owned()) },
        "two more matching copper experiences change the selected saved decision inside Pangine"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "historical-observed-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 5)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "historical-derived-view"), candidate_map(&[("[fern]", 1)]));
    retrieve_record_linked_answer(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-derived-answer-state"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[violet]", 2)]), selected: Some("[violet]".to_owned()) },
        "the repeated experience changes the record-first final answer from cedar to violet"
    );

    derive_all_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-routing-episode"),
        candidate_map(&[
            ("[multi-copper-cedar]", 1),
            ("[multi-copper-violet]", 5),
            ("[multi-gold-cedar]", 4),
            ("[multi-gold-violet]", 1),
            ("[multi-silver-violet]", 3),
        ]),
        "the increased amount remains visible on the original episode"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-derived-view"),
        candidate_map(&[("[fern]", 3), ("[moss]", 3)]),
        "the same all-record view projection remains tied because it preserves paths rather than their episode amounts"
    );
}

#[test]
#[ignore = "warning: ordinary staged mapping adds one mapping-source proof to each matching episode while derived view relationships remain one per distinct fact; carrying source amounts or resolving ambiguous paths would require an explicit policy"]
fn staged_record_view_mapping_does_not_transfer_episode_amounts_or_duplicate_ambiguous_experience() {
    let mut pangine = prepare_historical_answer_record_comparison();
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("carry-copper-violet", "silver", "copper", "violet", "opal"),
        3,
    );
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("carry-gold-violet", "silver", "gold", "violet", "opal"),
        1,
    );
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("carry-obsidian-noise", "obsidian", "copper", "violet", "opal"),
        20,
    );
    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[opal]");

    derive_all_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-routing-episode"),
        candidate_map(&[("[carry-copper-violet]", 3), ("[carry-gold-violet]", 1)]),
        "the direct join keeps unequal amounts on the two unambiguous episodes"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-derived-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "even without ambiguity, the views supplied by separate saved entries each appear once"
    );

    stage_and_rejoin_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "mapping-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "assigning the complete mappings and projecting them again preserves one row per path"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "rejoined-mapping-episode"),
        candidate_map(&[("[carry-copper-violet]", 4), ("[carry-gold-violet]", 2)]),
        "rejoining adds one stored mapping-row proof to each original episode amount"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "rejoined-mapping-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "the recovered amounts remain attached to episodes rather than transferring to derived views"
    );
    assert!(
        read_named_weights(&mut pangine, "rejoined-mapping-answer-state").is_empty(),
        "the staging comparison does not force the tied views into an answer-state combination"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("carry-silver-violet", "silver", "silver-record", "violet", "opal"),
        2,
    );
    stage_and_rejoin_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "rejoined-mapping-episode"),
        candidate_map(&[("[carry-copper-violet]", 4), ("[carry-gold-violet]", 2), ("[carry-silver-violet]", 3)]),
        "the mapping collection adds one proof to each episode without adding one for each ambiguous branch"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "mapping-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "the middle record supplies another compatible entry for each view, but the projected view relationships remain one fact each"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "mapping-entry"),
        candidate_map(&[("[copper-fern-entry]", 1), ("[gold-moss-entry]", 1), ("[silver-fern-entry]", 1), ("[silver-moss-entry]", 1),]),
        "the distinct compatible entries remain visible even though their repeated view values collapse"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "rejoined-mapping-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "the original-source rejoin still treats each compatible view relationship as one fact"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("carry-copper-violet", "silver", "copper", "violet", "opal"),
        2,
    );
    derive_all_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-routing-episode"),
        candidate_map(&[("[carry-copper-violet]", 5), ("[carry-gold-violet]", 1), ("[carry-silver-violet]", 2)]),
        "the untouched source still holds all five, one, and two episode experiences before another stage"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "all-record-derived-view"),
        candidate_map(&[("[fern]", 2), ("[moss]", 2)]),
        "the direct join sees two compatible paths to each view without turning the episode amounts into view amounts"
    );
    stage_and_rejoin_record_linked_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "rejoined-mapping-episode"),
        candidate_map(&[("[carry-copper-violet]", 6), ("[carry-gold-violet]", 2), ("[carry-silver-violet]", 3)]),
        "later repetition remains visible beside the one mapping-row proof after the same stage and rejoin"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "rejoined-mapping-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "ordinary staging collapses the two compatible paths per view and still does not reinterpret five episode experiences as five view relationships"
    );
}

#[test]
#[ignore = "warning: choosing the weighted observed reading before asking saved views for explanations preserves experience amounts, but this question order is only a tentative decision and not a universal correctness rule"]
fn weighted_observed_reading_can_be_tentative_while_saved_views_only_explain_it() {
    let mut pangine = prepare_historical_answer_record_comparison();
    for (event, record, reading, repetitions) in [
        ("tentative-copper-violet", "copper", "violet", 3),
        ("tentative-copper-cedar", "copper", "cedar", 1),
        ("tentative-silver-violet", "silver-record", "violet", 3),
        ("tentative-gold-cedar", "gold", "cedar", 4),
        ("tentative-gold-violet", "gold", "violet", 1),
    ] {
        experience(
            &mut pangine,
            "historical-answer-choice-episodes",
            &historical_answer_choice_episode(event, "silver", record, reading, "pearl"),
            repetitions,
        );
    }
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("tentative-obsidian-noise", "obsidian", "copper", "violet", "pearl"),
        20,
    );
    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[pearl]");

    derive_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 5), ("[violet]", 7)]), selected: Some("[violet]".to_owned()) },
        "the direct question keeps the amounts from every matching record and selects their overall reading"
    );
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-explanation-episode"),
        candidate_map(&[("[tentative-copper-violet]", 3), ("[tentative-gold-violet]", 1), ("[tentative-silver-violet]", 3)]),
        "the explanation retains only matching episodes that supplied the selected violet reading and keeps their amounts"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-explanation-entry"),
        candidate_map(&[("[copper-fern-entry]", 1), ("[gold-moss-entry]", 1), ("[silver-fern-entry]", 1), ("[silver-moss-entry]", 1),]),
        "four distinct saved entries can explain the selected reading without contributing to its prior choice"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-explanation-view"),
        candidate_map(&[("[fern]", 2), ("[moss]", 2)]),
        "the explanation may remain tied even though the tentative reading itself has a weighted winner"
    );
    assert!(
        !read_named_weights(&mut pangine, "tentative-explanation-answer-state").is_empty(),
        "the compatible saved answer states remain inspectable without selecting or combining them"
    );
    compare_tentative_reading_with_latest_views(&mut pangine);
    assert!(
        read_named_weights(&mut pangine, "tentative-latest-fern-agreement").is_empty(),
        "the tentative violet reading does not agree with the latest fern answer of cedar"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-latest-moss-agreement"),
        candidate_map(&[("[violet]", 2)]),
        "the same tentative reading agrees with the latest moss answer, with one proof supplied by each matching link"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 5), ("[violet]", 7)]), selected: Some("[violet]".to_owned()) },
        "asking for explanations and later agreement does not feed their one-each paths back into the tentative decision"
    );
    run_record_linked_choice_mapping(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "historical-decision-record"),
        DecisionResult { candidates: candidate_map(&[("[copper]", 4), ("[gold]", 5), ("[silver-record]", 3)]), selected: Some("[gold]".to_owned()) }
    );
    assert_eq!(
        read_named_decision(&mut pangine, "historical-observed-choice"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 4), ("[violet]", 1)]), selected: Some("[cedar]".to_owned()) },
        "on the same experience, selecting one record first produces cedar while the all-record tentative reading is violet"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 5), ("[violet]", 7)]), selected: Some("[violet]".to_owned()) },
        "the disagreement between question orders remains explicit rather than letting one silently overwrite the other"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("tentative-gold-cedar", "silver", "gold", "cedar", "pearl"),
        3,
    );
    derive_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 8), ("[violet]", 7)]), selected: Some("[cedar]".to_owned()) },
        "three later cedar experiences change Pangine's tentative reading without changing a saved view"
    );
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-explanation-episode"),
        candidate_map(&[("[tentative-copper-cedar]", 1), ("[tentative-gold-cedar]", 7)]),
        "the changed explanation keeps the original amounts for the episodes that now support cedar"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-explanation-entry"),
        candidate_map(&[("[copper-moss-entry]", 1), ("[gold-fern-entry]", 1)]),
        "only the saved entries whose recorded conclusions are cedar now explain the tentative reading"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-explanation-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "the explanation still does not need to choose one view"
    );
    compare_tentative_reading_with_latest_views(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-latest-fern-agreement"),
        candidate_map(&[("[cedar]", 2)]),
        "after the experience changes the tentative reading, both matching links prove agreement with the latest fern answer"
    );
    assert!(
        read_named_weights(&mut pangine, "tentative-latest-moss-agreement").is_empty(),
        "and it no longer agrees with the unchanged latest moss answer of violet"
    );
}

#[test]
#[ignore = "warning: a heavily repeated observed reading can win while no saved view explains it, so direct experience remains fallible and an empty explanation must not become an application correction"]
fn unsupported_tentative_reading_and_missing_history_remain_visible() {
    let mut pangine = prepare_historical_answer_record_comparison();
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("unsupported-amber", "silver", "copper", "amber", "cinder"),
        4,
    );
    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("supported-violet", "silver", "copper", "violet", "cinder"),
        2,
    );
    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[cinder]");

    derive_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[amber]", 4), ("[violet]", 2)]), selected: Some("[amber]".to_owned()) },
        "the direct question honors the represented experience even when the saved record cannot explain its dominant reading"
    );
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    assert!(read_named_weights(&mut pangine, "tentative-explanation-entry").is_empty());
    assert!(read_named_weights(&mut pangine, "tentative-explanation-view").is_empty());
    assert!(
        read_named_weights(&mut pangine, "tentative-explanation-answer-state").is_empty(),
        "an unsupported tentative reading remains selected but gets no invented saved-state explanation"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("supported-violet", "silver", "copper", "violet", "cinder"),
        2,
    );
    clear_percept(&mut pangine, "selected-tentative-historical-reading");
    derive_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_weights(&mut pangine, "tentative-historical-observed-reading"),
        candidate_map(&[("[amber]", 4), ("[violet]", 4)]),
        "equal experience leaves the tentative candidate state tied"
    );
    assert!(
        read_named_weights(&mut pangine, "selected-tentative-historical-reading").is_empty(),
        "the fixture does not accept the current spelling fallback as a resolved tentative reading"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("supported-violet", "silver", "copper", "violet", "cinder"),
        1,
    );
    derive_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[amber]", 4), ("[violet]", 5)]), selected: Some("[violet]".to_owned()) },
        "one more supported experience changes the tentative answer inside Pangine"
    );
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    assert_eq!(read_named_weights(&mut pangine, "tentative-explanation-entry"), candidate_map(&[("[copper-fern-entry]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "tentative-explanation-view"), candidate_map(&[("[fern]", 1)]));
    assert!(
        !read_named_weights(&mut pangine, "tentative-explanation-answer-state").is_empty(),
        "the newly dominant violet reading has one saved copper explanation"
    );

    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[onyx]");
    derive_tentative_historical_reading(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "an unfamiliar context leaves the tentative decision empty"
    );
    assert!(
        pangine.reference_concept("['selected-tentative-historical-reading'] = [tentative-reading]->^['tentative-historical-observed-reading']").is_err(),
        "an empty tentative state cannot be converted into a selected-reading link or application fallback"
    );
}

#[test]
#[ignore = "warning: an ordinary record can preserve one tentative decision and its downstream explanations after live experience changes, but the record does not make that question order or explanation a correctness rule"]
fn tentative_decision_record_preserves_old_choice_and_explanation_after_live_experience_changes() {
    let mut pangine = prepare_historical_answer_record_comparison();
    for (event, record, reading, repetitions) in [
        ("record-copper-violet", "copper", "violet", 3),
        ("record-copper-cedar", "copper", "cedar", 1),
        ("record-silver-violet", "silver-record", "violet", 3),
        ("record-gold-cedar", "gold", "cedar", 4),
        ("record-gold-violet", "gold", "violet", 1),
    ] {
        experience(
            &mut pangine,
            "historical-answer-choice-episodes",
            &historical_answer_choice_episode(event, "silver", record, reading, "pearl"),
            repetitions,
        );
    }
    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[pearl]");

    derive_tentative_historical_reading(&mut pangine);
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    materialize_tentative_decision_details(&mut pangine);
    capture_tentative_decision_record(&mut pangine, "violet-tentative-decision-record");
    extract_tentative_decision_record(&mut pangine, "violet-tentative-decision-record", "initial-record");
    assert_eq!(
        read_named_decision(&mut pangine, "initial-record-candidates"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 5), ("[violet]", 7)]), selected: Some("[violet]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "initial-record-choice"), candidate_map(&[("[violet]", 1)]));
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-explanation-episodes"),
        candidate_map(&[("[record-copper-violet]", 3), ("[record-gold-violet]", 1), ("[record-silver-violet]", 3)]),
        "the record keeps the weighted episode summary that explained the chosen violet reading"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-explanation-view-state"),
        candidate_map(&[("[fern]", 2), ("[moss]", 2)]),
        "the record keeps the original two-against-two explanation summary separately from its rows"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-agreement-state"),
        candidate_map(&[("[violet]", 2)]),
        "the two-link agreement amount is captured as its own state"
    );
    inspect_tentative_decision_record_rows(&mut pangine, "initial-record");
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-explanation-entry"),
        candidate_map(&[("[copper-fern-entry]", 1), ("[gold-moss-entry]", 1), ("[silver-fern-entry]", 1), ("[silver-moss-entry]", 1),])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-explanation-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "the four distinct stored rows survive, while repeated view values collapse to one each inside the record"
    );
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-latest-view"),
        candidate_map(&[("[fern]", 1), ("[moss]", 1)]),
        "both later views are recorded even though only one agrees"
    );
    assert_eq!(read_named_weights(&mut pangine, "initial-record-latest-reading"), candidate_map(&[("[cedar]", 1), ("[violet]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "initial-record-agreement-view"), candidate_map(&[("[moss]", 1)]));
    assert_eq!(
        read_named_weights(&mut pangine, "initial-record-agreement-reading"),
        candidate_map(&[("[violet]", 1)]),
        "the saved agreement row preserves its relationship once, while the separate agreement state preserves its amount two"
    );

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("record-gold-cedar", "silver", "gold", "cedar", "pearl"),
        3,
    );
    derive_tentative_historical_reading(&mut pangine);
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    materialize_tentative_decision_details(&mut pangine);
    capture_tentative_decision_record(&mut pangine, "cedar-tentative-decision-record");
    assert_eq!(
        read_named_decision(&mut pangine, "tentative-historical-observed-reading"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 8), ("[violet]", 7)]), selected: Some("[cedar]".to_owned()) },
        "new live experience changes the current tentative decision before the old record is replayed"
    );

    for percept in [
        "historical-answer-records",
        "historical-answer-choice-episodes",
        "historical-answer-context",
        "requested-historical-result",
        "tentative-historical-observed-reading",
        "selected-tentative-historical-reading",
        "tentative-selected-reading-value",
        "tentative-explanation-episode",
        "tentative-explanation-entry",
        "tentative-explanation-view",
        "tentative-explanation-answer-state",
        "recordable-agreement-reading",
        "tentative-record-explanation-rows",
        "tentative-latest-view-reading-rows",
        "tentative-latest-agreement-rows",
        "gold-fern-choice",
        "gold-moss-choice",
    ] {
        clear_percept(&mut pangine, percept);
    }

    extract_tentative_decision_record(&mut pangine, "violet-tentative-decision-record", "replayed-violet-record");
    inspect_tentative_decision_record_rows(&mut pangine, "replayed-violet-record");
    assert_eq!(
        read_named_decision(&mut pangine, "replayed-violet-record-candidates"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 5), ("[violet]", 7)]), selected: Some("[violet]".to_owned()) },
        "the old record replays the old weighted candidate state after every live input is cleared"
    );
    let expected_context = must_ref(&mut pangine, "[request]->[cobalt]->[silver]");
    let expected_result = must_ref(&mut pangine, "[request]->[saffron]->[pearl]");
    assert_eq!(must_ref(&mut pangine, "$['replayed-violet-record-context']"), expected_context);
    assert_eq!(must_ref(&mut pangine, "$['replayed-violet-record-requested-result']"), expected_result);
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-choice"), candidate_map(&[("[violet]", 1)]));
    assert_eq!(
        read_named_weights(&mut pangine, "replayed-violet-record-explanation-episodes"),
        candidate_map(&[("[record-copper-violet]", 3), ("[record-gold-violet]", 1), ("[record-silver-violet]", 3)])
    );
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-explanation-view-state"), candidate_map(&[("[fern]", 2), ("[moss]", 2)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-agreement-state"), candidate_map(&[("[violet]", 2)]));
    assert_eq!(
        read_named_weights(&mut pangine, "replayed-violet-record-explanation-entry"),
        candidate_map(&[("[copper-fern-entry]", 1), ("[gold-moss-entry]", 1), ("[silver-fern-entry]", 1), ("[silver-moss-entry]", 1),])
    );
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-latest-view"), candidate_map(&[("[fern]", 1), ("[moss]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-latest-reading"), candidate_map(&[("[cedar]", 1), ("[violet]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-agreement-view"), candidate_map(&[("[moss]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-violet-record-agreement-reading"), candidate_map(&[("[violet]", 1)]));

    extract_tentative_decision_record(&mut pangine, "cedar-tentative-decision-record", "replayed-cedar-record");
    inspect_tentative_decision_record_rows(&mut pangine, "replayed-cedar-record");
    assert_eq!(
        read_named_decision(&mut pangine, "replayed-cedar-record-candidates"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 8), ("[violet]", 7)]), selected: Some("[cedar]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-choice"), candidate_map(&[("[cedar]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-explanation-view-state"), candidate_map(&[("[fern]", 1), ("[moss]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-agreement-state"), candidate_map(&[("[cedar]", 2)]));
    assert_eq!(
        read_named_weights(&mut pangine, "replayed-cedar-record-explanation-episodes"),
        candidate_map(&[("[record-copper-cedar]", 1), ("[record-gold-cedar]", 7)])
    );
    assert_eq!(
        read_named_weights(&mut pangine, "replayed-cedar-record-explanation-entry"),
        candidate_map(&[("[copper-moss-entry]", 1), ("[gold-fern-entry]", 1)])
    );
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-latest-view"), candidate_map(&[("[fern]", 1), ("[moss]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-latest-reading"), candidate_map(&[("[cedar]", 1), ("[violet]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-agreement-view"), candidate_map(&[("[fern]", 1)]));
    assert_eq!(read_named_weights(&mut pangine, "replayed-cedar-record-agreement-reading"), candidate_map(&[("[cedar]", 1)]));
}

#[test]
#[ignore = "warning: a grouped inventory preserves recorded choice-agreement stories even for split descendant clauses, while explicitly flattening the records permits cross-mixing; this does not choose a record or make the grouped shape permanent"]
fn grouped_record_inventory_preserves_choice_agreement_pairs_until_records_are_flattened() {
    let mut pangine = prepare_historical_answer_record_comparison();
    for (event, record, reading, repetitions) in [
        ("record-copper-violet", "copper", "violet", 3),
        ("record-copper-cedar", "copper", "cedar", 1),
        ("record-silver-violet", "silver-record", "violet", 3),
        ("record-gold-cedar", "gold", "cedar", 4),
        ("record-gold-violet", "gold", "violet", 1),
    ] {
        experience(
            &mut pangine,
            "historical-answer-choice-episodes",
            &historical_answer_choice_episode(event, "silver", record, reading, "pearl"),
            repetitions,
        );
    }
    replace_state(&mut pangine, "historical-answer-context", "[request]->[cobalt]->[silver]");
    replace_state(&mut pangine, "requested-historical-result", "[request]->[saffron]->[pearl]");

    derive_tentative_historical_reading(&mut pangine);
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    materialize_tentative_decision_details(&mut pangine);
    capture_tentative_decision_record(&mut pangine, "violet-tentative-decision-record");

    experience(
        &mut pangine,
        "historical-answer-choice-episodes",
        &historical_answer_choice_episode("record-gold-cedar", "silver", "gold", "cedar", "pearl"),
        3,
    );
    derive_tentative_historical_reading(&mut pangine);
    select_tentative_historical_reading(&mut pangine);
    explain_tentative_historical_reading(&mut pangine);
    materialize_tentative_decision_details(&mut pangine);
    capture_tentative_decision_record(&mut pangine, "cedar-tentative-decision-record");

    experience(&mut pangine, "tentative-decision-record-inventory", "['violet-tentative-decision-record']", 1);
    experience(&mut pangine, "tentative-decision-record-inventory", "['cedar-tentative-decision-record']", 1);
    let inventory = must_ref(&mut pangine, "$['tentative-decision-record-inventory']");

    let whole_question = must_ref(
        &mut pangine,
        "(
           ([tentative-decision]->[choice]->['whole-record-reading'])
           ([tentative-decision]->[agreements]->
             (([tentative-reading]->['whole-record-reading'])
              (['whole-record-view']->[latest-reading]->['whole-record-reading'])))
         )",
    );
    let whole_reading = pangine.reference_percept("whole-record-reading");
    let whole_view = pangine.reference_percept("whole-record-view");
    let whole = pangine.complete_subject(&inventory, &whole_question).expect("valid whole-record inventory question");
    assert_eq!(
        binding_pairs(&pangine, whole.completions(), &whole_reading, &whole_view),
        BTreeSet::from([("[cedar]".to_owned(), "[fern]".to_owned()), ("[violet]".to_owned(), "[moss]".to_owned())]),
        "asking for the complete choice-and-agreement shape keeps each recorded reading with its own agreeing view"
    );

    let split_question = must_ref(
        &mut pangine,
        "([tentative-decision]->[choice]->['split-record-reading'])
         ([tentative-reading]->['split-agreement-reading'])
         (['split-record-view']->[latest-reading]->['split-agreement-reading'])",
    );
    let split_reading = pangine.reference_percept("split-record-reading");
    let split_view = pangine.reference_percept("split-record-view");
    let split = pangine.complete_subject(&inventory, &split_question).expect("valid split record-inventory question");
    assert_eq!(
        binding_pairs(&pangine, split.completions(), &split_reading, &split_view),
        BTreeSet::from([("[cedar]".to_owned(), "[fern]".to_owned()), ("[violet]".to_owned(), "[moss]".to_owned())]),
        "the grouped inventory keeps each split descendant match committed to the same complete record member"
    );

    must_run(&mut pangine, "['flattened-tentative-decision-records'] = $['violet-tentative-decision-record'] * $['cedar-tentative-decision-record']");
    let flattened = must_ref(&mut pangine, "$['flattened-tentative-decision-records']");
    let flattened_split = pangine.complete_subject(&flattened, &split_question).expect("valid explicitly flattened record question");
    assert_eq!(
        binding_pairs(&pangine, flattened_split.completions(), &split_reading, &split_view),
        BTreeSet::from([
            ("[cedar]".to_owned(), "[fern]".to_owned()),
            ("[cedar]".to_owned(), "[moss]".to_owned()),
            ("[violet]".to_owned(), "[fern]".to_owned()),
            ("[violet]".to_owned(), "[moss]".to_owned()),
        ]),
        "explicitly merging both record payloads removes their member boundary and admits both false cross-record pairs"
    );
}

#[test]
#[ignore = "warning: represented memory identity can keep two populated report sources separate, but source-reference choice remains provisional"]
fn represented_source_choice_switches_between_two_populated_disjoint_report_memories() {
    let mut pangine = Pangine::new();
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-a", "alpha-report-a", "alpha", "A", 3);
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-a", "alpha-report-b", "alpha", "B", 1);
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-b", "beta-report-a", "beta", "A", 1);
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-b", "beta-report-b", "beta", "B", 4);
    remember_source_scoped_origin(&mut pangine, "alpha-origin-row-a", "alpha", "alpha-report-a", "alpha-origin-a");
    remember_source_scoped_origin(&mut pangine, "alpha-origin-row-b", "alpha", "alpha-report-b", "alpha-origin-b");
    remember_source_scoped_origin(&mut pangine, "beta-origin-row-a", "beta", "beta-report-a", "beta-origin-a");
    remember_source_scoped_origin(&mut pangine, "beta-origin-row-b", "beta", "beta-report-b", "beta-origin-b");
    materialize_source_scoped_origin_rows(&mut pangine);

    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-a']", 2);
    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-b']", 1);
    assert_eq!(
        read_named_weights(&mut pangine, "source-scoped-report-memory-choice"),
        candidate_map(&[("['source-scoped-report-memory-a']", 2), ("['source-scoped-report-memory-b']", 1)])
    );
    assert_eq!(must_ref(&mut pangine, "^['source-scoped-report-memory-choice']"), pangine.reference_percept("source-scoped-report-memory-a"));
    run_selected_source_scoped_report_question(&mut pangine, "disjoint-alpha", "source-scoped-origin-rows", true);
    assert_eq!(
        read_named_decision(&mut pangine, "disjoint-alpha-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 1)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(
        read_named_weights(&mut pangine, "disjoint-alpha-origin"),
        candidate_map(&[("[alpha-origin-a]", 1), ("[alpha-origin-b]", 1)]),
        "the reusable rows retain origin identities once while the selected report memory supplies answer amounts"
    );

    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-b']", 2);
    assert_eq!(
        read_named_weights(&mut pangine, "source-scoped-report-memory-choice"),
        candidate_map(&[("['source-scoped-report-memory-a']", 2), ("['source-scoped-report-memory-b']", 3)])
    );
    assert_eq!(must_ref(&mut pangine, "^['source-scoped-report-memory-choice']"), pangine.reference_percept("source-scoped-report-memory-b"));
    run_selected_source_scoped_report_question(&mut pangine, "disjoint-beta", "source-scoped-origin-rows", true);
    assert_eq!(
        read_named_decision(&mut pangine, "disjoint-beta-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "disjoint-beta-origin"), candidate_map(&[("[beta-origin-a]", 1), ("[beta-origin-b]", 1)]));
}

#[test]
#[ignore = "warning: projected memory identity prevents overlapping report names from leaking only because this question requires it explicitly"]
fn projected_store_identity_prevents_overlapping_report_names_from_leaking_across_memories() {
    let mut pangine = Pangine::new();
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-a", "shared-report", "alpha", "A", 3);
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-b", "shared-report", "beta", "B", 4);
    remember_source_scoped_origin(&mut pangine, "alpha-shared-origin-row", "alpha", "shared-report", "alpha-origin");
    remember_source_scoped_origin(&mut pangine, "beta-shared-origin-row", "beta", "shared-report", "beta-origin");
    materialize_source_scoped_origin_rows(&mut pangine);

    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-a']", 2);
    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-b']", 1);
    run_selected_source_scoped_report_question(&mut pangine, "overlap-alpha-full-row", "source-scoped-origin-stage-rows", true);
    assert_eq!(
        read_named_weights(&mut pangine, "overlap-alpha-full-row-origin"),
        candidate_map(&[("[alpha-origin]", 1), ("[beta-origin]", 1)]),
        "a full stored row can satisfy its own report-store clause and leak across an overlapping report identity"
    );

    run_selected_source_scoped_report_question(&mut pangine, "overlap-alpha-scoped", "source-scoped-origin-rows", true);
    assert_eq!(
        read_named_decision(&mut pangine, "overlap-alpha-scoped-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "overlap-alpha-scoped-origin"), candidate_map(&[("[alpha-origin]", 1)]));

    run_selected_source_scoped_report_question(&mut pangine, "overlap-alpha-unscoped", "source-scoped-origin-rows", false);
    assert_eq!(read_named_weights(&mut pangine, "overlap-alpha-unscoped-answer"), candidate_map(&[("[A]", 3)]));
    assert_eq!(
        read_named_weights(&mut pangine, "overlap-alpha-unscoped-origin"),
        candidate_map(&[("[alpha-origin]", 1), ("[beta-origin]", 1)]),
        "without one shared memory binding the identical report name again admits both origins"
    );

    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-b']", 2);
    run_selected_source_scoped_report_question(&mut pangine, "overlap-beta-scoped", "source-scoped-origin-rows", true);
    assert_eq!(
        read_named_decision(&mut pangine, "overlap-beta-scoped-answer"),
        DecisionResult { candidates: candidate_map(&[("[B]", 4)]), selected: Some("[B]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "overlap-beta-scoped-origin"), candidate_map(&[("[beta-origin]", 1)]));
}

#[test]
#[ignore = "warning: represented inventory traversal preserves each live source history but currently treats repeated references as membership when its rows feed the next question"]
fn represented_memory_inventory_preserves_each_live_report_history_without_cross_leakage() {
    let mut pangine = Pangine::new();
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-a", "shared-report", "alpha", "A", 3);
    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-b", "shared-report", "beta", "B", 4);
    remember_source_scoped_origin(&mut pangine, "alpha-shared-origin-row", "alpha", "shared-report", "alpha-origin");
    remember_source_scoped_origin(&mut pangine, "beta-shared-origin-row", "beta", "shared-report", "beta-origin");
    materialize_source_scoped_origin_rows(&mut pangine);

    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-a']", 20);
    experience(&mut pangine, "source-scoped-report-memory-choice", "['source-scoped-report-memory-b']", 1);
    run_selected_source_scoped_report_question(&mut pangine, "choose-one-memory", "source-scoped-origin-rows", true);
    assert_eq!(
        read_named_decision(&mut pangine, "choose-one-memory-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(read_named_weights(&mut pangine, "choose-one-memory-origin"), candidate_map(&[("[alpha-origin]", 1)]));

    run_source_scoped_report_question(
        &mut pangine,
        "simultaneous-memories",
        "['source-scoped-report-memory-a']['source-scoped-report-memory-b']['source-scoped-origin-rows']",
        true,
    );
    assert_eq!(
        read_named_decision(&mut pangine, "simultaneous-memories-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) },
        "when both memories are sources their original report experience decides without inheriting the separate 20-to-1 reference amounts"
    );
    assert_eq!(read_named_weights(&mut pangine, "simultaneous-memories-origin"), candidate_map(&[("[alpha-origin]", 1), ("[beta-origin]", 1)]));

    run_source_scoped_report_question(
        &mut pangine,
        "represented-memory-inventory",
        "(['source-scoped-report-memory-choice'] @ ['listed-report-memory'])*['source-scoped-origin-rows']",
        true,
    );
    assert_eq!(
        read_named_weights(&mut pangine, "listed-report-memory"),
        candidate_map(&[("['source-scoped-report-memory-a']", 20), ("['source-scoped-report-memory-b']", 1)]),
        "the inventory question keeps the experience attached to each represented reference"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-memory-inventory-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 4)]), selected: Some("[B]".to_owned()) },
        "the inventory rows let the next question use every referenced live memory as a separate source without flattening its report history"
    );
    assert_eq!(read_named_weights(&mut pangine, "represented-memory-inventory-origin"), candidate_map(&[("[alpha-origin]", 1), ("[beta-origin]", 1)]));

    remember_source_scoped_report(&mut pangine, "source-scoped-report-memory-a", "shared-report", "alpha", "A", 2);
    run_source_scoped_report_question(
        &mut pangine,
        "represented-memory-inventory-later",
        "(['source-scoped-report-memory-choice'] @ ['listed-report-memory-later'])*['source-scoped-origin-rows']",
        true,
    );
    assert_eq!(
        read_named_decision(&mut pangine, "represented-memory-inventory-later-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5), ("[B]", 4)]), selected: Some("[A]".to_owned()) },
        "later report experience remains live through the represented inventory and changes Pangine's all-memory answer"
    );
    assert_eq!(read_named_weights(&mut pangine, "represented-memory-inventory-later-origin"), candidate_map(&[("[alpha-origin]", 1), ("[beta-origin]", 1)]));

    run_source_scoped_report_question(&mut pangine, "represented-memory-holder", "['source-scoped-report-memory-choice']['source-scoped-origin-rows']", true);
    assert_eq!(
        read_named_decision(&mut pangine, "represented-memory-holder-answer"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "the represented collection records references but does not silently turn every reference into a source"
    );
    assert_eq!(read_named_weights(&mut pangine, "represented-memory-holder-origin"), BTreeMap::new());

    must_run(&mut pangine, "($['source-scoped-report-memory-choice']) @ {['evaluated-memory-report']->[reported-answer]->['evaluated-memory-answer']}");
    assert_eq!(
        read_named_decision(&mut pangine, "evaluated-memory-answer"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 1)]), selected: Some("[A]".to_owned()) },
        "recursive evaluation sees both answers but flattens both the 20-to-1 reference experience and the original 3-to-4 report experience"
    );

    run_source_scoped_report_question(
        &mut pangine,
        "evaluated-memory-holder",
        "($['source-scoped-report-memory-choice'])($['source-scoped-origin-rows'])",
        true,
    );
    assert_eq!(
        read_named_decision(&mut pangine, "evaluated-memory-holder-answer"),
        DecisionResult { candidates: BTreeMap::new(), selected: None },
        "evaluating both collections as combined values does not preserve their separate source histories for the rejoin"
    );
    assert_eq!(read_named_weights(&mut pangine, "evaluated-memory-holder-origin"), BTreeMap::new());
}

#[test]
#[ignore = "warning: grounded event rows retain distinct identities but one assigned row collection is not one support source per event"]
fn assigned_event_view_does_not_automatically_count_each_distinct_event() {
    let mut pangine = Pangine::new();
    populate_source_event_reports(&mut pangine);

    let report_question = "(['stage-report']->[amber]->['stage-source'])(['stage-report']->[topaz]->['stage-condition'])([lantern]->[cobalt]->['stage-condition'])(['stage-report']->[indigo]->['stage-event'])(['stage-event']->[saffron]->['stage-choice'])";
    must_ref(&mut pangine, &format!("['eligible-report-rows'] = (['reports']['condition'] @ {report_question})"));
    assert_eq!(
        read_named_decision(&mut pangine, "stage-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    let event_question = "(['event-view-event']->[saffron]->['event-view-choice'])";
    must_ref(&mut pangine, &format!("['event-rows'] = (['eligible-report-rows'] @ {event_question})"));
    assert_eq!(
        read_named_weights(&mut pangine, "event-view-event"),
        candidate_map(&[("[distinct-b-event-one]", 1), ("[distinct-b-event-three]", 1), ("[distinct-b-event-two]", 1), ("[shared-a-event]", 1),]),
        "the grounded event stage exposes each event identity once"
    );
    assert_eq!(
        read_named_decision(&mut pangine, "event-view-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 1)]), selected: Some("[A]".to_owned()) },
        "one assigned report-row collection is one source witness per candidate, not one source per distinct event"
    );

    let event_rows = pangine.reference_percept("event-rows");
    let distinct_event = pangine.reference_percept("distinct-event");
    let distinct_choice = pangine.reference_percept("distinct-choice");
    let question = must_ref(&mut pangine, "(['distinct-event']->[saffron]->['distinct-choice'])");
    let result = pangine.complete_question(std::slice::from_ref(&event_rows), &question).expect("valid assigned event-row question");
    let event_choices = result
        .completions()
        .iter()
        .map(|completion| {
            (
                pangine.format_concept(completion.binding(&distinct_event).expect("bound distinct event"), false),
                pangine.format_concept(completion.binding(&distinct_choice).expect("bound distinct-event choice"), false),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        event_choices,
        BTreeSet::from([
            ("[distinct-b-event-one]".to_owned(), "[B]".to_owned()),
            ("[distinct-b-event-three]".to_owned(), "[B]".to_owned()),
            ("[distinct-b-event-two]".to_owned(), "[B]".to_owned()),
            ("[shared-a-event]".to_owned(), "[A]".to_owned()),
        ]),
        "the second assigned view keeps all four event-to-outcome pairings"
    );

    let repeated_event = pangine.reference_percept("repeated-event");
    let repeated_choice = pangine.reference_percept("repeated-choice");
    let repeated_question = must_ref(&mut pangine, "x3(['repeated-event']->[saffron]->['repeated-choice'])");
    let repeated_result = pangine.complete_question(std::slice::from_ref(&event_rows), &repeated_question).expect("valid repeated-event shape question");
    let [repeated_completion] = repeated_result.completions() else {
        panic!("the event rows should retain one relationship grounded from three reports");
    };
    assert_eq!(pangine.format_concept(repeated_completion.binding(&repeated_event).expect("bound repeated event"), false), "[shared-a-event]");
    assert_eq!(pangine.format_concept(repeated_completion.binding(&repeated_choice).expect("bound repeated choice"), false), "[A]");

    pangine.reference_concept("['event-rows'] @ (['distinct-event']->[saffron]->['distinct-choice'])").expect("valid materialized assigned event-row question");
    assert_eq!(
        read_named_weights(&mut pangine, "distinct-event"),
        candidate_map(&[("[distinct-b-event-one]", 1), ("[distinct-b-event-three]", 1), ("[distinct-b-event-two]", 1), ("[shared-a-event]", 1),])
    );
    assert_eq!(
        read_named_decision(&mut pangine, "distinct-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 1)]), selected: Some("[A]".to_owned()) },
        "the four retained event rows do not become four independent direct sources after assignment"
    );
}

#[test]
#[ignore = "warning: experience-shaped additive choice remains provisional across question shapes"]
fn experience_shaped_choice_survives_opaque_renaming_and_a_direct_question() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive-one", "[mark]->[cedar]", 2);
    experience(&mut pangine, "archive-two", "[mark]->[quartz]", 1);
    pangine.reference_percept("archive-three");
    pangine.reference_percept("archive-four");

    ask_direct_decision(&mut pangine);
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 2), ("[quartz]", 1)]), selected: Some("[cedar]".to_owned()) }
    );

    experience(&mut pangine, "archive-three", "[mark]->[quartz]", 1);
    experience(&mut pangine, "archive-four", "[mark]->[quartz]", 1);

    ask_direct_decision(&mut pangine);
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 2), ("[quartz]", 3)]), selected: Some("[quartz]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: repeated experience and an explicit coefficient boundary are distinct current decision inputs"]
fn explicit_coefficient_history_does_not_currently_equal_repeated_experience() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "[mark]->[A]", 2);
    experience(&mut pangine, "memory", "x2([mark]->[B])", 1);

    let memory = pangine.reference_percept("memory");
    let choice = pangine.reference_percept("choice");
    let question = must_ref(&mut pangine, "[mark]->['choice']");
    let b = must_ref(&mut pangine, "[B]");
    let weighted = must_ref(&mut pangine, "x2([mark]->[B])");
    let result = pangine.complete_question(std::slice::from_ref(&memory), &question).expect("valid coefficient-history question");
    let b_completion = result.completions().iter().find(|completion| completion.binding(&choice) == Some(&b)).expect("B completion");
    let [evidence] = b_completion.evidence() else {
        panic!("the direct B answer should have one evidence fragment");
    };
    assert_eq!(evidence.source_relevance(), Relevance::DEFAULT);
    assert_eq!(evidence.coefficient_ancestors().collect::<Vec<_>>(), vec![&weighted]);

    pangine.reference_concept("['memory'] @ [mark]->['choice']").expect("valid materialized coefficient-history question");
    assert_eq!(read_decision(&mut pangine), DecisionResult { candidates: candidate_map(&[("[A]", 2), ("[B]", 1)]), selected: Some("[A]".to_owned()) });
}

#[test]
#[ignore = "warning: equal support currently hides distinguishable experience histories from decision"]
fn equal_current_totals_retain_different_experience_histories_until_projection() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "body", &decision_experience("event-repeat", "A"), 2);
    experience(&mut pangine, "body", &decision_experience("event-distinct-one", "B"), 1);
    experience(&mut pangine, "body", &decision_experience("event-distinct-two", "B"), 1);
    let weighted_experience = format!("x2({})", decision_experience("event-weighted", "C"));
    experience(&mut pangine, "body", &weighted_experience, 2);

    let body = pangine.reference_percept("body");
    let event = pangine.reference_percept("event");
    let choice = pangine.reference_percept("choice");
    let weighted_experience = must_ref(&mut pangine, &weighted_experience);
    let question_text = "(['event']->[signal]->[mark])(['event']->[answer]->['choice'])";
    let question = must_ref(&mut pangine, question_text);
    let result = pangine.complete_question(std::slice::from_ref(&body), &question).expect("valid combined-history question");

    let histories = result
        .completions()
        .iter()
        .map(|completion| {
            let event = pangine.format_concept(completion.binding(&event).expect("bound event"), false);
            let evidence = completion.evidence().iter().find(|evidence| evidence.binding(&choice).is_some()).expect("answer evidence");
            let coefficient_ancestors = evidence.coefficient_ancestors().collect::<BTreeSet<_>>();
            assert!(coefficient_ancestors.is_empty() || coefficient_ancestors == BTreeSet::from([&weighted_experience]));
            (
                event,
                ExperienceHistory {
                    candidate: pangine.format_concept(completion.binding(&choice).expect("bound choice"), false),
                    source_relevance: evidence.source_relevance(),
                    coefficient_bearing: !coefficient_ancestors.is_empty(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        histories,
        BTreeMap::from([
            (
                "[event-distinct-one]".to_owned(),
                ExperienceHistory { candidate: "[B]".to_owned(), source_relevance: Relevance::DEFAULT, coefficient_bearing: false },
            ),
            (
                "[event-distinct-two]".to_owned(),
                ExperienceHistory { candidate: "[B]".to_owned(), source_relevance: Relevance::DEFAULT, coefficient_bearing: false },
            ),
            ("[event-repeat]".to_owned(), ExperienceHistory { candidate: "[A]".to_owned(), source_relevance: Relevance::new(2), coefficient_bearing: false },),
            ("[event-weighted]".to_owned(), ExperienceHistory { candidate: "[C]".to_owned(), source_relevance: Relevance::new(2), coefficient_bearing: true },),
        ])
    );

    let rows = pangine
        .reference_concept(&format!("['body'] @ {question_text}"))
        .expect("valid materialized combined-history question")
        .expect("combined-history rows");
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[A]", 2), ("[B]", 2), ("[C]", 2)]), selected: Some("[A]".to_owned()) }
    );

    let projected_choice = pangine.reference_percept("projected-choice");
    let projected_weight_question = must_ref(&mut pangine, "x2['projected-choice']");
    let projected_weight_result =
        pangine.complete_question(std::slice::from_ref(&choice), &projected_weight_question).expect("valid projected-weight question");
    assert_eq!(
        projected_weight_result
            .completions()
            .iter()
            .map(|completion| pangine.format_concept(completion.binding(&projected_choice).expect("bound projected choice"), false))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["[A]".to_owned(), "[B]".to_owned(), "[C]".to_owned()])
    );
    assert!(projected_weight_result.completions().iter().all(|completion| {
        let [evidence] = completion.evidence() else {
            return false;
        };
        evidence.source_relevance() == Relevance::DEFAULT && evidence.coefficient_ancestors().next().is_none()
    }));
    pangine.reference_concept("['choice'] @ x2['projected-choice']").expect("valid materialized projected-weight question");
    assert_eq!(
        read_named_decision(&mut pangine, "projected-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 1), ("[C]", 1)]), selected: Some("[A]".to_owned()) }
    );

    let recalled = pangine.reference_percept("recalled");
    assert!(pangine.set_percept_value(&recalled, Some(rows)));
    let recalled_choice = pangine.reference_percept("recalled-choice");
    let recalled_question_text = "(['recalled-event']->[signal]->[mark])(['recalled-event']->[answer]->['recalled-choice'])";
    let recalled_question = must_ref(&mut pangine, recalled_question_text);
    let recalled_result = pangine.complete_question(std::slice::from_ref(&recalled), &recalled_question).expect("valid recalled-history question");
    assert_eq!(recalled_result.completions().len(), 4, "the four correlated answer rows survive");
    for completion in recalled_result.completions() {
        let evidence = completion.evidence().iter().find(|evidence| evidence.binding(&recalled_choice).is_some()).expect("recalled answer evidence");
        assert_eq!(evidence.source_relevance(), Relevance::DEFAULT);
        assert!(evidence.coefficient_ancestors().next().is_none());
    }

    pangine.reference_concept(&format!("['recalled'] @ {recalled_question_text}")).expect("valid materialized recalled-history question");
    assert_eq!(
        read_named_decision(&mut pangine, "recalled-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 1), ("[C]", 1)]), selected: Some("[A]".to_owned()) }
    );

    let missing_weight_question = must_ref(&mut pangine, "x2['missing-weight-choice']");
    assert!(pangine
        .complete_question(std::slice::from_ref(&recalled_choice), &missing_weight_question)
        .expect("valid recalled-weight question")
        .completions()
        .is_empty());
}

#[test]
#[ignore = "warning: exact coefficient-pattern matching is a provisional Pangine-native history operation"]
fn exact_coefficient_pattern_can_bind_structure_without_becoming_experience_strength() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "body", "[mark]->[A]", 2);
    let weighted_experience = "x2([mark]->[C])";
    experience(&mut pangine, "body", weighted_experience, 2);

    let body = pangine.reference_percept("body");
    let weighted_experience = must_ref(&mut pangine, weighted_experience);
    let whole = pangine.reference_percept("whole");
    let whole_result = pangine.complete_question(std::slice::from_ref(&body), &whole).expect("valid whole-value question");
    let weighted_completion = whole_result
        .completions()
        .iter()
        .find(|completion| completion.binding(&whole) == Some(&weighted_experience))
        .expect("the complete coefficient-bearing value remains available");
    let [weighted_evidence] = weighted_completion.evidence() else {
        panic!("the complete coefficient-bearing value should have one source proof");
    };
    assert_eq!(weighted_evidence.source_relevance(), Relevance::new(2));
    assert!(weighted_evidence.coefficient_ancestors().next().is_none(), "binding the complete wrapper crosses no coefficient boundary");

    let question_text = "x2([mark]->['choice'])";
    let question = must_ref(&mut pangine, question_text);
    let result = pangine.complete_question(std::slice::from_ref(&body), &question).expect("valid coefficient-specific question");
    let [completion] = result.completions() else {
        panic!("only the coefficient-bearing source should answer the exact coefficient pattern");
    };
    let choice = pangine.reference_percept("choice");
    assert_eq!(pangine.format_concept(completion.binding(&choice).expect("bound choice"), false), "[C]");
    let [evidence] = completion.evidence() else {
        panic!("the exact coefficient pattern should have one source proof");
    };
    assert_eq!(evidence.source_concept(), &weighted_experience);
    assert_eq!(evidence.source_relevance(), Relevance::new(2));
    assert!(evidence.coefficient_ancestors().next().is_none(), "matching the complete requested wrapper crosses no coefficient boundary");

    let other_question = must_ref(&mut pangine, "x3([mark]->['other-choice'])");
    assert!(pangine.complete_question(std::slice::from_ref(&body), &other_question).expect("valid different-coefficient question").completions().is_empty());

    pangine.reference_concept(&format!("['body'] @ {question_text}")).expect("valid materialized coefficient-specific question");
    assert_eq!(read_decision(&mut pangine), DecisionResult { candidates: candidate_map(&[("[C]", 2)]), selected: Some("[C]".to_owned()) });
}

#[test]
#[ignore = "warning: structural inversion is not negative answer support in the current question projection"]
fn structural_inversion_does_not_automatically_become_counterevidence() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "body", "[support-event]->[vote]->[A]", 2);
    let inverted_experience = "!([inverted-event]->[vote]->[A])";
    experience(&mut pangine, "body", inverted_experience, 5);

    let body = pangine.reference_percept("body");
    let choice = pangine.reference_percept("choice");
    let inverted_experience = must_ref(&mut pangine, inverted_experience);
    let ordinary_question = must_ref(&mut pangine, "['event']->[vote]->['choice']");
    let result = pangine.complete_question(std::slice::from_ref(&body), &ordinary_question).expect("valid ordinary inversion probe");
    let inverted_completion = result
        .completions()
        .iter()
        .find(|completion| {
            completion.evidence().iter().any(|evidence| evidence.source_concept() == &inverted_experience && evidence.binding(&choice).is_some())
        })
        .expect("the ordinary question should inspect the inverted operand");
    let inverted_evidence = inverted_completion
        .evidence()
        .iter()
        .find(|evidence| evidence.source_concept() == &inverted_experience && evidence.binding(&choice).is_some())
        .expect("inverted answer evidence");
    assert_eq!(inverted_evidence.source_relevance(), Relevance::new(5));
    assert_eq!(inverted_evidence.coefficient_ancestors().collect::<BTreeSet<_>>(), BTreeSet::from([&inverted_experience]));

    pangine.reference_concept("['body'] @ ['event']->[vote]->['choice']").expect("valid materialized ordinary inversion probe");
    assert_eq!(read_decision(&mut pangine), DecisionResult { candidates: candidate_map(&[("[A]", 7)]), selected: Some("[A]".to_owned()) });

    pangine.reference_concept("['body'] @ !(['inverted-event']->[vote]->['inverted-choice'])").expect("valid exact inverted question");
    assert_eq!(
        read_named_decision(&mut pangine, "inverted-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 5)]), selected: Some("[A]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: explicit support-minus-counterevidence is a promising Pangine program, not a settled decision rule"]
fn separate_support_and_counter_questions_can_be_combined_inside_pangine() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "body", "[support-a]->[supports]->[A]", 4);
    experience(&mut pangine, "body", "[support-b]->[supports]->[B]", 3);

    let support_question = "['body'] @ ['support-event']->[supports]->['support-choice']";
    let counter_question = "['body'] @ ['counter-event']->[opposes]->['counter-choice']";
    pangine.reference_concept(support_question).expect("valid represented support question");
    pangine.reference_concept(counter_question).expect("valid empty represented counter question");
    assert_eq!(
        read_named_decision(&mut pangine, "support-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );
    assert_eq!(read_named_decision(&mut pangine, "counter-choice"), DecisionResult { candidates: BTreeMap::new(), selected: None });

    must_ref(&mut pangine, "['net-choice'] = $['support-choice']");
    must_ref(&mut pangine, "['net-choice'] /= $['counter-choice']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut pangine, "body", "[counter-a]->[opposes]->[A]", 3);
    pangine.reference_concept(counter_question).expect("valid represented counter question");
    assert_eq!(
        read_named_decision(&mut pangine, "counter-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3)]), selected: Some("[A]".to_owned()) }
    );
    must_ref(&mut pangine, "['net-choice'] = $['support-choice']");
    must_ref(&mut pangine, "['net-choice'] /= $['counter-choice']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "body", "[counter-b]->[opposes]->[B]", 4);
    pangine.reference_concept(counter_question).expect("valid updated represented counter question");
    must_ref(&mut pangine, "['net-choice'] = $['support-choice']");
    must_ref(&mut pangine, "['net-choice'] /= $['counter-choice']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", -1)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut pangine, "body", "[counter-a]->[opposes]->[A]", 2);
    pangine.reference_concept(counter_question).expect("valid inhibiting represented counter question");
    must_ref(&mut pangine, "['net-choice'] = $['support-choice']");
    must_ref(&mut pangine, "['net-choice'] /= $['counter-choice']");
    assert_eq!(read_named_decision(&mut pangine, "net-choice"), DecisionResult { candidates: candidate_map(&[("[A]", -1), ("[B]", -1)]), selected: None });
}

#[test]
#[ignore = "warning: opaque relation names confirm that the explicit Pangine program supplies the counterevidence role"]
fn counterevidence_program_survives_opaque_relation_names() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive", "[rune-one]->[amber]->[cedar]", 4);
    experience(&mut pangine, "archive", "[rune-two]->[amber]->[quartz]", 3);
    experience(&mut pangine, "archive", "[rune-three]->[violet]->[cedar]", 3);

    let selected = must_ref(
        &mut pangine,
        "['archive'] @ ['amber-rune']->[amber]->['amber-choice'];
         ['archive'] @ ['violet-rune']->[violet]->['violet-choice'];
         ['opaque-net'] = $['amber-choice'];
         ['opaque-net'] /= $['violet-choice'];
         ^['opaque-net']",
    );
    assert_eq!(pangine.format_concept(&selected, false), "[quartz]");

    assert_eq!(
        read_named_decision(&mut pangine, "opaque-net"),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 1), ("[quartz]", 3)]), selected: Some("[quartz]".to_owned()) }
    );
}

#[test]
#[ignore = "warning: represented stance controls an explicit two-stream Pangine program, not a universal sign rule"]
fn represented_stance_changes_the_answer_without_changing_the_generic_program() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive", "[amber-a]->[amber]->[A]", 4);
    experience(&mut pangine, "archive", "[amber-b]->[amber]->[B]", 3);
    experience(&mut pangine, "archive", "[violet-a]->[violet]->[A]", 3);
    experience(&mut pangine, "archive", "[violet-b]->[violet]->[B]", 5);
    experience(&mut pangine, "stance", "[amber]->[role]->[positive]", 1);
    experience(&mut pangine, "stance", "[violet]->[role]->[negative]", 1);

    let archive = pangine.reference_percept("archive");
    let stance = pangine.reference_percept("stance");
    let raw_choice = pangine.reference_percept("raw-choice");
    let raw_role = pangine.reference_percept("raw-role");
    let raw_question_text = "(['raw-event']->['raw-relation']->['raw-choice'])(['raw-relation']->[role]->['raw-role'])";
    let raw_question = must_ref(&mut pangine, raw_question_text);
    let raw_result = pangine.complete_question(&[archive, stance], &raw_question).expect("valid represented-stance join");
    assert_eq!(
        raw_result
            .completions()
            .iter()
            .map(|completion| {
                (
                    pangine.format_concept(completion.binding(&raw_choice).expect("bound raw choice"), false),
                    pangine.format_concept(completion.binding(&raw_role).expect("bound raw role"), false),
                )
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ("[A]".to_owned(), "[negative]".to_owned()),
            ("[A]".to_owned(), "[positive]".to_owned()),
            ("[B]".to_owned(), "[negative]".to_owned()),
            ("[B]".to_owned(), "[positive]".to_owned()),
        ])
    );
    pangine.reference_concept(&format!("['archive']['stance'] @ {raw_question_text}")).expect("valid materialized represented-stance join");
    assert_eq!(
        read_named_decision(&mut pangine, "raw-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 7), ("[B]", 8)]), selected: Some("[B]".to_owned()) }
    );

    ask_represented_stance(&mut pangine, "['archive']['stance']", "['positive-relation']", "['negative-relation']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", -2)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut pangine, "stance", "[amber]->[role]->[positive]", 19);
    ask_represented_stance(&mut pangine, "['archive']['stance']", "['positive-relation']", "['negative-relation']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", -2)]), selected: Some("[A]".to_owned()) }
    );

    clear_percept(&mut pangine, "stance");
    experience(&mut pangine, "stance", "[amber]->[role]->[positive]", 1);
    ask_represented_stance(&mut pangine, "['archive']['stance']", "['positive-relation']", "['negative-relation']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    clear_percept(&mut pangine, "stance");
    experience(&mut pangine, "stance", "[amber]->[role]->[negative]", 1);
    experience(&mut pangine, "stance", "[violet]->[role]->[positive]", 1);
    ask_represented_stance(&mut pangine, "['archive']['stance']", "['positive-relation']", "['negative-relation']");
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", -1), ("[B]", 2)]), selected: Some("[B]".to_owned()) }
    );

    clear_percept(&mut pangine, "stance");
    experience(&mut pangine, "stance", "[amber]->[role]->[positive]", 1);
    experience(&mut pangine, "stance", "[amber]->[role]->[negative]", 1);
    ask_represented_stance(&mut pangine, "['archive']['stance']", "['positive-relation']", "['negative-relation']");
    assert_eq!(read_named_decision(&mut pangine, "net-choice"), DecisionResult { candidates: BTreeMap::new(), selected: None });
}

#[test]
#[ignore = "warning: a grounded stance snapshot preserves one decision context but is not a general provenance system"]
fn a_contextual_stance_snapshot_keeps_original_experience_rejoinable() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive", "[amber-a]->[amber]->[A]", 4);
    experience(&mut pangine, "archive", "[amber-b]->[amber]->[B]", 3);
    experience(&mut pangine, "archive", "[violet-a]->[violet]->[A]", 3);
    experience(&mut pangine, "archive", "[violet-b]->[violet]->[B]", 5);
    experience(&mut pangine, "stance", "[north]->[amber]->[role]->[positive]", 1);
    experience(&mut pangine, "stance", "[north]->[violet]->[role]->[negative]", 1);
    experience(&mut pangine, "stance", "[south]->[amber]->[role]->[negative]", 1);
    experience(&mut pangine, "stance", "[south]->[violet]->[role]->[positive]", 1);
    experience(&mut pangine, "context", "[request]->[zone]->[north]", 20);

    let snapshot_question = "(['snapshot-zone']->['snapshot-relation']->[role]->['snapshot-role'])([request]->[zone]->['snapshot-zone'])";
    must_ref(&mut pangine, &format!("['decision-stance'] = ['stance']['context'] @ {snapshot_question}"));
    ask_represented_stance(
        &mut pangine,
        "['archive']['decision-stance']",
        "['support-zone']->['positive-relation']",
        "['counter-zone']->['negative-relation']",
    );
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", -2)]), selected: Some("[A]".to_owned()) }
    );
    must_ref(&mut pangine, "['selected-choice'] = ^['net-choice']");
    assert_eq!(selected_experience_histories(&mut pangine), candidate_history_map(&[("[amber-a]", "[positive]", 4), ("[violet-a]", "[negative]", 3)]));

    let net_choice = pangine.reference_percept("net-choice");
    let history_question = must_ref(
        &mut pangine,
        "(['history-event']->['history-relation']->$['selected-choice'])(['history-zone']->['history-relation']->[role]->['history-role'])",
    );
    assert!(pangine.complete_question(std::slice::from_ref(&net_choice), &history_question).expect("valid net-only history question").completions().is_empty());

    must_ref(&mut pangine, "['context'] = [request]->[zone]->[south]");
    assert_eq!(selected_experience_histories(&mut pangine), candidate_history_map(&[("[amber-a]", "[positive]", 4), ("[violet-a]", "[negative]", 3)]));

    must_ref(&mut pangine, &format!("['decision-stance'] = ['stance']['context'] @ {snapshot_question}"));
    ask_represented_stance(
        &mut pangine,
        "['archive']['decision-stance']",
        "['support-zone']->['positive-relation']",
        "['counter-zone']->['negative-relation']",
    );
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", -1), ("[B]", 2)]), selected: Some("[B]".to_owned()) }
    );
    must_ref(&mut pangine, "['selected-choice'] = ^['net-choice']");
    assert_eq!(selected_experience_histories(&mut pangine), candidate_history_map(&[("[amber-b]", "[negative]", 3), ("[violet-b]", "[positive]", 5)]));

    experience(&mut pangine, "archive", "[later-violet-b]->[violet]->[B]", 10);
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", -1), ("[B]", 2)]), selected: Some("[B]".to_owned()) },
        "the stored net does not silently recompute"
    );
    assert_eq!(
        selected_experience_histories(&mut pangine),
        candidate_history_map(&[("[amber-b]", "[negative]", 3), ("[later-violet-b]", "[positive]", 10), ("[violet-b]", "[positive]", 5)]),
        "rejoining untouched sources reads their current experience, not a historical proof snapshot"
    );
}

#[test]
#[ignore = "warning: rejoining staged context to original experience is promising but not a general decision pipeline"]
fn a_grounded_context_stage_can_filter_then_rejoin_original_experience() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "body", &answer_experience("event-repeat", "A"), 2);
    experience(&mut pangine, "body", &answer_experience("event-distinct-one", "B"), 1);
    experience(&mut pangine, "body", &answer_experience("event-distinct-two", "B"), 1);
    let weighted_experience = format!("x2({})", answer_experience("event-weighted", "C"));
    experience(&mut pangine, "body", &weighted_experience, 2);
    experience(&mut pangine, "body", &answer_experience("event-noise", "D"), 20);

    for event in ["event-repeat", "event-distinct-one", "event-distinct-two", "event-weighted"] {
        experience(&mut pangine, "locations", &format!("[{event}]->[zone]->[north]"), 1);
    }
    experience(&mut pangine, "locations", "[event-noise]->[zone]->[south]", 1);
    experience(&mut pangine, "context", "[request]->[zone]->[north]", 1);

    let stage_question = "(['stage-event']->[zone]->['stage-zone'])([request]->[zone]->['stage-zone'])";
    must_ref(&mut pangine, &format!("['eligible-events'] = ['locations']['context'] @ {stage_question}"));

    let body = pangine.reference_percept("body");
    let eligible_events = pangine.reference_percept("eligible-events");
    let final_event = pangine.reference_percept("final-event");
    let choice = pangine.reference_percept("choice");
    let weighted_experience = must_ref(&mut pangine, &weighted_experience);
    let final_question_text = "(['final-event']->[zone]->['final-zone'])(['final-event']->[answer]->['choice'])";
    let final_question = must_ref(&mut pangine, final_question_text);
    let result = pangine.complete_question(&[body, eligible_events], &final_question).expect("valid staged-context decision question");

    let histories = result
        .completions()
        .iter()
        .map(|completion| {
            let event = pangine.format_concept(completion.binding(&final_event).expect("bound final event"), false);
            let evidence = completion.evidence().iter().find(|evidence| evidence.binding(&choice).is_some()).expect("answer evidence");
            let coefficient_ancestors = evidence.coefficient_ancestors().collect::<BTreeSet<_>>();
            assert!(coefficient_ancestors.is_empty() || coefficient_ancestors == BTreeSet::from([&weighted_experience]));
            (
                event,
                ExperienceHistory {
                    candidate: pangine.format_concept(completion.binding(&choice).expect("bound choice"), false),
                    source_relevance: evidence.source_relevance(),
                    coefficient_bearing: !coefficient_ancestors.is_empty(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        histories,
        BTreeMap::from([
            (
                "[event-distinct-one]".to_owned(),
                ExperienceHistory { candidate: "[B]".to_owned(), source_relevance: Relevance::DEFAULT, coefficient_bearing: false },
            ),
            (
                "[event-distinct-two]".to_owned(),
                ExperienceHistory { candidate: "[B]".to_owned(), source_relevance: Relevance::DEFAULT, coefficient_bearing: false },
            ),
            ("[event-repeat]".to_owned(), ExperienceHistory { candidate: "[A]".to_owned(), source_relevance: Relevance::new(2), coefficient_bearing: false },),
            ("[event-weighted]".to_owned(), ExperienceHistory { candidate: "[C]".to_owned(), source_relevance: Relevance::new(2), coefficient_bearing: true },),
        ])
    );

    pangine.reference_concept(&format!("['body']['eligible-events'] @ {final_question_text}")).expect("valid materialized staged-context decision question");
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[A]", 2), ("[B]", 2), ("[C]", 2)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut pangine, "signals", "[north]->[leans]->[A]", 1);
    experience(&mut pangine, "signals", "[north]->[leans]->[B]", 5);
    experience(&mut pangine, "signals", "[north]->[leans]->[C]", 1);
    let supported_question =
        "(['supported-event']->[zone]->['supported-zone'])(['supported-event']->[answer]->['supported-choice'])([north]->[leans]->['supported-choice'])";
    pangine.reference_concept(&format!("['body']['eligible-events']['signals'] @ {supported_question}")).expect("valid answer-linked context question");
    assert_eq!(
        read_named_decision(&mut pangine, "supported-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 3), ("[B]", 7), ("[C]", 3)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "context", "[request]->[zone]->[north]", 19);
    must_ref(&mut pangine, &format!("['eligible-events'] = ['locations']['context'] @ {stage_question}"));
    pangine.reference_concept(&format!("['body']['eligible-events'] @ {final_question_text}")).expect("valid repeated-context decision question");
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[A]", 2), ("[B]", 2), ("[C]", 2)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut pangine, "context", "[request]->[zone]->[south]", 1);
    must_ref(&mut pangine, &format!("['eligible-events'] = ['locations']['context'] @ {stage_question}"));
    pangine.reference_concept(&format!("['body']['eligible-events'] @ {final_question_text}")).expect("valid overlapping-context decision question");
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[A]", 2), ("[B]", 2), ("[C]", 2), ("[D]", 20)]), selected: Some("[D]".to_owned()) }
    );

    must_ref(&mut pangine, "['context'] = [request]->[zone]->[south]");
    must_ref(&mut pangine, &format!("['eligible-events'] = ['locations']['context'] @ {stage_question}"));
    pangine.reference_concept(&format!("['body']['eligible-events'] @ {final_question_text}")).expect("valid changed-context decision question");
    assert_eq!(read_decision(&mut pangine), DecisionResult { candidates: candidate_map(&[("[D]", 20)]), selected: Some("[D]".to_owned()) });
}

#[test]
#[ignore = "warning: staged support-minus-counterevidence is one explicit Pangine program, not an automatic meaning of context"]
fn staged_context_can_filter_support_and_counterevidence_before_pangine_combines_them() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "body", "[north-support-a]->[supports]->[A]", 4);
    experience(&mut pangine, "body", "[north-support-b]->[supports]->[B]", 3);
    experience(&mut pangine, "body", "[south-support-c]->[supports]->[C]", 20);
    experience(&mut pangine, "body", "[south-counter-c]->[opposes]->[C]", 18);

    for event in ["north-support-a", "north-support-b"] {
        experience(&mut pangine, "locations", &format!("[{event}]->[zone]->[north]"), 1);
    }
    for event in ["south-support-c", "south-counter-c"] {
        experience(&mut pangine, "locations", &format!("[{event}]->[zone]->[south]"), 1);
    }
    experience(&mut pangine, "context", "[request]->[zone]->[north]", 1);

    let stage_question = "(['stage-event']->[zone]->['stage-zone'])([request]->[zone]->['stage-zone'])";
    let support_question = "(['support-event']->[zone]->['support-zone'])(['support-event']->[supports]->['support-choice'])";
    let counter_question = "(['counter-event']->[zone]->['counter-zone'])(['counter-event']->[opposes]->['counter-choice'])";
    let ask = |pangine: &mut Pangine| {
        must_ref(pangine, &format!("['eligible-events'] = ['locations']['context'] @ {stage_question}"));
        pangine.reference_concept(&format!("['body']['eligible-events'] @ {support_question}")).expect("valid staged support question");
        pangine.reference_concept(&format!("['body']['eligible-events'] @ {counter_question}")).expect("valid staged counter question");
        must_ref(pangine, "['net-choice'] = $['support-choice']");
        must_ref(pangine, "['net-choice'] /= $['counter-choice']");
    };

    ask(&mut pangine);
    assert_eq!(read_named_decision(&mut pangine, "counter-choice"), DecisionResult { candidates: BTreeMap::new(), selected: None });
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 4), ("[B]", 3)]), selected: Some("[A]".to_owned()) }
    );

    experience(&mut pangine, "body", "[north-counter-a]->[opposes]->[A]", 3);
    experience(&mut pangine, "locations", "[north-counter-a]->[zone]->[north]", 1);
    ask(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "context", "[request]->[zone]->[north]", 19);
    ask(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", 3)]), selected: Some("[B]".to_owned()) }
    );

    experience(&mut pangine, "body", "[north-counter-b]->[opposes]->[B]", 4);
    experience(&mut pangine, "locations", "[north-counter-b]->[zone]->[north]", 1);
    ask(&mut pangine);
    assert_eq!(
        read_named_decision(&mut pangine, "net-choice"),
        DecisionResult { candidates: candidate_map(&[("[A]", 1), ("[B]", -1)]), selected: Some("[A]".to_owned()) }
    );

    must_ref(&mut pangine, "['context'] = [request]->[zone]->[south]");
    ask(&mut pangine);
    assert_eq!(read_named_decision(&mut pangine, "net-choice"), DecisionResult { candidates: candidate_map(&[("[C]", 2)]), selected: Some("[C]".to_owned()) });
}

#[test]
#[ignore = "warning: a one-clause staged filter is still only one provisional way to carry context"]
fn staged_rejoin_survives_opaque_renaming_and_a_one_clause_stage() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive", "[rune-one]->[mark]->[cedar]", 2);
    experience(&mut pangine, "archive", "[rune-two]->[mark]->[quartz]", 1);
    experience(&mut pangine, "archive", "[rune-three]->[mark]->[quartz]", 1);
    experience(&mut pangine, "archive", "[rune-noise]->[mark]->[obsidian]", 20);

    experience(&mut pangine, "catalogue", "[amber]->[rune-one]", 1);
    experience(&mut pangine, "catalogue", "[amber]->[rune-two]", 1);
    experience(&mut pangine, "catalogue", "[amber]->[rune-three]", 1);
    experience(&mut pangine, "catalogue", "[violet]->[rune-noise]", 1);

    must_ref(&mut pangine, "['amber-runes'] = ['catalogue'] @ [amber]->['stage-rune']");
    let question = "(['rune-class']->['rune'])(['rune']->[mark]->['choice'])";
    pangine.reference_concept(&format!("['archive']['amber-runes'] @ {question}")).expect("valid renamed staged-rejoin question");
    assert_eq!(
        read_decision(&mut pangine),
        DecisionResult { candidates: candidate_map(&[("[cedar]", 2), ("[quartz]", 2)]), selected: Some("[cedar]".to_owned()) }
    );
}

fn ask_represented_stance(pangine: &mut Pangine, selector: &str, positive_relation: &str, negative_relation: &str) {
    let support_question = format!("(['support-event']->['positive-relation']->['support-choice'])({positive_relation}->[role]->[positive])");
    let counter_question = format!("(['counter-event']->['negative-relation']->['counter-choice'])({negative_relation}->[role]->[negative])");
    pangine
        .reference_concept(&format!(
            "{selector} @ {support_question};
             {selector} @ {counter_question};
             ['net-choice'] = $['support-choice'];
             ['net-choice'] /= $['counter-choice']"
        ))
        .expect("valid represented-stance program");
}

fn selected_experience_histories(pangine: &mut Pangine) -> BTreeMap<String, (String, Relevance)> {
    let archive = pangine.reference_percept("archive");
    let decision_stance = pangine.reference_percept("decision-stance");
    let history_event = pangine.reference_percept("history-event");
    let history_role = pangine.reference_percept("history-role");
    let question =
        must_ref(pangine, "(['history-event']->['history-relation']->$['selected-choice'])(['history-zone']->['history-relation']->[role]->['history-role'])");
    pangine
        .complete_question(&[archive.clone(), decision_stance], &question)
        .expect("valid selected-history rejoin")
        .completions()
        .iter()
        .map(|completion| {
            let evidence = completion
                .evidence()
                .iter()
                .find(|evidence| evidence.source_subject() == &archive && evidence.binding(&history_event).is_some())
                .expect("archive evidence for selected history");
            (
                pangine.format_concept(completion.binding(&history_event).expect("bound history event"), false),
                (pangine.format_concept(completion.binding(&history_role).expect("bound history role"), false), evidence.source_relevance()),
            )
        })
        .collect()
}

fn candidate_history_map(entries: &[(&str, &str, i64)]) -> BTreeMap<String, (String, Relevance)> {
    entries.iter().map(|(event, role, relevance)| ((*event).to_owned(), ((*role).to_owned(), Relevance::new(*relevance)))).collect()
}

fn clear_percept(pangine: &mut Pangine, name: &str) {
    let input = format!("['{name}'] = []");
    assert!(pangine.reference_concept(&input).unwrap_or_else(|error| panic!("failed to clear {name:?}: {error}")).is_none());
}

fn decide(rows: &[(&str, &str, &str, usize)], eligible_pairs: &[(&str, &str)]) -> DecisionResult {
    let mut pangine = Pangine::new();
    for (index, (source, context, candidate, repetitions)) in rows.iter().enumerate() {
        let case = format!("case-{index}");
        let row = format!("([{case}]->[source]->[{source}])([{case}]->[context]->[{context}])([{case}]->[answer]->[{candidate}])");
        experience(&mut pangine, "evidence", &row, *repetitions);
    }
    for (source, context) in eligible_pairs {
        experience(&mut pangine, "policy", &format!("[{source}]->[eligible]->[{context}]"), 1);
    }

    let question = "['evidence']['policy'] @ (['case']->[source]->['source'])(['case']->[context]->['context'])(['case']->[answer]->['choice'])(['source']->[eligible]->['context'])";
    pangine.reference_concept(question).unwrap_or_else(|error| panic!("failed to parse represented policy question: {error}"));

    read_decision(&mut pangine)
}

fn sensor_experience(pangine: &mut Pangine, sensor: &str, location: &str, response: &str, repetitions: usize) {
    let row = format!("([{sensor}]->[location]->[{location}])([{sensor}]->[state]->[firing])([{sensor}]->[response]->[{response}])");
    experience(pangine, "body", &row, repetitions);
}

fn prepare_explicit_activation_comparison() -> Pangine {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "opal-sensor", "[mark]->[saffron]->[cedar]", 1);
    experience(&mut pangine, "quartz-sensor", "[mark]->[saffron]->[violet]", 2);
    experience(&mut pangine, "activation-pointer-inventory", "['opal-sensor']", 3);
    experience(&mut pangine, "activation-pointer-inventory", "['quartz-sensor']", 1);
    must_run(&mut pangine, "(['activation-pointer-inventory'] @ ['activation-pointer-state']) @ [mark]->[saffron]->['retained-report-answer']");

    experience(&mut pangine, "activation-observations", "([lantern]->[amber]->['opal-sensor'])([lantern]->[cobalt]->[cedar])", 3);
    experience(&mut pangine, "activation-observations", "([topaz-one]->[amber]->['quartz-sensor'])([topaz-one]->[cobalt]->[violet])", 1);
    experience(&mut pangine, "activation-observations", "([topaz-two]->[amber]->['quartz-sensor'])([topaz-two]->[cobalt]->[violet])", 1);
    must_run(
        &mut pangine,
        "['activation-observations'] @ (['activation-event']->[amber]->['activation-source'])(['activation-event']->[cobalt]->['activation-answer'])",
    );
    pangine
}

fn prepare_historical_answer_record_comparison() -> Pangine {
    let mut pangine = prepare_explicit_activation_comparison();
    capture_answer_state(&mut pangine, "copper-fern-state", "copper-fern-choice", "retained-report-answer");
    capture_answer_state(&mut pangine, "copper-moss-state", "copper-moss-choice", "activation-answer");

    experience(&mut pangine, "activation-observations", "([topaz-one]->[amber]->['quartz-sensor'])([topaz-one]->[cobalt]->[violet])", 2);
    must_run(
        &mut pangine,
        "['activation-observations'] @
           (['silver-activation-event']->[amber]->['silver-activation-source'])
           (['silver-activation-event']->[cobalt]->['silver-firing-state'])",
    );
    capture_answer_state(&mut pangine, "silver-fern-state", "silver-fern-choice", "retained-report-answer");
    capture_answer_state(&mut pangine, "silver-moss-state", "silver-moss-choice", "silver-firing-state");

    experience(&mut pangine, "opal-sensor", "[mark]->[saffron]->[cedar]", 3);
    must_run(&mut pangine, "(['activation-pointer-inventory'] @ ['gold-pointer-state']) @ [mark]->[saffron]->['gold-report-state']");
    capture_answer_state(&mut pangine, "gold-fern-state", "gold-fern-choice", "gold-report-state");
    capture_answer_state(&mut pangine, "gold-moss-state", "gold-moss-choice", "silver-firing-state");

    for (entry, record, view, state, choice) in [
        ("copper-fern-entry", "copper", "fern", "copper-fern-state", "copper-fern-choice"),
        ("copper-moss-entry", "copper", "moss", "copper-moss-state", "copper-moss-choice"),
        ("silver-fern-entry", "silver-record", "fern", "silver-fern-state", "silver-fern-choice"),
        ("silver-moss-entry", "silver-record", "moss", "silver-moss-state", "silver-moss-choice"),
        ("gold-fern-entry", "gold", "fern", "gold-fern-state", "gold-fern-choice"),
        ("gold-moss-entry", "gold", "moss", "gold-moss-state", "gold-moss-choice"),
    ] {
        remember_historical_answer_view(&mut pangine, entry, record, view, state, choice);
    }
    pangine
}

fn answer_view_episode(event: &str, context: &str, view: &str) -> String {
    format!("([{event}]->[amber]->[{context}])([{event}]->[violet]->[{view}])")
}

fn answer_view_result_episode(event: &str, context: &str, view: &str, result: &str) -> String {
    format!("([{event}]->[amber]->[{context}])([{event}]->[violet]->[{view}])([{event}]->[indigo]->[{result}])")
}

fn answer_choice_result_episode(event: &str, context: &str, choice: &str, result: &str) -> String {
    format!("([{event}]->[amber]->[{context}])([{event}]->[violet]->[{choice}])([{event}]->[indigo]->[{result}])")
}

fn historical_answer_choice_episode(event: &str, context: &str, record: &str, choice: &str, result: &str) -> String {
    format!(
        "([{event}]->[amber]->[{context}])
         ([{event}]->[sienna]->[{record}])
         ([{event}]->[violet]->[{choice}])
         ([{event}]->[indigo]->[{result}])"
    )
}

fn capture_answer_state(pangine: &mut Pangine, state: &str, choice: &str, source: &str) {
    must_run(pangine, &format!("['{state}'] = $['{source}']; ['{choice}'] = ^['{state}']"));
}

fn remember_historical_answer_view(pangine: &mut Pangine, entry: &str, record: &str, view: &str, state: &str, choice: &str) {
    experience(
        pangine,
        "historical-answer-records",
        &format!(
            "([{entry}]->[decision-record]->[{record}])
             ([{entry}]->[answer-view]->[{view}])
             ([{entry}]->[answer-state]->$['{state}'])
             ([{entry}]->[answer-choice]->$['{choice}'])"
        ),
        1,
    );
}

fn derive_context_answer_view(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['activation-answer-view-episodes']['activation-answer-context'] @
           (['routing-episode']->[amber]->['routing-context'])
           (['routing-episode']->[violet]->['routing-view'])
           ([request]->[cobalt]->['routing-context'])",
    );
}

fn run_context_selected_activation_answer(pangine: &mut Pangine) {
    derive_context_answer_view(pangine);
    must_run(pangine, "['selected-answer-view'] = [selected-view]->^['routing-view']");
    must_run(
        pangine,
        "['activation-answer-view-record']['selected-answer-view'] @
           (['represented-answer-view']->[rose]->['context-selected-answer-state'])
           ([selected-view]->['represented-answer-view'])",
    );
    must_run(pangine, "['context-selected-answer-choice'] = ^['context-selected-answer-state']");
}

fn derive_context_result_answer_view(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['activation-answer-view-result-episodes']['activation-answer-context']['requested-answer-result'] @
           (['result-routing-episode']->[amber]->['result-routing-context'])
           (['result-routing-episode']->[violet]->['result-routing-view'])
           (['result-routing-episode']->[indigo]->['result-routing-result'])
           ([request]->[cobalt]->['result-routing-context'])
           ([request]->[saffron]->['result-routing-result'])",
    );
}

fn run_context_result_selected_activation_answer(pangine: &mut Pangine) {
    derive_context_result_answer_view(pangine);
    must_run(pangine, "['result-selected-answer-view'] = [selected-view]->^['result-routing-view']");
    must_run(
        pangine,
        "['activation-answer-view-record']['result-selected-answer-view'] @
           (['represented-answer-view']->[rose]->['result-selected-answer-state'])
           ([selected-view]->['represented-answer-view'])",
    );
    must_run(pangine, "['result-selected-answer-choice'] = ^['result-selected-answer-state']");
}

fn derive_all_choice_result_answer_views(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['activation-answer-observation-record']['activation-answer-choice-result-episodes']['activation-answer-context']['requested-answer-result'] @
           (['direct-choice-derived-answer-view']->[topaz]->['direct-past-answer-choice'])
           (['direct-choice-routing-episode']->[amber]->['direct-choice-routing-context'])
           (['direct-choice-routing-episode']->[violet]->['direct-past-answer-choice'])
           (['direct-choice-routing-episode']->[indigo]->['direct-choice-routing-result'])
           ([request]->[cobalt]->['direct-choice-routing-context'])
           ([request]->[saffron]->['direct-choice-routing-result'])",
    );
}

fn derive_observed_answer_choice(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['activation-answer-choice-result-episodes']['activation-answer-context']['requested-answer-result'] @
           (['choice-routing-episode']->[amber]->['choice-routing-context'])
           (['choice-routing-episode']->[violet]->['observed-answer-choice'])
           (['choice-routing-episode']->[indigo]->['choice-routing-result'])
           ([request]->[cobalt]->['choice-routing-context'])
           ([request]->[saffron]->['choice-routing-result'])",
    );
}

fn run_choice_result_derived_activation_answer(pangine: &mut Pangine) {
    derive_observed_answer_choice(pangine);
    must_run(pangine, "['selected-past-answer-choice'] = [past-choice]->^['observed-answer-choice']");
    must_run(
        pangine,
        "['activation-answer-observation-record']['selected-past-answer-choice'] @
           (['choice-derived-answer-view']->[topaz]->['recorded-answer-choice'])
           ([past-choice]->['recorded-answer-choice'])",
    );
    must_run(pangine, "['choice-selected-answer-view'] = [selected-view]->^['choice-derived-answer-view']");
    must_run(
        pangine,
        "['activation-answer-observation-record']['choice-selected-answer-view'] @
           (['represented-answer-view']->[rose]->['choice-derived-answer-state'])
           ([selected-view]->['represented-answer-view'])",
    );
    must_run(pangine, "['choice-derived-answer-choice'] = ^['choice-derived-answer-state']");
}

fn derive_historical_decision_record(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result'] @
           (['historical-routing-episode']->[amber]->['historical-routing-context'])
           (['historical-routing-episode']->[sienna]->['historical-decision-record'])
           (['historical-routing-episode']->[indigo]->['historical-routing-result'])
           ([request]->[cobalt]->['historical-routing-context'])
           ([request]->[saffron]->['historical-routing-result'])",
    );
}

fn derive_all_record_linked_views(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['historical-answer-records']['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result'] @
           (['all-record-routing-episode']->[amber]->['all-record-routing-context'])
           (['all-record-routing-episode']->[sienna]->['all-record-routing-record'])
           (['all-record-routing-episode']->[violet]->['all-record-routing-choice'])
           (['all-record-routing-episode']->[indigo]->['all-record-routing-result'])
           (['all-record-view-entry']->[decision-record]->['all-record-routing-record'])
           (['all-record-view-entry']->[answer-view]->['all-record-derived-view'])
           (['all-record-view-entry']->[answer-choice]->['all-record-routing-choice'])
           ([request]->[cobalt]->['all-record-routing-context'])
           ([request]->[saffron]->['all-record-routing-result'])",
    );
}

fn stage_and_rejoin_record_linked_views(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['complete-record-view-rows'] =
         ['historical-answer-records']['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result'] @
           (['staged-source-episode']->[amber]->['staged-source-context'])
           (['staged-source-episode']->[sienna]->['staged-source-record'])
           (['staged-source-episode']->[violet]->['staged-source-choice'])
           (['staged-source-episode']->[indigo]->['staged-source-result'])
           (['staged-source-entry']->[decision-record]->['staged-source-record'])
           (['staged-source-entry']->[answer-view]->['staged-source-view'])
           (['staged-source-entry']->[answer-choice]->['staged-source-choice'])
           ([request]->[cobalt]->['staged-source-context'])
           ([request]->[saffron]->['staged-source-result'])",
    );
    must_run(
        pangine,
        "['record-view-mapping-rows'] = ['complete-record-view-rows'] @
           (['mapping-episode']->[sienna]->['mapping-record'])
           (['mapping-episode']->[violet]->['mapping-choice'])
           (['mapping-entry']->[decision-record]->['mapping-record'])
           (['mapping-entry']->[answer-view]->['mapping-view'])
           (['mapping-entry']->[answer-choice]->['mapping-choice'])",
    );
    must_run(
        pangine,
        "['historical-answer-choice-episodes']['record-view-mapping-rows']['historical-answer-context']['requested-historical-result'] @
           (['rejoined-mapping-episode']->[amber]->['rejoined-mapping-context'])
           (['rejoined-mapping-episode']->[sienna]->['rejoined-mapping-record'])
           (['rejoined-mapping-episode']->[violet]->['rejoined-mapping-choice'])
           (['rejoined-mapping-episode']->[indigo]->['rejoined-mapping-result'])
           (['rejoined-mapping-entry']->[decision-record]->['rejoined-mapping-record'])
           (['rejoined-mapping-entry']->[answer-view]->['rejoined-mapping-view'])
           (['rejoined-mapping-entry']->[answer-choice]->['rejoined-mapping-choice'])
           ([request]->[cobalt]->['rejoined-mapping-context'])
           ([request]->[saffron]->['rejoined-mapping-result'])",
    );
}

fn derive_tentative_historical_reading(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result'] @
           (['tentative-reading-episode']->[amber]->['tentative-reading-context'])
           (['tentative-reading-episode']->[sienna]->['tentative-reading-record'])
           (['tentative-reading-episode']->[violet]->['tentative-historical-observed-reading'])
           (['tentative-reading-episode']->[indigo]->['tentative-reading-result'])
           ([request]->[cobalt]->['tentative-reading-context'])
           ([request]->[saffron]->['tentative-reading-result'])",
    );
}

fn select_tentative_historical_reading(pangine: &mut Pangine) {
    must_run(pangine, "['selected-tentative-historical-reading'] = [tentative-reading]->^['tentative-historical-observed-reading']");
}

fn explain_tentative_historical_reading(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['historical-answer-records']['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result']['selected-tentative-historical-reading'] @
           (['tentative-explanation-episode']->[amber]->['tentative-explanation-context'])
           (['tentative-explanation-episode']->[sienna]->['tentative-explanation-record'])
           (['tentative-explanation-episode']->[violet]->['tentative-explanation-reading'])
           (['tentative-explanation-episode']->[indigo]->['tentative-explanation-result'])
           (['tentative-explanation-entry']->[decision-record]->['tentative-explanation-record'])
           (['tentative-explanation-entry']->[answer-view]->['tentative-explanation-view'])
           (['tentative-explanation-entry']->[answer-state]->['tentative-explanation-answer-state'])
           (['tentative-explanation-entry']->[answer-choice]->['tentative-explanation-reading'])
           ([request]->[cobalt]->['tentative-explanation-context'])
           ([request]->[saffron]->['tentative-explanation-result'])
           ([tentative-reading]->['tentative-explanation-reading'])",
    );
}

fn compare_tentative_reading_with_latest_views(pangine: &mut Pangine) {
    must_run(pangine, "['latest-fern-reading-link'] = [latest-reading]->$['gold-fern-choice']");
    must_run(pangine, "['latest-moss-reading-link'] = [latest-reading]->$['gold-moss-choice']");
    must_run(
        pangine,
        "['selected-tentative-historical-reading']['latest-fern-reading-link'] @
           ([tentative-reading]->['tentative-latest-fern-agreement'])
           ([latest-reading]->['tentative-latest-fern-agreement'])",
    );
    must_run(
        pangine,
        "['selected-tentative-historical-reading']['latest-moss-reading-link'] @
           ([tentative-reading]->['tentative-latest-moss-agreement'])
           ([latest-reading]->['tentative-latest-moss-agreement'])",
    );
}

fn materialize_tentative_decision_details(pangine: &mut Pangine) {
    must_run(
        pangine,
        "['tentative-record-explanation-rows'] =
         ['historical-answer-records']['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result']['selected-tentative-historical-reading'] @
           (['recordable-explanation-episode']->[amber]->['recordable-explanation-context'])
           (['recordable-explanation-episode']->[sienna]->['recordable-explanation-record'])
           (['recordable-explanation-episode']->[violet]->['recordable-explanation-reading'])
           (['recordable-explanation-episode']->[indigo]->['recordable-explanation-result'])
           (['recordable-explanation-entry']->[decision-record]->['recordable-explanation-record'])
           (['recordable-explanation-entry']->[answer-view]->['recordable-explanation-view'])
           (['recordable-explanation-entry']->[answer-state]->['recordable-explanation-answer-state'])
           (['recordable-explanation-entry']->[answer-choice]->['recordable-explanation-reading'])
           ([request]->[cobalt]->['recordable-explanation-context'])
           ([request]->[saffron]->['recordable-explanation-result'])
           ([tentative-reading]->['recordable-explanation-reading'])",
    );
    must_run(
        pangine,
        "['tentative-latest-view-reading-rows'] =
           ([fern]->[latest-reading]->$['gold-fern-choice'])
           ([moss]->[latest-reading]->$['gold-moss-choice'])",
    );
    must_run(
        pangine,
        "['tentative-latest-agreement-rows'] =
         ['selected-tentative-historical-reading']['tentative-latest-view-reading-rows'] @
           ([tentative-reading]->['recordable-agreement-reading'])
           (['recordable-agreement-view']->[latest-reading]->['recordable-agreement-reading'])",
    );
    must_run(pangine, "['tentative-selected-reading-value'] = ^['tentative-historical-observed-reading']");
}

fn capture_tentative_decision_record(pangine: &mut Pangine, record: &str) {
    must_run(
        pangine,
        &format!(
            "['{record}'] =
               ([tentative-decision]->[context]->$['historical-answer-context'])
               ([tentative-decision]->[requested-result]->$['requested-historical-result'])
               ([tentative-decision]->[candidates]->$['tentative-historical-observed-reading'])
               ([tentative-decision]->[choice]->$['tentative-selected-reading-value'])
               ([tentative-decision]->[explanation-episodes]->$['tentative-explanation-episode'])
               ([tentative-decision]->[explanation-view-state]->$['tentative-explanation-view'])
               ([tentative-decision]->[explanation-rows]->$['tentative-record-explanation-rows'])
               ([tentative-decision]->[latest-view-readings]->$['tentative-latest-view-reading-rows'])
               ([tentative-decision]->[agreement-state]->$['recordable-agreement-reading'])
               ([tentative-decision]->[agreements]->$['tentative-latest-agreement-rows'])"
        ),
    );
}

fn extract_tentative_decision_record(pangine: &mut Pangine, record: &str, prefix: &str) {
    for (field, output) in [
        ("context", "context"),
        ("requested-result", "requested-result"),
        ("candidates", "candidates"),
        ("choice", "choice"),
        ("explanation-episodes", "explanation-episodes"),
        ("explanation-view-state", "explanation-view-state"),
        ("explanation-rows", "explanation-rows"),
        ("latest-view-readings", "latest-view-readings"),
        ("agreement-state", "agreement-state"),
        ("agreements", "agreements"),
    ] {
        must_run(pangine, &format!("['{record}'] @ [tentative-decision]->[{field}]->['{prefix}-{output}']"));
    }
}

fn inspect_tentative_decision_record_rows(pangine: &mut Pangine, prefix: &str) {
    must_run(
        pangine,
        &format!(
            "['{prefix}-explanation-rows'] @
               (['{prefix}-explanation-episode']->[sienna]->['{prefix}-explanation-record'])
               (['{prefix}-explanation-episode']->[violet]->['{prefix}-explanation-reading'])
               (['{prefix}-explanation-entry']->[decision-record]->['{prefix}-explanation-record'])
               (['{prefix}-explanation-entry']->[answer-view]->['{prefix}-explanation-view'])
               (['{prefix}-explanation-entry']->[answer-choice]->['{prefix}-explanation-reading'])"
        ),
    );
    must_run(pangine, &format!("['{prefix}-latest-view-readings'] @ ['{prefix}-latest-view']->[latest-reading]->['{prefix}-latest-reading']"));
    must_run(
        pangine,
        &format!(
            "['{prefix}-agreements'] @
               ([tentative-reading]->['{prefix}-agreement-reading'])
               (['{prefix}-agreement-view']->[latest-reading]->['{prefix}-agreement-reading'])"
        ),
    );
}

fn run_record_linked_choice_mapping(pangine: &mut Pangine) {
    derive_historical_decision_record(pangine);
    must_run(pangine, "['selected-historical-record'] = [historical-record]->^['historical-decision-record']");
    must_run(
        pangine,
        "['historical-answer-choice-episodes']['historical-answer-context']['requested-historical-result']['selected-historical-record'] @
           (['historical-choice-episode']->[amber]->['historical-choice-context'])
           (['historical-choice-episode']->[sienna]->['historical-choice-record'])
           (['historical-choice-episode']->[violet]->['historical-observed-choice'])
           (['historical-choice-episode']->[indigo]->['historical-choice-result'])
           ([request]->[cobalt]->['historical-choice-context'])
           ([request]->[saffron]->['historical-choice-result'])
           ([historical-record]->['historical-choice-record'])",
    );
    must_run(pangine, "['selected-historical-choice'] = [historical-choice]->^['historical-observed-choice']");
    must_run(
        pangine,
        "['historical-answer-records']['selected-historical-record']['selected-historical-choice'] @
           (['historical-view-entry']->[decision-record]->['historical-view-record'])
           (['historical-view-entry']->[answer-view]->['historical-derived-view'])
           (['historical-view-entry']->[answer-choice]->['historical-recorded-choice'])
           ([historical-record]->['historical-view-record'])
           ([historical-choice]->['historical-recorded-choice'])",
    );
}

fn retrieve_record_linked_answer(pangine: &mut Pangine) {
    must_run(pangine, "['selected-historical-view'] = [historical-view]->^['historical-derived-view']");
    must_run(
        pangine,
        "['historical-answer-records']['selected-historical-record']['selected-historical-view'] @
           (['historical-state-entry']->[decision-record]->['historical-state-record'])
           (['historical-state-entry']->[answer-view]->['historical-state-view'])
           (['historical-state-entry']->[answer-state]->['historical-derived-answer-state'])
           ([historical-record]->['historical-state-record'])
           ([historical-view]->['historical-state-view'])",
    );
    must_run(pangine, "['historical-derived-answer-choice'] = ^['historical-derived-answer-state']");
}

fn ask_sensor_decision(pangine: &mut Pangine) {
    let question = "['body'] @ (['sensor']->[location]->[hand])(['sensor']->[state]->[firing])(['sensor']->[response]->['choice'])";
    pangine.reference_concept(question).unwrap_or_else(|error| panic!("failed to parse sensor decision question: {error}"));
}

fn ask_direct_decision(pangine: &mut Pangine) {
    let question = "['archive-one']['archive-two']['archive-three']['archive-four'] @ [mark]->['choice']";
    pangine.reference_concept(question).unwrap_or_else(|error| panic!("failed to parse direct decision question: {error}"));
}

fn opaque_firing(event: &str, candidate: &str) -> String {
    format!("([{event}]->[amber]->[quill])([{event}]->[violet]->[{candidate}])")
}

fn source_event_report(report: &str, source: &str, condition: &str, event: &str, candidate: &str) -> String {
    format!("([{report}]->[amber]->[{source}])([{report}]->[topaz]->[{condition}])([{report}]->[indigo]->[{event}])([{event}]->[saffron]->[{candidate}])")
}

fn populate_question_order_record(pangine: &mut Pangine) {
    populate_source_event_reports(pangine);
    experience(pangine, "reports", &source_event_report("report-b-four", "source-seven", "opal", "distinct-b-event-four", "B"), 1);

    let report_question = "(['record-report']->[amber]->['record-source'])(['record-report']->[topaz]->['record-condition'])([lantern]->[cobalt]->['record-condition'])(['record-report']->[indigo]->['record-event'])(['record-event']->[saffron]->['outcome-first-state'])";
    must_ref(pangine, &format!("['eligible-order-reports'] = (['reports']['condition'] @ {report_question})"));
    must_ref(pangine, "['outcome-first-conclusion'] = ^['outcome-first-state']");
    must_ref(pangine, "['selected-record-event'] = ^['record-event']");
    must_ref(pangine, "['selected-record-event-link'] = [selected]->$['selected-record-event']");

    let event_first_question = "([selected]->['record-event-first-event'])(['record-event-first-report']->[topaz]->['record-event-first-condition'])([lantern]->[cobalt]->['record-event-first-condition'])(['record-event-first-report']->[indigo]->['record-event-first-event'])(['record-event-first-event']->[saffron]->['event-first-state'])";
    must_ref(pangine, &format!("['selected-record-event-link']['reports']['condition'] @ {event_first_question}"));
    must_ref(pangine, "['event-first-conclusion'] = ^['event-first-state']");

    must_ref(
        pangine,
        "['question-order-record'] =
           ([reasoning]->[reports]->$['eligible-order-reports'])
           ([reasoning]->[sources]->$['record-source'])
           ([reasoning]->[events]->$['record-event'])
           ([outcome-first]->[candidates]->$['outcome-first-state'])
           ([outcome-first]->[conclusion]->$['outcome-first-conclusion'])
           ([event-first]->[selected-event]->$['selected-record-event'])
           ([event-first]->[candidates]->$['event-first-state'])
           ([event-first]->[conclusion]->$['event-first-conclusion'])",
    );
}

fn clear_question_order_live_state(pangine: &mut Pangine) {
    for percept in [
        "reports",
        "condition",
        "eligible-order-reports",
        "record-source",
        "record-event",
        "outcome-first-state",
        "outcome-first-conclusion",
        "selected-record-event",
        "selected-record-event-link",
        "event-first-state",
        "event-first-conclusion",
    ] {
        clear_percept(pangine, percept);
    }
}

fn run_represented_question_order_program(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['order-guidance']['reasoning-context'] @
           (['guidance-context']->[guides]->['relevant-order'])
           ([request]->[context]->['guidance-context']);
         ['selected-reasoning-order'] = [selected-order]->^['relevant-order'];
         ['question-order-record']['selected-reasoning-order'] @
           (['represented-order']->[candidates]->['represented-order-state'])
           ([selected-order]->['represented-order']);
         ['represented-order-choice'] = ^['represented-order-state']",
    );
}

fn question_order_episode(episode: &str, context: &str, order: &str, consequence: &str) -> String {
    format!("([{episode}]->[amber]->[{context}])([{episode}]->[violet]->[{order}])([{episode}]->[indigo]->[{consequence}])")
}

fn remember_question_order_episodes(pangine: &mut Pangine, episodes: &[(&str, &str, &str, &str)]) {
    for (episode, context, order, consequence) in episodes {
        experience(pangine, "order-episodes", &question_order_episode(episode, context, order, consequence), 1);
    }
}

fn remember_weighted_question_order_episodes(pangine: &mut Pangine, episodes: &[(&str, &str, &str, &str, usize)]) {
    for (episode, context, order, consequence, repetitions) in episodes {
        experience(pangine, "order-episodes", &question_order_episode(episode, context, order, consequence), *repetitions);
    }
}

fn remember_pairwise_equal_weighted_question_order_episodes(first: &mut Pangine, second: &mut Pangine) {
    remember_first_pairwise_equal_weighted_question_order_episodes(first);
    remember_second_pairwise_equal_weighted_question_order_episodes(second);
}

fn remember_first_pairwise_equal_weighted_question_order_episodes(pangine: &mut Pangine) {
    remember_weighted_question_order_episodes(
        pangine,
        &[
            ("episode-one", "cedar", "outcome-first", "opal", 2),
            ("episode-two", "cedar", "outcome-first", "basalt", 1),
            ("episode-three", "cedar", "event-first", "opal", 1),
            ("episode-four", "cedar", "event-first", "basalt", 2),
            ("episode-five", "quartz", "outcome-first", "opal", 1),
            ("episode-six", "quartz", "outcome-first", "basalt", 2),
            ("episode-seven", "quartz", "event-first", "opal", 2),
            ("episode-eight", "quartz", "event-first", "basalt", 1),
        ],
    );
}

fn remember_second_pairwise_equal_weighted_question_order_episodes(pangine: &mut Pangine) {
    remember_weighted_question_order_episodes(
        pangine,
        &[
            ("episode-one", "cedar", "outcome-first", "opal", 1),
            ("episode-two", "cedar", "outcome-first", "basalt", 2),
            ("episode-three", "cedar", "event-first", "opal", 2),
            ("episode-four", "cedar", "event-first", "basalt", 1),
            ("episode-five", "quartz", "outcome-first", "opal", 2),
            ("episode-six", "quartz", "outcome-first", "basalt", 1),
            ("episode-seven", "quartz", "event-first", "opal", 1),
            ("episode-eight", "quartz", "event-first", "basalt", 2),
        ],
    );
}

fn prepare_first_weighted_question_order_memory(pangine: &mut Pangine) {
    populate_question_order_record(pangine);
    clear_question_order_live_state(pangine);
    remember_first_pairwise_equal_weighted_question_order_episodes(pangine);
    replace_state(pangine, "episode-context", "[request]->[cobalt]->[cedar]");
    clear_percept(pangine, "requested-consequence");
}

fn valuation_report(report: &str, source: &str, consequence: &str, role: &str) -> String {
    format!("([{report}]->[report-source]->[{source}])([{report}]->[report-consequence]->[{consequence}])([{report}]->[report-role]->[{role}])")
}

fn valuation_observation(report: &str, source: &str, episode: &str, context: &str, order: &str, consequence: &str, role: &str) -> String {
    format!(
        "([{report}]->[report-source]->[{source}])
         ([{report}]->[reported-episode]->[{episode}])
         ([{report}]->[reported-context]->[{context}])
         ([{report}]->[reported-order]->[{order}])
         ([{report}]->[reported-consequence]->[{consequence}])
         ([{report}]->[report-role]->[{role}])"
    )
}

fn enclosed_valuation_observation(report: &str, observation: &str, origins: &[(&str, usize)]) -> String {
    let mut body = observation.to_owned();
    let mut origin_body = String::new();
    for (origin, repetitions) in origins {
        for _ in 0..*repetitions {
            origin_body.push_str(&format!("([{report}]->[origin]->[{origin}])"));
        }
    }
    body.push_str(&format!("([{report}]->[origins]->({origin_body}))"));
    format!("[{report}]->[observation]->({body})")
}

fn remember_enclosed_overlapping_origin_observations(pangine: &mut Pangine, outcome_report_repetitions: usize, shared_origin_repetitions: usize) {
    experience(
        pangine,
        "enclosed-valuation-observations",
        &enclosed_valuation_observation(
            "outcome-positive-report",
            &valuation_observation("outcome-positive-report", "source-one", "episode-one", "cedar", "outcome-first", "opal", "positive"),
            &[("episode-one-origin", shared_origin_repetitions), ("outcome-positive-report-origin", 1)],
        ),
        outcome_report_repetitions,
    );
    for (report, source) in
        [("event-positive-report-one", "source-two"), ("event-positive-report-two", "source-three"), ("event-positive-report-three", "source-four")]
    {
        experience(
            pangine,
            "enclosed-valuation-observations",
            &enclosed_valuation_observation(
                report,
                &valuation_observation(report, source, "episode-three", "cedar", "event-first", "opal", "positive"),
                &[("episode-three-origin", 1)],
            ),
            1,
        );
    }
    for (report, source) in [("outcome-negative-report-one", "source-five"), ("outcome-negative-report-two", "source-six")] {
        experience(
            pangine,
            "enclosed-valuation-observations",
            &enclosed_valuation_observation(
                report,
                &valuation_observation(report, source, "episode-two", "cedar", "outcome-first", "basalt", "negative"),
                &[("episode-two-origin", 1)],
            ),
            1,
        );
    }
    experience(
        pangine,
        "enclosed-valuation-observations",
        &enclosed_valuation_observation(
            "event-negative-report",
            &valuation_observation("event-negative-report", "source-seven", "episode-four", "cedar", "event-first", "basalt", "negative"),
            &[("episode-four-origin", 1)],
        ),
        1,
    );
    experience(
        pangine,
        "enclosed-valuation-observations",
        &enclosed_valuation_observation(
            "mismatched-report",
            &valuation_observation("mismatched-report", "mismatched-source", "episode-one", "cedar", "event-first", "opal", "positive"),
            &[("mismatched-report-origin", 1)],
        ),
        20,
    );
}

fn remember_valuation_origin(pangine: &mut Pangine, subject: &str, origin: &str) {
    experience(pangine, "valuation-provenance", &format!("[{subject}]->[origin]->[{origin}]"), 1);
}

fn remember_origin_independence(pangine: &mut Pangine, left: &str, right: &str) {
    experience(pangine, "valuation-provenance", &format!("[{left}]->[independent-of]->[{right}]"), 1);
}

fn prepare_complete_valuation_observation_memory(pangine: &mut Pangine) {
    prepare_first_weighted_question_order_memory(pangine);
    experience(
        pangine,
        "valuation-observation-control",
        &valuation_observation("event-positive-report", "event-positive-source", "episode-three", "cedar", "event-first", "opal", "positive"),
        1,
    );
    for (report, source) in [("outcome-negative-report-one", "outcome-negative-source-one"), ("outcome-negative-report-two", "outcome-negative-source-two")] {
        experience(
            pangine,
            "valuation-observation-control",
            &valuation_observation(report, source, "episode-two", "cedar", "outcome-first", "basalt", "negative"),
            1,
        );
    }
    experience(
        pangine,
        "valuation-observation-control",
        &valuation_observation("mismatched-report", "mismatched-source", "episode-one", "cedar", "event-first", "opal", "positive"),
        20,
    );
}

fn prepare_balanced_valuation_observation_memory(pangine: &mut Pangine) {
    prepare_first_weighted_question_order_memory(pangine);
    for (report, source, episode, order, consequence, role, repetitions) in [
        ("outcome-positive-report", "source-one", "episode-one", "outcome-first", "opal", "positive", 2),
        ("outcome-negative-report", "source-two", "episode-two", "outcome-first", "basalt", "negative", 1),
        ("event-positive-report", "source-three", "episode-three", "event-first", "opal", "positive", 3),
        ("event-negative-report", "source-four", "episode-four", "event-first", "basalt", "negative", 1),
    ] {
        experience(pangine, "valuation-observations", &valuation_observation(report, source, episode, "cedar", order, consequence, role), repetitions);
    }
    experience(
        pangine,
        "valuation-observations",
        &valuation_observation("mismatched-report", "mismatched-source", "episode-one", "cedar", "event-first", "opal", "positive"),
        20,
    );
}

fn remember_common_balanced_valuation_origins(pangine: &mut Pangine) {
    for (episode, origin) in [
        ("episode-one", "episode-one-origin"),
        ("episode-two", "episode-two-origin"),
        ("episode-three", "episode-three-origin"),
        ("episode-four", "episode-four-origin"),
    ] {
        remember_valuation_origin(pangine, episode, origin);
    }
    for (report, origin) in
        [("outcome-negative-report", "episode-two-origin"), ("event-positive-report", "episode-three-origin"), ("event-negative-report", "episode-four-origin")]
    {
        remember_valuation_origin(pangine, report, origin);
    }
    remember_valuation_origin(pangine, "mismatched-report", "mismatched-report-origin");
    remember_origin_independence(pangine, "mismatched-report-origin", "episode-one-origin");
}

fn prepare_overlapping_origin_question_order_memory(pangine: &mut Pangine) {
    prepare_first_weighted_question_order_memory(pangine);
    experience(
        pangine,
        "valuation-observations",
        &valuation_observation("outcome-positive-report", "source-one", "episode-one", "cedar", "outcome-first", "opal", "positive"),
        2,
    );
    for (report, source) in
        [("event-positive-report-one", "source-two"), ("event-positive-report-two", "source-three"), ("event-positive-report-three", "source-four")]
    {
        experience(pangine, "valuation-observations", &valuation_observation(report, source, "episode-three", "cedar", "event-first", "opal", "positive"), 1);
    }
    for (report, source) in [("outcome-negative-report-one", "source-five"), ("outcome-negative-report-two", "source-six")] {
        experience(pangine, "valuation-observations", &valuation_observation(report, source, "episode-two", "cedar", "outcome-first", "basalt", "negative"), 1);
    }
    experience(
        pangine,
        "valuation-observations",
        &valuation_observation("event-negative-report", "source-seven", "episode-four", "cedar", "event-first", "basalt", "negative"),
        1,
    );
    experience(
        pangine,
        "valuation-observations",
        &valuation_observation("mismatched-report", "mismatched-source", "episode-one", "cedar", "event-first", "opal", "positive"),
        20,
    );

    for (episode, origin) in [
        ("episode-one", "episode-one-origin"),
        ("episode-two", "episode-two-origin"),
        ("episode-three", "episode-three-origin"),
        ("episode-four", "episode-four-origin"),
    ] {
        remember_valuation_origin(pangine, episode, origin);
    }
    remember_valuation_origin(pangine, "outcome-positive-report", "episode-one-origin");
    remember_valuation_origin(pangine, "outcome-positive-report", "outcome-positive-report-origin");
    remember_origin_independence(pangine, "outcome-positive-report-origin", "episode-one-origin");
    for report in ["event-positive-report-one", "event-positive-report-two", "event-positive-report-three"] {
        remember_valuation_origin(pangine, report, "episode-three-origin");
    }
    for report in ["outcome-negative-report-one", "outcome-negative-report-two"] {
        remember_valuation_origin(pangine, report, "episode-two-origin");
    }
    remember_valuation_origin(pangine, "event-negative-report", "episode-four-origin");
    remember_valuation_origin(pangine, "mismatched-report", "mismatched-report-origin");
    remember_origin_independence(pangine, "mismatched-report-origin", "episode-one-origin");
}

fn complete_valuation_report_rows(pangine: &mut Pangine) -> BTreeMap<String, (String, String, String, Relevance)> {
    let report = pangine.reference_percept("inspected-valuation-report");
    let source = pangine.reference_percept("inspected-valuation-source");
    let consequence = pangine.reference_percept("inspected-valuation-consequence");
    let role = pangine.reference_percept("inspected-valuation-role");
    let sources = [pangine.reference_percept("valuation-reports"), pangine.reference_percept("valuation-control")];
    let question = must_ref(
        pangine,
        "(['inspected-valuation-report']->[report-source]->['inspected-valuation-source'])
         (['inspected-valuation-report']->[report-consequence]->['inspected-valuation-consequence'])
         (['inspected-valuation-report']->[report-role]->['inspected-valuation-role'])",
    );
    let result = pangine.complete_question(&sources, &question).expect("valid complete valuation-report question");
    let rows = result
        .completions()
        .iter()
        .map(|completion| {
            let source_concepts = completion.evidence().iter().map(|evidence| evidence.source_concept()).collect::<BTreeSet<_>>();
            assert_eq!(source_concepts.len(), 1, "every clause in one valuation report must come from the same complete row");
            let relevances = completion.evidence().iter().map(|evidence| evidence.source_relevance()).collect::<BTreeSet<_>>();
            assert_eq!(relevances.len(), 1, "one complete valuation report has one retained relevance");
            (
                pangine.format_concept(completion.binding(&report).expect("bound valuation report"), false),
                (
                    pangine.format_concept(completion.binding(&source).expect("bound valuation source"), false),
                    pangine.format_concept(completion.binding(&consequence).expect("bound valuation consequence"), false),
                    pangine.format_concept(completion.binding(&role).expect("bound valuation role"), false),
                    *relevances.iter().next().expect("one valuation-report relevance"),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(rows.len(), result.completions().len(), "valuation-report identities must remain distinct");
    rows
}

fn valuation_report_map(entries: &[(&str, &str, &str, &str, i64)]) -> BTreeMap<String, (String, String, String, Relevance)> {
    entries
        .iter()
        .map(|(report, source, consequence, role, relevance)| {
            (format!("[{report}]"), (format!("[{source}]"), format!("[{consequence}]"), format!("[{role}]"), Relevance::new(*relevance)))
        })
        .collect()
}

fn complete_question_order_episode_rows(pangine: &mut Pangine) -> BTreeSet<QuestionOrderEpisode> {
    let episode = pangine.reference_percept("inspected-episode");
    let context = pangine.reference_percept("inspected-context");
    let order = pangine.reference_percept("inspected-order");
    let consequence = pangine.reference_percept("inspected-consequence");
    let source = pangine.reference_percept("order-episodes");
    let question = must_ref(
        pangine,
        "(['inspected-episode']->[amber]->['inspected-context'])
         (['inspected-episode']->[violet]->['inspected-order'])
         (['inspected-episode']->[indigo]->['inspected-consequence'])",
    );
    let result = pangine.complete_question(std::slice::from_ref(&source), &question).expect("valid complete question-order episode question");
    let rows = result
        .completions()
        .iter()
        .map(|completion| {
            assert!(completion.evidence().iter().all(|evidence| evidence.source_percept() == Some(&source)));
            let source_concepts = completion.evidence().iter().map(|evidence| evidence.source_concept()).collect::<BTreeSet<_>>();
            assert_eq!(source_concepts.len(), 1, "every clause in one episode completion must come from the same remembered row");
            let relevances = completion.evidence().iter().map(|evidence| evidence.source_relevance()).collect::<BTreeSet<_>>();
            assert_eq!(relevances.len(), 1, "one complete remembered episode has one retained relevance");
            QuestionOrderEpisode {
                episode: pangine.format_concept(completion.binding(&episode).expect("bound inspected episode"), false),
                context: pangine.format_concept(completion.binding(&context).expect("bound inspected context"), false),
                order: pangine.format_concept(completion.binding(&order).expect("bound inspected order"), false),
                consequence: pangine.format_concept(completion.binding(&consequence).expect("bound inspected consequence"), false),
                relevance: *relevances.iter().next().expect("one episode relevance"),
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(rows.len(), result.completions().len(), "complete episode rows must remain distinct");
    rows
}

fn episode_pair_totals(
    rows: &BTreeSet<QuestionOrderEpisode>,
    key: impl Fn(&QuestionOrderEpisode) -> (String, String),
) -> BTreeMap<(String, String), Relevance> {
    let mut totals = BTreeMap::new();
    for row in rows {
        let key = key(row);
        let previous = totals.get(&key).copied().unwrap_or(Relevance::EMPTY);
        totals.insert(key, previous.checked_add(row.relevance).expect("episode pair total within signed relevance range"));
    }
    totals
}

fn episode_pair_map(entries: &[(&str, &str, i64)]) -> BTreeMap<(String, String), Relevance> {
    entries.iter().map(|(left, right, relevance)| (((*left).to_owned(), (*right).to_owned()), Relevance::new(*relevance))).collect()
}

fn run_episode_question_order_program(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['order-episodes']['episode-context']['requested-consequence'] @
           (['past-episode']->[amber]->['past-context'])
           (['past-episode']->[violet]->['episode-order'])
           (['past-episode']->[indigo]->['past-consequence'])
           ([request]->[cobalt]->['past-context'])
           ([request]->[saffron]->['past-consequence']);
         ['selected-episode-order'] = [selected-order]->^['episode-order'];
         ['question-order-record']['selected-episode-order'] @
           (['represented-episode-order']->[candidates]->['episode-order-state'])
           ([selected-order]->['represented-episode-order']);
         ['episode-order-choice'] = ^['episode-order-state']",
    );
}

fn set_consequence_stance(pangine: &mut Pangine, positive: &str, negative: &str) {
    clear_percept(pangine, "consequence-stance");
    experience(pangine, "consequence-stance", &format!("[{positive}]->[role]->[positive]"), 1);
    experience(pangine, "consequence-stance", &format!("[{negative}]->[role]->[negative]"), 1);
}

fn run_stance_question_order_program(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['order-episodes']['episode-context']['consequence-stance'] @
           (['positive-episode']->[amber]->['positive-context'])
           (['positive-episode']->[violet]->['positive-order'])
           (['positive-episode']->[indigo]->['positive-consequence'])
           ([request]->[cobalt]->['positive-context'])
           (['positive-consequence']->[role]->[positive]);
         ['order-episodes']['episode-context']['consequence-stance'] @
           (['negative-episode']->[amber]->['negative-context'])
           (['negative-episode']->[violet]->['negative-order'])
           (['negative-episode']->[indigo]->['negative-consequence'])
           ([request]->[cobalt]->['negative-context'])
           (['negative-consequence']->[role]->[negative]);
         ['episode-order-net'] = $['positive-order'];
         ['episode-order-net'] /= $['negative-order'];
         ['selected-episode-order'] = [selected-order]->^['episode-order-net'];
         ['question-order-record']['selected-episode-order'] @
           (['represented-episode-order']->[candidates]->['episode-order-state'])
           ([selected-order]->['represented-episode-order']);
         ['episode-order-choice'] = ^['episode-order-state']",
    );
}

fn run_valuation_report_question_order_program(pangine: &mut Pangine) {
    pangine
        .reference_concept(
            "['order-episodes']['episode-context']['valuation-reports']['valuation-control'] @
               (['positive-episode']->[amber]->['positive-context'])
               (['positive-episode']->[violet]->['positive-order'])
               (['positive-episode']->[indigo]->['positive-consequence'])
               ([request]->[cobalt]->['positive-context'])
               (['positive-report']->[report-source]->['positive-report-source'])
               (['positive-report']->[report-consequence]->['positive-consequence'])
               (['positive-report']->[report-role]->[positive]);
             ['order-episodes']['episode-context']['valuation-reports']['valuation-control'] @
               (['negative-episode']->[amber]->['negative-context'])
               (['negative-episode']->[violet]->['negative-order'])
               (['negative-episode']->[indigo]->['negative-consequence'])
               ([request]->[cobalt]->['negative-context'])
               (['negative-report']->[report-source]->['negative-report-source'])
               (['negative-report']->[report-consequence]->['negative-consequence'])
               (['negative-report']->[report-role]->[negative]);
             ['episode-order-net'] = $['positive-order'];
             ['episode-order-net'] /= $['negative-order'];
             ['selected-episode-order'] = [selected-order]->^['episode-order-net'];
             ['question-order-record']['selected-episode-order'] @
               (['represented-episode-order']->[candidates]->['episode-order-state'])
               ([selected-order]->['represented-episode-order']);
             ['episode-order-choice'] = ^['episode-order-state']",
        )
        .expect("valid source-identified valuation-report question-order program");
}

fn run_complete_valuation_observation_question_order_program(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['order-episodes']['episode-context']['valuation-observations']['valuation-observation-control']['valuation-current-one']['valuation-current-two']['valuation-current-three']['valuation-current-four'] @
           (['positive-observation']->[report-source]->['positive-observation-source'])
           (['positive-observation']->[reported-episode]->['positive-reported-episode'])
           (['positive-observation']->[reported-context]->['positive-context'])
           (['positive-observation']->[reported-order]->['positive-order'])
           (['positive-observation']->[reported-consequence]->['positive-consequence'])
           (['positive-observation']->[report-role]->[positive])
           (['positive-reported-episode']->[amber]->['positive-context'])
           (['positive-reported-episode']->[violet]->['positive-order'])
           (['positive-reported-episode']->[indigo]->['positive-consequence'])
           ([request]->[cobalt]->['positive-context']);
         ['order-episodes']['episode-context']['valuation-observations']['valuation-observation-control']['valuation-current-one']['valuation-current-two']['valuation-current-three']['valuation-current-four'] @
           (['negative-observation']->[report-source]->['negative-observation-source'])
           (['negative-observation']->[reported-episode]->['negative-reported-episode'])
           (['negative-observation']->[reported-context]->['negative-context'])
           (['negative-observation']->[reported-order]->['negative-order'])
           (['negative-observation']->[reported-consequence]->['negative-consequence'])
           (['negative-observation']->[report-role]->[negative])
           (['negative-reported-episode']->[amber]->['negative-context'])
           (['negative-reported-episode']->[violet]->['negative-order'])
           (['negative-reported-episode']->[indigo]->['negative-consequence'])
           ([request]->[cobalt]->['negative-context']);
         ['episode-order-net'] = $['positive-order'];
         ['episode-order-net'] /= $['negative-order'];
         ['selected-episode-order'] = [selected-order]->^['episode-order-net'];
         ['question-order-record']['selected-episode-order'] @
           (['represented-episode-order']->[candidates]->['episode-order-state'])
           ([selected-order]->['represented-episode-order']);
         ['episode-order-choice'] = ^['episode-order-state']",
    );
}

fn validate_complete_valuation_observations(pangine: &mut Pangine, observation_sources: &str) {
    let validation_question = "(['validation-observation']->[report-source]->['validation-source'])
         (['validation-observation']->[reported-episode]->['validation-episode'])
         (['validation-observation']->[reported-context]->['validation-context'])
         (['validation-observation']->[reported-order]->['validation-order'])
         (['validation-observation']->[reported-consequence]->['validation-consequence'])
         (['validation-observation']->[report-role]->['validation-role'])
         (['validation-episode']->[amber]->['validation-context'])
         (['validation-episode']->[violet]->['validation-order'])
         (['validation-episode']->[indigo]->['validation-consequence'])
         ([request]->[cobalt]->['validation-context'])";
    must_ref(pangine, &format!("['validated-observation-rows'] = (['order-episodes']['episode-context']{observation_sources} @ {validation_question})"));

    let key_question = "(['validation-key-observation']->[reported-episode]->['validation-key-episode'])
         (['validation-key-episode']->[amber]->['validation-key-context'])
         (['validation-key-episode']->[violet]->['validation-key-order'])
         (['validation-key-episode']->[indigo]->['validation-key-consequence'])";
    must_ref(pangine, &format!("['validated-observation-keys'] = (['validated-observation-rows'] @ {key_question})"));
}

fn run_staged_valuation_observation_question_order_program(pangine: &mut Pangine) {
    let observation_sources =
        "['valuation-observations']['valuation-observation-control']['valuation-current-one']['valuation-current-two']['valuation-current-three']['valuation-current-four']";
    validate_complete_valuation_observations(pangine, observation_sources);

    let staged_sources = format!("{observation_sources}['validated-observation-keys']['episode-context']");
    let positive_question = "(['staged-positive-observation']->[report-source]->['staged-positive-source'])
         (['staged-positive-observation']->[reported-episode]->['staged-positive-episode'])
         (['staged-positive-observation']->[reported-context]->['staged-positive-context'])
         (['staged-positive-observation']->[reported-order]->['staged-positive-order'])
         (['staged-positive-observation']->[reported-order]->['staged-positive-validation-order'])
         (['staged-positive-observation']->[reported-consequence]->['staged-positive-consequence'])
         (['staged-positive-observation']->[report-role]->[positive])
         (['staged-positive-episode']->[amber]->['staged-positive-context'])
         (['staged-positive-episode']->[violet]->['staged-positive-validation-order'])
         (['staged-positive-episode']->[indigo]->['staged-positive-consequence'])
         ([request]->[cobalt]->['staged-positive-context'])";
    must_ref(pangine, &format!("{staged_sources} @ {positive_question}"));

    let negative_question = "(['staged-negative-observation']->[report-source]->['staged-negative-source'])
         (['staged-negative-observation']->[reported-episode]->['staged-negative-episode'])
         (['staged-negative-observation']->[reported-context]->['staged-negative-context'])
         (['staged-negative-observation']->[reported-order]->['staged-negative-order'])
         (['staged-negative-observation']->[reported-order]->['staged-negative-validation-order'])
         (['staged-negative-observation']->[reported-consequence]->['staged-negative-consequence'])
         (['staged-negative-observation']->[report-role]->[negative])
         (['staged-negative-episode']->[amber]->['staged-negative-context'])
         (['staged-negative-episode']->[violet]->['staged-negative-validation-order'])
         (['staged-negative-episode']->[indigo]->['staged-negative-consequence'])
         ([request]->[cobalt]->['staged-negative-context'])";
    must_ref(pangine, &format!("{staged_sources} @ {negative_question}"));

    must_ref(
        pangine,
        "['staged-episode-order-net'] = $['staged-positive-order'];
         ['staged-episode-order-net'] /= $['staged-negative-order'];
         ['staged-selected-episode-order'] = [selected-order]->^['staged-episode-order-net'];
         ['question-order-record']['staged-selected-episode-order'] @
           (['staged-represented-episode-order']->[candidates]->['staged-episode-order-state'])
           ([selected-order]->['staged-represented-episode-order']);
         ['staged-episode-order-choice'] = ^['staged-episode-order-state']",
    );
}

fn run_provenance_routed_valuation_observation_question_order_program(pangine: &mut Pangine) {
    let observation_sources =
        "['valuation-observations']['valuation-observation-control']['valuation-current-one']['valuation-current-two']['valuation-current-three']['valuation-current-four']";
    validate_complete_valuation_observations(pangine, observation_sources);

    let independent_sources = format!("['order-episodes']['episode-context']{observation_sources}['valuation-provenance']");
    let independent_positive_question = "(['independent-positive-observation']->[report-source]->['independent-positive-source'])
         (['independent-positive-observation']->[reported-episode]->['independent-positive-episode'])
         (['independent-positive-observation']->[reported-context]->['independent-positive-context'])
         (['independent-positive-observation']->[reported-order]->['independent-positive-order'])
         (['independent-positive-observation']->[reported-consequence]->['independent-positive-consequence'])
         (['independent-positive-observation']->[report-role]->[positive])
         (['independent-positive-observation']->[origin]->['independent-positive-report-origin'])
         (['independent-positive-episode']->[origin]->['independent-positive-episode-origin'])
         (['independent-positive-report-origin']->[independent-of]->['independent-positive-episode-origin'])
         (['independent-positive-episode']->[amber]->['independent-positive-context'])
         (['independent-positive-episode']->[violet]->['independent-positive-order'])
         (['independent-positive-episode']->[indigo]->['independent-positive-consequence'])
         ([request]->[cobalt]->['independent-positive-context'])";
    must_run(pangine, &format!("{independent_sources} @ {independent_positive_question}"));

    let independent_negative_question = "(['independent-negative-observation']->[report-source]->['independent-negative-source'])
         (['independent-negative-observation']->[reported-episode]->['independent-negative-episode'])
         (['independent-negative-observation']->[reported-context]->['independent-negative-context'])
         (['independent-negative-observation']->[reported-order]->['independent-negative-order'])
         (['independent-negative-observation']->[reported-consequence]->['independent-negative-consequence'])
         (['independent-negative-observation']->[report-role]->[negative])
         (['independent-negative-observation']->[origin]->['independent-negative-report-origin'])
         (['independent-negative-episode']->[origin]->['independent-negative-episode-origin'])
         (['independent-negative-report-origin']->[independent-of]->['independent-negative-episode-origin'])
         (['independent-negative-episode']->[amber]->['independent-negative-context'])
         (['independent-negative-episode']->[violet]->['independent-negative-order'])
         (['independent-negative-episode']->[indigo]->['independent-negative-consequence'])
         ([request]->[cobalt]->['independent-negative-context'])";
    must_run(pangine, &format!("{independent_sources} @ {independent_negative_question}"));

    let same_occurrence_sources = format!("{observation_sources}['validated-observation-keys']['episode-context']['valuation-provenance']");
    let same_occurrence_positive_question = "(['same-occurrence-positive-observation']->[report-source]->['same-occurrence-positive-source'])
         (['same-occurrence-positive-observation']->[reported-episode]->['same-occurrence-positive-episode'])
         (['same-occurrence-positive-observation']->[reported-context]->['same-occurrence-positive-context'])
         (['same-occurrence-positive-observation']->[reported-order]->['same-occurrence-positive-order'])
         (['same-occurrence-positive-observation']->[reported-order]->['same-occurrence-positive-validation-order'])
         (['same-occurrence-positive-observation']->[reported-consequence]->['same-occurrence-positive-consequence'])
         (['same-occurrence-positive-observation']->[report-role]->[positive])
         (['same-occurrence-positive-observation']->[origin]->['same-occurrence-positive-origin'])
         (['same-occurrence-positive-episode']->[origin]->['same-occurrence-positive-origin'])
         (['same-occurrence-positive-episode']->[amber]->['same-occurrence-positive-context'])
         (['same-occurrence-positive-episode']->[violet]->['same-occurrence-positive-validation-order'])
         (['same-occurrence-positive-episode']->[indigo]->['same-occurrence-positive-consequence'])
         ([request]->[cobalt]->['same-occurrence-positive-context'])";
    must_run(pangine, &format!("{same_occurrence_sources} @ {same_occurrence_positive_question}"));

    let same_occurrence_negative_question = "(['same-occurrence-negative-observation']->[report-source]->['same-occurrence-negative-source'])
         (['same-occurrence-negative-observation']->[reported-episode]->['same-occurrence-negative-episode'])
         (['same-occurrence-negative-observation']->[reported-context]->['same-occurrence-negative-context'])
         (['same-occurrence-negative-observation']->[reported-order]->['same-occurrence-negative-order'])
         (['same-occurrence-negative-observation']->[reported-order]->['same-occurrence-negative-validation-order'])
         (['same-occurrence-negative-observation']->[reported-consequence]->['same-occurrence-negative-consequence'])
         (['same-occurrence-negative-observation']->[report-role]->[negative])
         (['same-occurrence-negative-observation']->[origin]->['same-occurrence-negative-origin'])
         (['same-occurrence-negative-episode']->[origin]->['same-occurrence-negative-origin'])
         (['same-occurrence-negative-episode']->[amber]->['same-occurrence-negative-context'])
         (['same-occurrence-negative-episode']->[violet]->['same-occurrence-negative-validation-order'])
         (['same-occurrence-negative-episode']->[indigo]->['same-occurrence-negative-consequence'])
         ([request]->[cobalt]->['same-occurrence-negative-context'])";
    must_run(pangine, &format!("{same_occurrence_sources} @ {same_occurrence_negative_question}"));

    must_ref(
        pangine,
        "['provenance-positive-order'] = $['independent-positive-order'];
         ['provenance-positive-order'] *= $['same-occurrence-positive-order'];
         ['provenance-negative-order'] = $['independent-negative-order'];
         ['provenance-negative-order'] *= $['same-occurrence-negative-order'];
         ['provenance-episode-order-net'] = $['provenance-positive-order'];
         ['provenance-episode-order-net'] /= $['provenance-negative-order'];
         ['provenance-selected-episode-order'] = [selected-order]->^['provenance-episode-order-net'];
         ['question-order-record']['provenance-selected-episode-order'] @
           (['provenance-represented-episode-order']->[candidates]->['provenance-episode-order-state'])
           ([selected-order]->['provenance-represented-episode-order']);
         ['provenance-episode-order-choice'] = ^['provenance-episode-order-state']",
    );
}

fn run_origin_first_valuation_observation_question_order_program(pangine: &mut Pangine) {
    let observation_sources =
        "['valuation-observations']['valuation-observation-control']['valuation-current-one']['valuation-current-two']['valuation-current-three']['valuation-current-four']";
    validate_complete_valuation_observations(pangine, observation_sources);
    let sources = format!("{observation_sources}['validated-observation-keys']['episode-context']['valuation-provenance']");

    let positive_origin_question = "(['origin-positive-observation']->[report-source]->['origin-positive-source'])
         (['origin-positive-observation']->[reported-episode]->['origin-positive-episode'])
         (['origin-positive-observation']->[reported-context]->['origin-positive-context'])
         (['origin-positive-observation']->[reported-order]->['origin-positive-order'])
         (['origin-positive-observation']->[reported-order]->['origin-positive-validation-order'])
         (['origin-positive-observation']->[reported-consequence]->['origin-positive-consequence'])
         (['origin-positive-observation']->[report-role]->[positive])
         (['origin-positive-observation']->[origin]->['origin-positive-origin'])
         (['origin-positive-episode']->[origin]->['origin-positive-episode-origin'])
         (['origin-positive-episode']->[amber]->['origin-positive-context'])
         (['origin-positive-episode']->[violet]->['origin-positive-validation-order'])
         (['origin-positive-episode']->[indigo]->['origin-positive-consequence'])
         ([request]->[cobalt]->['origin-positive-context'])";
    must_run(pangine, &format!("{sources} @ {positive_origin_question}"));

    let negative_origin_question = "(['origin-negative-observation']->[report-source]->['origin-negative-source'])
         (['origin-negative-observation']->[reported-episode]->['origin-negative-episode'])
         (['origin-negative-observation']->[reported-context]->['origin-negative-context'])
         (['origin-negative-observation']->[reported-order]->['origin-negative-order'])
         (['origin-negative-observation']->[reported-order]->['origin-negative-validation-order'])
         (['origin-negative-observation']->[reported-consequence]->['origin-negative-consequence'])
         (['origin-negative-observation']->[report-role]->[negative])
         (['origin-negative-observation']->[origin]->['origin-negative-origin'])
         (['origin-negative-episode']->[origin]->['origin-negative-episode-origin'])
         (['origin-negative-episode']->[amber]->['origin-negative-context'])
         (['origin-negative-episode']->[violet]->['origin-negative-validation-order'])
         (['origin-negative-episode']->[indigo]->['origin-negative-consequence'])
         ([request]->[cobalt]->['origin-negative-context'])";
    must_run(pangine, &format!("{sources} @ {negative_origin_question}"));

    must_ref(pangine, "['origin-first-selected-positive-origin'] = [selected-origin]->^['origin-positive-origin']");
    let selected_positive_sources = format!("['origin-first-selected-positive-origin']{sources}");
    let selected_positive_question = "([selected-origin]->['origin-first-positive-origin'])
         (['origin-first-positive-observation']->[report-source]->['origin-first-positive-source'])
         (['origin-first-positive-observation']->[reported-episode]->['origin-first-positive-episode'])
         (['origin-first-positive-observation']->[reported-context]->['origin-first-positive-context'])
         (['origin-first-positive-observation']->[reported-order]->['origin-first-positive-order'])
         (['origin-first-positive-observation']->[reported-order]->['origin-first-positive-validation-order'])
         (['origin-first-positive-observation']->[reported-consequence]->['origin-first-positive-consequence'])
         (['origin-first-positive-observation']->[report-role]->[positive])
         (['origin-first-positive-observation']->[origin]->['origin-first-positive-origin'])
         (['origin-first-positive-episode']->[origin]->['origin-first-positive-episode-origin'])
         (['origin-first-positive-episode']->[amber]->['origin-first-positive-context'])
         (['origin-first-positive-episode']->[violet]->['origin-first-positive-validation-order'])
         (['origin-first-positive-episode']->[indigo]->['origin-first-positive-consequence'])
         ([request]->[cobalt]->['origin-first-positive-context'])";
    must_run(pangine, &format!("{selected_positive_sources} @ {selected_positive_question}"));

    must_ref(pangine, "['origin-first-selected-negative-origin'] = [selected-origin]->^['origin-negative-origin']");
    let selected_negative_sources = format!("['origin-first-selected-negative-origin']{sources}");
    let selected_negative_question = "([selected-origin]->['origin-first-negative-origin'])
         (['origin-first-negative-observation']->[report-source]->['origin-first-negative-source'])
         (['origin-first-negative-observation']->[reported-episode]->['origin-first-negative-episode'])
         (['origin-first-negative-observation']->[reported-context]->['origin-first-negative-context'])
         (['origin-first-negative-observation']->[reported-order]->['origin-first-negative-order'])
         (['origin-first-negative-observation']->[reported-order]->['origin-first-negative-validation-order'])
         (['origin-first-negative-observation']->[reported-consequence]->['origin-first-negative-consequence'])
         (['origin-first-negative-observation']->[report-role]->[negative])
         (['origin-first-negative-observation']->[origin]->['origin-first-negative-origin'])
         (['origin-first-negative-episode']->[origin]->['origin-first-negative-episode-origin'])
         (['origin-first-negative-episode']->[amber]->['origin-first-negative-context'])
         (['origin-first-negative-episode']->[violet]->['origin-first-negative-validation-order'])
         (['origin-first-negative-episode']->[indigo]->['origin-first-negative-consequence'])
         ([request]->[cobalt]->['origin-first-negative-context'])";
    must_run(pangine, &format!("{selected_negative_sources} @ {selected_negative_question}"));

    must_ref(pangine, "['origin-first-episode-order-net'] = $['origin-first-positive-order']");
    must_run(pangine, "['origin-first-episode-order-net'] /= $['origin-first-negative-order']");
}

fn retrieve_origin_first_question_order_choice(pangine: &mut Pangine) {
    must_ref(pangine, "['origin-first-selected-episode-order'] = [selected-order]->^['origin-first-episode-order-net']");
    must_ref(
        pangine,
        "['question-order-record']['origin-first-selected-episode-order'] @
           (['origin-first-represented-episode-order']->[candidates]->['origin-first-episode-order-state'])
           ([selected-order]->['origin-first-represented-episode-order'])",
    );
    must_ref(pangine, "['origin-first-episode-order-choice'] = ^['origin-first-episode-order-state']");
}

fn run_all_origins_valuation_observation_question_order_program(pangine: &mut Pangine) {
    let observation_sources =
        "['valuation-observations']['valuation-observation-control']['valuation-current-one']['valuation-current-two']['valuation-current-three']['valuation-current-four']";
    validate_complete_valuation_observations(pangine, observation_sources);
    let stage_sources = format!("{observation_sources}['validated-observation-keys']['episode-context']['valuation-provenance']");

    let origin_report_question = "(['all-origin-stage-observation']->[report-source]->['all-origin-stage-source'])
         (['all-origin-stage-observation']->[reported-episode]->['all-origin-stage-episode'])
         (['all-origin-stage-observation']->[reported-context]->['all-origin-stage-context'])
         (['all-origin-stage-observation']->[reported-order]->['all-origin-stage-order'])
         (['all-origin-stage-observation']->[reported-order]->['all-origin-stage-validation-order'])
         (['all-origin-stage-observation']->[reported-consequence]->['all-origin-stage-consequence'])
         (['all-origin-stage-observation']->[report-role]->['all-origin-stage-role'])
         (['all-origin-stage-observation']->[origin]->['all-origin-stage-origin'])
         (['all-origin-stage-episode']->[origin]->['all-origin-stage-episode-origin'])
         (['all-origin-stage-episode']->[amber]->['all-origin-stage-context'])
         (['all-origin-stage-episode']->[violet]->['all-origin-stage-validation-order'])
         (['all-origin-stage-episode']->[indigo]->['all-origin-stage-consequence'])
         ([request]->[cobalt]->['all-origin-stage-context'])";
    must_ref(pangine, &format!("['all-origin-report-rows'] = ({stage_sources} @ {origin_report_question})"));

    must_ref(
        pangine,
        "['all-origin-projection-rows'] =
           (['all-origin-report-rows'] @
             (['all-origin-projected-report']->[report-role]->['all-origin-projected-role'])
             (['all-origin-projected-report']->[origin]->['all-origin']))",
    );

    let final_sources = format!("{observation_sources}['validated-observation-keys']['episode-context']['all-origin-projection-rows']");
    let positive_question = "(['all-origin-positive-observation']->[report-source]->['all-origin-positive-source'])
         (['all-origin-positive-observation']->[reported-episode]->['all-origin-positive-episode'])
         (['all-origin-positive-observation']->[reported-context]->['all-origin-positive-context'])
         (['all-origin-positive-observation']->[reported-order]->['all-origin-positive-order'])
         (['all-origin-positive-observation']->[reported-order]->['all-origin-positive-validation-order'])
         (['all-origin-positive-observation']->[reported-consequence]->['all-origin-positive-consequence'])
         (['all-origin-positive-observation']->[report-role]->[positive])
         (['all-origin-positive-observation']->[origin]->['all-origin-positive-origin'])
         (['all-origin-positive-episode']->[amber]->['all-origin-positive-context'])
         (['all-origin-positive-episode']->[violet]->['all-origin-positive-validation-order'])
         (['all-origin-positive-episode']->[indigo]->['all-origin-positive-consequence'])
         ([request]->[cobalt]->['all-origin-positive-context'])";
    must_run(pangine, &format!("{final_sources} @ {positive_question}"));

    let negative_question = "(['all-origin-negative-observation']->[report-source]->['all-origin-negative-source'])
         (['all-origin-negative-observation']->[reported-episode]->['all-origin-negative-episode'])
         (['all-origin-negative-observation']->[reported-context]->['all-origin-negative-context'])
         (['all-origin-negative-observation']->[reported-order]->['all-origin-negative-order'])
         (['all-origin-negative-observation']->[reported-order]->['all-origin-negative-validation-order'])
         (['all-origin-negative-observation']->[reported-consequence]->['all-origin-negative-consequence'])
         (['all-origin-negative-observation']->[report-role]->[negative])
         (['all-origin-negative-observation']->[origin]->['all-origin-negative-origin'])
         (['all-origin-negative-episode']->[amber]->['all-origin-negative-context'])
         (['all-origin-negative-episode']->[violet]->['all-origin-negative-validation-order'])
         (['all-origin-negative-episode']->[indigo]->['all-origin-negative-consequence'])
         ([request]->[cobalt]->['all-origin-negative-context'])";
    must_run(pangine, &format!("{final_sources} @ {negative_question}"));

    must_ref(pangine, "['all-origin-episode-order-net'] = $['all-origin-positive-order']");
    must_run(pangine, "['all-origin-episode-order-net'] /= $['all-origin-negative-order']");
}

fn retrieve_all_origins_question_order_choice(pangine: &mut Pangine) {
    must_ref(pangine, "['all-origin-selected-episode-order'] = [selected-order]->^['all-origin-episode-order-net']");
    must_ref(
        pangine,
        "['question-order-record']['all-origin-selected-episode-order'] @
           (['all-origin-represented-episode-order']->[candidates]->['all-origin-episode-order-state'])
           ([selected-order]->['all-origin-represented-episode-order'])",
    );
    must_ref(pangine, "['all-origin-episode-order-choice'] = ^['all-origin-episode-order-state']");
}

fn run_enclosed_origin_report_row_program(pangine: &mut Pangine) {
    let observation_sources = "['enclosed-valuation-observations']";
    let stage_sources = format!("{observation_sources}['order-episodes']['episode-context']");
    let stage_question = "(['enclosed-stage-report']->[observation]->(
           (['enclosed-stage-report']->[report-source]->['enclosed-stage-source'])
           (['enclosed-stage-report']->[reported-episode]->['enclosed-stage-episode'])
           (['enclosed-stage-report']->[reported-context]->['enclosed-stage-context'])
           (['enclosed-stage-report']->[reported-order]->['enclosed-stage-order'])
           (['enclosed-stage-report']->[reported-consequence]->['enclosed-stage-consequence'])
           (['enclosed-stage-report']->[report-role]->['enclosed-stage-role'])
           (['enclosed-stage-report']->[origins]->['enclosed-stage-origin-group'])))
         (['enclosed-stage-episode']->[amber]->['enclosed-stage-context'])
         (['enclosed-stage-episode']->[violet]->['enclosed-stage-order'])
         (['enclosed-stage-episode']->[indigo]->['enclosed-stage-consequence'])
         ([request]->[cobalt]->['enclosed-stage-context'])";
    must_ref(pangine, &format!("['enclosed-origin-report-rows'] = ({stage_sources} @ {stage_question})"));
    must_ref(pangine, "['enclosed-stage-origin-group'] @ (['enclosed-stage-origin-report']->[origin]->['enclosed-stage-origin'])");

    let recalled_question = "(['enclosed-recalled-report']->[observation]->(
           (['enclosed-recalled-report']->[report-source]->['enclosed-recalled-source'])
           (['enclosed-recalled-report']->[reported-episode]->['enclosed-recalled-episode'])
           (['enclosed-recalled-report']->[reported-context]->['enclosed-recalled-context'])
           (['enclosed-recalled-report']->[reported-order]->['enclosed-recalled-order'])
           (['enclosed-recalled-report']->[reported-consequence]->['enclosed-recalled-consequence'])
           (['enclosed-recalled-report']->[report-role]->['enclosed-recalled-role'])
           (['enclosed-recalled-report']->[origins]->['enclosed-recalled-origin-group'])))";
    must_ref(pangine, &format!("['enclosed-recalled-origin-report-rows'] = (['enclosed-origin-report-rows'] @ {recalled_question})"));
    must_ref(pangine, "['enclosed-recalled-origin-group'] @ (['enclosed-recalled-origin-report']->[origin]->['enclosed-recalled-origin'])");
}

fn prepare_referenced_origin_report_rows(pangine: &mut Pangine) {
    must_ref(pangine, "['origin-report-source-pointer'] = [origin-report-question]->[source]->['valuation-observations']");
    materialize_referenced_origin_report_rows(pangine);
}

fn materialize_referenced_origin_report_rows(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['referenced-origin-report-rows'] =
           (['all-origin-projection-rows']['origin-report-source-pointer'] @
             (['referenced-row-report']->[report-role]->['referenced-row-role'])
             (['referenced-row-report']->[origin]->['referenced-row-origin'])
             ([origin-report-question]->[source]->['referenced-row-source']))",
    );
    must_ref(pangine, "['origin-report-source-pointer'] @ [origin-report-question]->[source]->['referenced-original-source']");
}

fn run_referenced_origin_report_row_rejoin_program(pangine: &mut Pangine) {
    let sources = "^['referenced-original-source']['validated-observation-keys']['episode-context']['referenced-origin-report-rows']";
    let positive_question = "(['referenced-positive-report']->[report-source]->['referenced-positive-source'])
         (['referenced-positive-report']->[reported-episode]->['referenced-positive-episode'])
         (['referenced-positive-report']->[reported-context]->['referenced-positive-context'])
         (['referenced-positive-report']->[reported-order]->['referenced-positive-order'])
         (['referenced-positive-report']->[reported-order]->['referenced-positive-validation-order'])
         (['referenced-positive-report']->[reported-consequence]->['referenced-positive-consequence'])
         (['referenced-positive-report']->[report-role]->[positive])
         (['referenced-positive-report']->[origin]->['referenced-positive-origin'])
         (['referenced-positive-episode']->[amber]->['referenced-positive-context'])
         (['referenced-positive-episode']->[violet]->['referenced-positive-validation-order'])
         (['referenced-positive-episode']->[indigo]->['referenced-positive-consequence'])
         ([request]->[cobalt]->['referenced-positive-context'])";
    must_run(pangine, &format!("{sources} @ {positive_question}"));

    let negative_question = "(['referenced-negative-report']->[report-source]->['referenced-negative-source'])
         (['referenced-negative-report']->[reported-episode]->['referenced-negative-episode'])
         (['referenced-negative-report']->[reported-context]->['referenced-negative-context'])
         (['referenced-negative-report']->[reported-order]->['referenced-negative-order'])
         (['referenced-negative-report']->[reported-order]->['referenced-negative-validation-order'])
         (['referenced-negative-report']->[reported-consequence]->['referenced-negative-consequence'])
         (['referenced-negative-report']->[report-role]->[negative])
         (['referenced-negative-report']->[origin]->['referenced-negative-origin'])
         (['referenced-negative-episode']->[amber]->['referenced-negative-context'])
         (['referenced-negative-episode']->[violet]->['referenced-negative-validation-order'])
         (['referenced-negative-episode']->[indigo]->['referenced-negative-consequence'])
         ([request]->[cobalt]->['referenced-negative-context'])";
    must_run(pangine, &format!("{sources} @ {negative_question}"));

    must_ref(pangine, "['referenced-episode-order-net'] = $['referenced-positive-order']");
    must_run(pangine, "['referenced-episode-order-net'] /= $['referenced-negative-order']");
    must_ref(pangine, "['referenced-selected-episode-order'] = [selected-order]->^['referenced-episode-order-net']");
    must_ref(
        pangine,
        "['question-order-record']['referenced-selected-episode-order'] @
           (['referenced-represented-episode-order']->[candidates]->['referenced-episode-order-state'])
           ([selected-order]->['referenced-represented-episode-order'])",
    );
    must_ref(pangine, "['referenced-episode-order-choice'] = ^['referenced-episode-order-state']");
}

fn remember_source_scoped_report(pangine: &mut Pangine, memory: &str, report: &str, store: &str, answer: &str, repetitions: usize) {
    experience(pangine, memory, &format!("([{report}]->[report-store]->[{store}])([{report}]->[reported-answer]->[{answer}])"), repetitions);
}

fn remember_source_scoped_origin(pangine: &mut Pangine, row: &str, store: &str, report: &str, origin: &str) {
    experience(
        pangine,
        "source-scoped-report-origins",
        &format!("([{row}]->[origin-store]->[{store}])([{row}]->[origin-report]->[{report}])([{row}]->[origin]->[{origin}])"),
        1,
    );
}

fn materialize_source_scoped_origin_rows(pangine: &mut Pangine) {
    let sources = "['source-scoped-report-memory-a']['source-scoped-report-memory-b']['source-scoped-report-origins']";
    let stage_question = concat!(
        "(['source-scoped-stage-report']->[report-store]->['source-scoped-stage-store'])",
        "(['source-scoped-stage-origin-row']->[origin-store]->['source-scoped-stage-store'])",
        "(['source-scoped-stage-origin-row']->[origin-report]->['source-scoped-stage-report'])",
        "(['source-scoped-stage-origin-row']->[origin]->['source-scoped-stage-origin'])"
    );
    must_ref(pangine, &format!("['source-scoped-origin-stage-rows'] = ({sources} @ {stage_question})"));

    let projection_question = concat!(
        "{['source-scoped-projected-origin-row']->[origin-store]->['source-scoped-projected-store']}",
        "{['source-scoped-projected-origin-row']->[origin-report]->['source-scoped-projected-report']}",
        "{['source-scoped-projected-origin-row']->[origin]->['source-scoped-projected-origin']}"
    );
    must_ref(pangine, &format!("['source-scoped-origin-rows'] = (['source-scoped-origin-stage-rows'] @ {projection_question})"));
}

fn run_selected_source_scoped_report_question(pangine: &mut Pangine, prefix: &str, row_source: &str, require_store_match: bool) {
    let sources = format!("^['source-scoped-report-memory-choice']['{row_source}']");
    run_source_scoped_report_question(pangine, prefix, &sources, require_store_match);
}

fn run_source_scoped_report_question(pangine: &mut Pangine, prefix: &str, sources: &str, require_store_match: bool) {
    let report_store_output = if require_store_match { "store" } else { "report-store" };
    let origin_store_output = if require_store_match { "store" } else { "origin-store" };
    let report_store_clause = format!("{{['{prefix}-report']->[report-store]->['{prefix}-{report_store_output}']}}");
    let origin_store_clause = format!("{{['{prefix}-origin-row']->[origin-store]->['{prefix}-{origin_store_output}']}}");
    let answer_clause = format!("{{['{prefix}-report']->[reported-answer]->['{prefix}-answer']}}");
    let report_clause = format!("{{['{prefix}-origin-row']->[origin-report]->['{prefix}-report']}}");
    let origin_clause = format!("{{['{prefix}-origin-row']->[origin]->['{prefix}-origin']}}");
    let question = format!("{report_store_clause}{origin_store_clause}{answer_clause}{report_clause}{origin_clause}");
    must_run(pangine, &format!("{sources} @ {question}"));
}

fn populate_source_event_reports(pangine: &mut Pangine) {
    for (report, source) in [("report-a-one", "source-one"), ("report-a-two", "source-two"), ("report-a-three", "source-three")] {
        experience(pangine, "reports", &source_event_report(report, source, "opal", "shared-a-event", "A"), 1);
    }
    for (report, source, event) in [
        ("report-b-one", "source-four", "distinct-b-event-one"),
        ("report-b-two", "source-five", "distinct-b-event-two"),
        ("report-b-three", "source-six", "distinct-b-event-three"),
    ] {
        experience(pangine, "reports", &source_event_report(report, source, "opal", event, "B"), 1);
    }
    experience(pangine, "reports", &source_event_report("noise-report", "noise-source", "obsidian", "noise-event", "B"), 20);
    replace_state(pangine, "condition", "[lantern]->[cobalt]->[opal]");
}

fn opaque_detached_basis_firing(event: &str, candidate: &str, basis: &str) -> String {
    format!("({})([{event}]->[cobalt]->[{basis}])", opaque_firing(event, candidate))
}

fn opaque_embedded_basis_firing(event: &str, candidate: &str, basis: &str) -> String {
    format!("([{event}]->[amber]->[{basis}]->[quill])([{event}]->[violet]->[{basis}]->[{candidate}])")
}

fn populate_embedded_retained_views(pangine: &mut Pangine) {
    let accumulated_a = opaque_embedded_basis_firing("rune-one", "A", "cedar");
    let accumulated_b = [
        opaque_embedded_basis_firing("rune-two", "B", "cedar"),
        opaque_embedded_basis_firing("rune-three", "B", "cedar"),
        opaque_embedded_basis_firing("rune-four", "B", "cedar"),
    ];
    experience(pangine, "event-memory", &accumulated_a, 4);
    for row in &accumulated_b {
        experience(pangine, "event-memory", row, 1);
    }

    let current_a = opaque_embedded_basis_firing("rune-one", "A", "quartz");
    let current_b = [
        opaque_embedded_basis_firing("rune-two", "B", "quartz"),
        opaque_embedded_basis_firing("rune-three", "B", "quartz"),
        opaque_embedded_basis_firing("rune-four", "B", "quartz"),
    ];
    replace_state(pangine, "current-one", &current_a);
    for (percept, row) in ["current-two", "current-three", "current-four"].into_iter().zip(&current_b) {
        replace_state(pangine, percept, row);
    }
}

fn opaque_enclosed_basis_firing(event: &str, candidate: &str, basis: &str) -> String {
    format!("[{basis}]->[ochre]->(([{event}]->[amber]->[quill])([{event}]->[violet]->[{candidate}]))")
}

fn populate_enclosed_retained_views(pangine: &mut Pangine) {
    let accumulated_a = opaque_enclosed_basis_firing("rune-one", "A", "cedar");
    let accumulated_b = [
        opaque_enclosed_basis_firing("rune-two", "B", "cedar"),
        opaque_enclosed_basis_firing("rune-three", "B", "cedar"),
        opaque_enclosed_basis_firing("rune-four", "B", "cedar"),
    ];
    experience(pangine, "event-memory", &accumulated_a, 4);
    for row in &accumulated_b {
        experience(pangine, "event-memory", row, 1);
    }

    let current_a = opaque_enclosed_basis_firing("rune-one", "A", "quartz");
    let current_b = [
        opaque_enclosed_basis_firing("rune-two", "B", "quartz"),
        opaque_enclosed_basis_firing("rune-three", "B", "quartz"),
        opaque_enclosed_basis_firing("rune-four", "B", "quartz"),
    ];
    replace_state(pangine, "current-one", &current_a);
    for (percept, row) in ["current-two", "current-three", "current-four"].into_iter().zip(&current_b) {
        replace_state(pangine, percept, row);
    }
}

fn replace_state(pangine: &mut Pangine, percept: &str, state: &str) {
    must_ref(pangine, &format!("['{percept}'] = {state}"));
}

fn firing_histories(
    pangine: &mut Pangine,
    sources: &[ConceptId],
    question: &ConceptId,
    event: &ConceptId,
    choice: &ConceptId,
) -> BTreeMap<String, (String, String, Relevance)> {
    pangine
        .complete_question(sources, question)
        .expect("valid firing-history question")
        .completions()
        .iter()
        .map(|completion| {
            let evidence = completion.evidence().iter().find(|evidence| evidence.binding(choice).is_some()).expect("answer evidence for firing history");
            (
                pangine.format_concept(completion.binding(event).expect("bound firing event"), false),
                (
                    pangine.format_concept(completion.binding(choice).expect("bound firing choice"), false),
                    pangine.format_concept(evidence.source_percept().expect("firing source Percept"), false),
                    evidence.source_relevance(),
                ),
            )
        })
        .collect()
}

fn firing_history(choice: &str, source: &str, relevance: i64) -> (String, String, Relevance) {
    (choice.to_owned(), source.to_owned(), Relevance::new(relevance))
}

fn choice_source_names(pangine: &mut Pangine, result: &pangine::CompletionResult, choice: &ConceptId) -> BTreeSet<String> {
    result
        .completions()
        .iter()
        .filter_map(|completion| {
            completion
                .evidence()
                .iter()
                .find(|evidence| evidence.binding(choice).is_some())
                .and_then(|evidence| evidence.source_percept())
                .map(|source| pangine.format_concept(source, false))
        })
        .collect()
}

fn retained_view_source_names() -> BTreeSet<String> {
    ["['event-memory']", "['current-one']", "['current-two']", "['current-three']", "['current-four']"].into_iter().map(str::to_owned).collect()
}

fn binding_pairs(pangine: &Pangine, completions: &[Completion], left: &ConceptId, right: &ConceptId) -> BTreeSet<(String, String)> {
    completions
        .iter()
        .map(|completion| {
            (
                pangine.format_concept(completion.binding(left).expect("bound left pair member"), false),
                pangine.format_concept(completion.binding(right).expect("bound right pair member"), false),
            )
        })
        .collect()
}

fn decision_experience(event: &str, candidate: &str) -> String {
    format!("([{event}]->[signal]->[mark])([{event}]->[answer]->[{candidate}])")
}

fn answer_experience(event: &str, candidate: &str) -> String {
    format!("[{event}]->[answer]->[{candidate}]")
}

fn read_decision(pangine: &mut Pangine) -> DecisionResult {
    read_named_decision(pangine, "choice")
}

fn read_named_decision(pangine: &mut Pangine, choice_name: &str) -> DecisionResult {
    let candidates = read_named_weights(pangine, choice_name);
    let selected = pangine
        .reference_concept(&format!("^['{choice_name}']"))
        .expect("valid represented decision")
        .map(|candidate| pangine.format_concept(&candidate, false));
    DecisionResult { candidates, selected }
}

fn read_named_weights(pangine: &mut Pangine, percept_name: &str) -> BTreeMap<String, Relevance> {
    let percept = pangine.reference_percept(percept_name);
    pangine
        .get_value(&percept)
        .into_iter()
        .flat_map(|value| pangine.get_relevance_map(&value))
        .map(|(relevance, candidate)| (pangine.format_concept(&candidate, false), relevance))
        .collect()
}

fn candidate_map(entries: &[(&str, i64)]) -> BTreeMap<String, Relevance> {
    entries.iter().map(|(candidate, relevance)| ((*candidate).to_owned(), Relevance::new(*relevance))).collect()
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

fn must_run(pangine: &mut Pangine, input: &str) {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
