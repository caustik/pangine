//! Warning check for applying recorded outcomes to a linked decision.
//!
//! The program gives each candidate and episode a stable identity. Helpful and
//! failed episodes are selected with ordinary questions, then their complete
//! decision outputs adjust the still-linked candidate answer. The outcome
//! meanings and additive effects are explicit program choices, not universal
//! Pangine semantics.
//!
//! A second check compares carrying every reviewed outcome into the final
//! decision with choosing one outcome first. It then changes the current input
//! and repeats the same three-Answer chain.

use pangine::{AnswerView, ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

const CANDIDATE_QUESTION: &str = "
    (['candidate']->[environment]->$['environment-input'])
    (['candidate']->[symptom]->$['symptom-input'])
    (['candidate']->[decision]->['decision'])";

const OUTCOME_QUESTION: &str = "
    (['episode']->[environment]->$['environment-input'])
    (['episode']->[symptom]->$['symptom-input'])
    (['episode']->[decision]->['episode-decision'])
    (['episode']->[outcome]->[helpful])";

const REVIEW_QUESTION: &str = "
    (['review']->[environment]->$['environment-input'])
    (['review']->[symptom]->$['symptom-input'])
    (['review']->[episode]->['trusted-episode'])
    (['review']->[assessment]->[trusted])";

#[test]
#[ignore = "warning: helpful-minus-failed linked support is one explicit outcome policy"]
fn linked_outcomes_adjust_complete_troubleshooting_choices_without_hiding_sources() {
    let mut pangine = Pangine::new();
    remember_candidate(&mut pangine, "candidate-clean", "clean-build", "cargo");
    remember_candidate(&mut pangine, "candidate-inspect", "inspect-symbols", "dumpbin");
    remember_candidate(&mut pangine, "candidate-reconfigure", "reconfigure", "cmake");

    remember_episode(&mut pangine, "episode-clean-failed-1", "clean-build", "cargo", "failed");
    remember_episode(&mut pangine, "episode-clean-failed-2", "clean-build", "cargo", "failed");
    remember_episode(&mut pangine, "episode-inspect-helpful", "inspect-symbols", "dumpbin", "helpful");

    must_ref(&mut pangine, "['environment-input'] = [windows]");
    must_ref(&mut pangine, "['symptom-input'] = [link-error]");
    must_ref(&mut pangine, &format!("['candidates'] @ {CANDIDATE_QUESTION}"));

    assert_eq!(must_ref(&mut pangine, "&['decision']"), must_ref(&mut pangine, CANDIDATE_QUESTION));
    assert_eq!(
        must_ref(&mut pangine, "$['decision']"),
        must_ref(&mut pangine, "([clean-build]->[cargo])([inspect-symbols]->[dumpbin])([reconfigure]->[cmake])")
    );

    ask_outcome_decisions(&mut pangine, "helpful", "helpful");
    ask_outcome_decisions(&mut pangine, "failed", "failed");

    let decision = pangine.reference_percept("decision");
    let helpful_decision = pangine.reference_percept("helpful-decision");
    let failed_decision = pangine.reference_percept("failed-decision");
    let candidates = pangine.answer_view(&decision).expect("candidate answer");
    let helpful = pangine.answer_view(&helpful_decision).expect("helpful answer");
    let failed = pangine.answer_view(&failed_decision).expect("failed answer");
    let adjusted = candidates.adjust(&mut pangine, &helpful, Relevance::DEFAULT).expect("helpful adjustment").into_view();
    let adjusted = adjusted.adjust(&mut pangine, &failed, Relevance::new(-1)).expect("failed adjustment").into_view();
    adjusted.answer().publish(&mut pangine).expect("current candidate revision");

    assert_eq!(must_ref(&mut pangine, "&['decision']"), must_ref(&mut pangine, CANDIDATE_QUESTION));
    assert_eq!(
        must_ref(&mut pangine, "$['decision']"),
        must_ref(&mut pangine, "x2([inspect-symbols]->[dumpbin])([reconfigure]->[cmake])!([clean-build]->[cargo])")
    );
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "x2[candidate-inspect][candidate-reconfigure]![candidate-clean]"));

    let answer = pangine.answer_snapshot(&decision).expect("published decision answer");
    let source_inventory = answer
        .result()
        .completions()
        .iter()
        .map(|completion| {
            let choice = pangine.format_concept(completion.binding(&decision).expect("decision binding"), false);
            let sources = completion
                .evidence()
                .iter()
                .map(|evidence| {
                    let source = pangine.format_concept(evidence.source_concept(), false);
                    (source, (evidence.source_relevance().weight(), evidence.source_contribution().weight()))
                })
                .collect::<BTreeMap<_, _>>();
            (choice, sources)
        })
        .collect::<BTreeMap<_, _>>();
    assert_sources(&source_inventory, "{[clean-build]->[cargo]}", &["candidate-clean", "episode-clean-failed-1", "episode-clean-failed-2"]);
    assert_sources(&source_inventory, "{[inspect-symbols]->[dumpbin]}", &["candidate-inspect", "episode-inspect-helpful"]);
    assert_sources(&source_inventory, "{[reconfigure]->[cmake]}", &["candidate-reconfigure"]);
    assert!(source_inventory["{[clean-build]->[cargo]}"]
        .iter()
        .filter(|(source, _)| source.contains("episode-clean-failed"))
        .all(|(_, relevance)| *relevance == (1, -1)));

    must_ref(&mut pangine, "['selected-decision'] = ^['decision']");
    assert_eq!(must_ref(&mut pangine, "$['selected-decision']"), must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]"));

    must_ref(
        &mut pangine,
        "['episodes'] @
           (['supporting-episode']->[environment]->$['environment-input'])
           (['supporting-episode']->[symptom]->$['symptom-input'])
           (['supporting-episode']->[decision]->($['selected-decision']))
           (['supporting-episode']->[outcome]->['supporting-outcome'])",
    );
    assert_eq!(must_ref(&mut pangine, "$['supporting-episode']"), must_ref(&mut pangine, "[episode-inspect-helpful]"));
    assert_eq!(must_ref(&mut pangine, "$['supporting-outcome']"), must_ref(&mut pangine, "[helpful]"));

    must_ref(&mut pangine, "['episodes'] @ [episode-clean-failed-1]->[outcome]->['recorded-failure']");
    assert_eq!(must_ref(&mut pangine, "$['recorded-failure']"), must_ref(&mut pangine, "[failed]"));
}

#[test]
#[ignore = "warning: early-versus-late choice depends on provisional additive support and canonical tie breaking"]
fn unresolved_outcomes_keep_aggregate_evidence_across_changing_inputs() {
    let mut pangine = Pangine::new();
    remember_context_candidate(&mut pangine, "candidate-a", "windows", "link-error", "A");
    remember_context_candidate(&mut pangine, "candidate-b", "windows", "link-error", "B");
    remember_context_episode(&mut pangine, "episode-a-1", "windows", "link-error", "A");
    remember_context_episode(&mut pangine, "episode-a-2", "windows", "link-error", "A");
    remember_context_episode(&mut pangine, "episode-b", "windows", "link-error", "B");
    remember_context_review(&mut pangine, "review-b", "windows", "link-error", "episode-b");

    remember_context_candidate(&mut pangine, "candidate-c", "linux", "runtime-error", "C");
    remember_context_candidate(&mut pangine, "candidate-d", "linux", "runtime-error", "D");
    remember_context_episode(&mut pangine, "episode-c", "linux", "runtime-error", "C");
    remember_context_episode(&mut pangine, "episode-d", "linux", "runtime-error", "D");
    remember_context_review(&mut pangine, "review-d", "linux", "runtime-error", "episode-d");

    set_current_input(&mut pangine, "windows", "link-error");
    let windows = compare_choice_timing(&mut pangine);

    assert_eq!(windows.late_choice, must_ref(&mut pangine, "[A]"));
    assert_eq!(windows.early_episode, must_ref(&mut pangine, "[episode-b]"));
    assert_eq!(windows.early_choice, must_ref(&mut pangine, "[B]"));
    assert_possibility(&mut pangine, &windows.late, "[A]", 3, true, &["candidate-a", "episode-a-1", "episode-a-2"]);
    assert_possibility(&mut pangine, &windows.late, "[B]", 3, true, &["candidate-b", "episode-b", "review-b"]);
    assert_possibility(&mut pangine, &windows.early, "[A]", 1, false, &["candidate-a"]);
    assert_possibility(&mut pangine, &windows.early, "[B]", 3, true, &["candidate-b", "episode-b", "review-b"]);

    set_current_input(&mut pangine, "linux", "runtime-error");
    let linux = compare_choice_timing(&mut pangine);

    assert_eq!(linux.late_choice, must_ref(&mut pangine, "[D]"));
    assert_eq!(linux.early_episode, must_ref(&mut pangine, "[episode-d]"));
    assert_eq!(linux.early_choice, must_ref(&mut pangine, "[D]"));
    assert_possibility(&mut pangine, &linux.late, "[C]", 2, false, &["candidate-c", "episode-c"]);
    assert_possibility(&mut pangine, &linux.late, "[D]", 3, true, &["candidate-d", "episode-d", "review-d"]);
    assert_possibility(&mut pangine, &linux.early, "[C]", 1, false, &["candidate-c"]);
    assert_possibility(&mut pangine, &linux.early, "[D]", 3, true, &["candidate-d", "episode-d", "review-d"]);

    let linux_sources = linux
        .late
        .possibilities(&mut pangine)
        .expect("current possibilities")
        .into_iter()
        .flat_map(|possibility| possibility.sources().iter().map(|source| pangine.format_concept(source.concept(), false)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert!(linux_sources.iter().all(|source| !source.contains("candidate-a") && !source.contains("episode-a") && !source.contains("review-b")));
}

struct ChoiceTiming {
    late: AnswerView,
    early: AnswerView,
    late_choice: ConceptId,
    early_episode: ConceptId,
    early_choice: ConceptId,
}

fn compare_choice_timing(pangine: &mut Pangine) -> ChoiceTiming {
    must_ref(pangine, &format!("['candidates'] @ {CANDIDATE_QUESTION}"));
    must_ref(pangine, &format!("['episodes'] @ {OUTCOME_QUESTION}"));
    must_ref(pangine, &format!("['reviews'] @ {REVIEW_QUESTION}"));

    let decision = pangine.reference_percept("decision");
    let episode = pangine.reference_percept("episode");
    let episode_decision = pangine.reference_percept("episode-decision");
    let trusted_episode = pangine.reference_percept("trusted-episode");
    let candidates = pangine.answer_view(&decision).expect("candidate answer");
    let outcomes = pangine.answer_view(&episode).expect("outcome answer");
    let trusted = pangine.answer_view(&trusted_episode).expect("review answer");

    let trusted_outcomes = outcomes.adjusted_by(pangine, &trusted, Relevance::DEFAULT).expect("matching reviewed episode");
    let all_outcome_decisions = trusted_outcomes.projecting(pangine, episode_decision.clone()).expect("all outcome decisions");
    let late = candidates.adjusted_by(pangine, &all_outcome_decisions, Relevance::DEFAULT).expect("all matching outcome decisions");
    let late_choice = late.choose(pangine).expect("late candidate choice").selected().clone();

    let chosen_outcome = trusted_outcomes.choose(pangine).expect("early outcome choice");
    let early_episode = chosen_outcome.selected().clone();
    let chosen_outcome_decision = chosen_outcome.view().projecting(pangine, episode_decision).expect("chosen outcome decision");
    let early = candidates.adjusted_by(pangine, &chosen_outcome_decision, Relevance::DEFAULT).expect("chosen matching outcome decision");
    let early_choice = early.choose(pangine).expect("early candidate choice").selected().clone();

    ChoiceTiming { late, early, late_choice, early_episode, early_choice }
}

fn remember_context_candidate(pangine: &mut Pangine, candidate: &str, environment: &str, symptom: &str, decision: &str) {
    must_ref(
        pangine,
        &format!(
            "['candidates'] ~= ([{candidate}]->[environment]->[{environment}])
                               ([{candidate}]->[symptom]->[{symptom}])
                               ([{candidate}]->[decision]->[{decision}])"
        ),
    );
}

fn remember_context_episode(pangine: &mut Pangine, episode: &str, environment: &str, symptom: &str, decision: &str) {
    must_ref(
        pangine,
        &format!(
            "['episodes'] ~= ([{episode}]->[environment]->[{environment}])
                             ([{episode}]->[symptom]->[{symptom}])
                             ([{episode}]->[decision]->[{decision}])
                             ([{episode}]->[outcome]->[helpful])"
        ),
    );
}

fn remember_context_review(pangine: &mut Pangine, review: &str, environment: &str, symptom: &str, episode: &str) {
    must_ref(
        pangine,
        &format!(
            "['reviews'] ~= ([{review}]->[environment]->[{environment}])
                            ([{review}]->[symptom]->[{symptom}])
                            ([{review}]->[episode]->[{episode}])
                            ([{review}]->[assessment]->[trusted])"
        ),
    );
}

fn set_current_input(pangine: &mut Pangine, environment: &str, symptom: &str) {
    must_ref(pangine, &format!("['environment-input'] = [{environment}]"));
    must_ref(pangine, &format!("['symptom-input'] = [{symptom}]"));
}

fn assert_possibility(
    pangine: &mut Pangine,
    view: &AnswerView,
    expected_value: &str,
    expected_strength: i64,
    expected_top_tie: bool,
    expected_sources: &[&str],
) {
    let possibility = view
        .possibilities(pangine)
        .expect("answer possibilities")
        .into_iter()
        .find(|possibility| pangine.format_concept(possibility.value(), false) == expected_value)
        .unwrap_or_else(|| panic!("missing possibility {expected_value}"));
    let sources = possibility.sources().iter().map(|source| pangine.format_concept(source.concept(), false)).collect::<BTreeSet<_>>();

    assert_eq!(possibility.strength().weight(), expected_strength, "unexpected strength for {expected_value}");
    assert_eq!(possibility.is_top_tie(), expected_top_tie, "unexpected tie state for {expected_value}");
    for expected_source in expected_sources {
        assert!(sources.iter().any(|source| source.contains(expected_source)), "missing {expected_source} from {expected_value}: {sources:?}");
    }
    assert_eq!(sources.len(), expected_sources.len(), "unexpected sources for {expected_value}: {sources:?}");
}

fn remember_candidate(pangine: &mut Pangine, candidate: &str, action: &str, tool: &str) {
    must_ref(
        pangine,
        &format!(
            "['candidates'] ~= ([{candidate}]->[environment]->[windows])
                               ([{candidate}]->[symptom]->[link-error])
                               ([{candidate}]->[decision]->([{action}]->[{tool}]))"
        ),
    );
}

fn remember_episode(pangine: &mut Pangine, episode: &str, action: &str, tool: &str, outcome: &str) {
    must_ref(
        pangine,
        &format!(
            "['episodes'] ~= ([{episode}]->[environment]->[windows])
                             ([{episode}]->[symptom]->[link-error])
                             ([{episode}]->[decision]->([{action}]->[{tool}]))
                             ([{episode}]->[outcome]->[{outcome}])"
        ),
    );
}

fn ask_outcome_decisions(pangine: &mut Pangine, role: &str, outcome: &str) {
    must_ref(
        pangine,
        &format!(
            "['episodes'] @
               (['{role}-episode']->[environment]->$['environment-input'])
               (['{role}-episode']->[symptom]->$['symptom-input'])
               (['{role}-episode']->[decision]->['{role}-decision'])
               (['{role}-episode']->[outcome]->[{outcome}])"
        ),
    );
}

fn assert_sources(inventory: &BTreeMap<String, BTreeMap<String, (i64, i64)>>, decision: &str, expected_sources: &[&str]) {
    let sources = &inventory[decision];
    for expected in expected_sources {
        assert!(sources.keys().any(|source| source.contains(expected)), "missing {expected} from {decision}: {sources:?}");
    }
    assert_eq!(sources.len(), expected_sources.len());
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
