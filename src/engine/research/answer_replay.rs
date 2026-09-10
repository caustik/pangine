//! Compares rebuilding an Answer from all retained episodes with carrying the
//! same open Answer forward one episode at a time. It also compares inverse
//! adjustment with replay after one mutable source is replaced or retracted.

use super::super::{concept_answer::ConceptAnswer, AnswerView, CompletionResult, ConceptId, Pangine};
use crate::Relevance;
use std::collections::BTreeSet;
use std::env;
use std::time::{Duration, Instant};

const DEFAULT_ANSWER_REPLAY_SIZES: &str = "10,100,1000";

const DECISION_QUESTION: &str = "
    ({candidate}->[action]->{action})
    ({candidate}->[tool]->{tool})";

const HELPFUL_QUESTION: &str = "
    ({helpful-episode}->[action]->{helpful-action})
    ({helpful-episode}->[tool]->{helpful-tool})
    ({helpful-episode}->[outcome]->[helpful])";

const FAILED_QUESTION: &str = "
    ({failed-episode}->[action]->{failed-action})
    ({failed-episode}->[tool]->{failed-tool})
    ({failed-episode}->[outcome]->[failed])";

const CANDIDATES: [(&str, &str); 3] = [("inspect-symbols", "dumpbin"), ("inspect-symbols", "link-map"), ("reconfigure", "cmake")];

#[derive(Clone, Copy)]
struct AnswerMetrics {
    rows: usize,
    evidence: usize,
    sources: usize,
}

struct ResearchQuestions {
    decision: ConceptId,
    decision_projection: ConceptId,
    helpful: ConceptId,
    helpful_projection: ConceptId,
    failed: ConceptId,
    failed_projection: ConceptId,
}

struct OutcomeViews {
    helpful: AnswerView,
    failed: AnswerView,
}

struct AnswerComparison {
    replay: AnswerMetrics,
    carried: AnswerMetrics,
    replay_bytes: usize,
    carried_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct VisibleAnswer {
    possibilities: Vec<(String, i64, usize, bool)>,
    selected: Option<String>,
}

impl ResearchQuestions {
    fn new(pangine: &mut Pangine) -> Self {
        Self {
            decision: must_ref(pangine, DECISION_QUESTION),
            decision_projection: must_ref(pangine, "{action}->{tool}"),
            helpful: must_ref(pangine, HELPFUL_QUESTION),
            helpful_projection: must_ref(pangine, "{helpful-action}->{helpful-tool}"),
            failed: must_ref(pangine, FAILED_QUESTION),
            failed_projection: must_ref(pangine, "{failed-action}->{failed-tool}"),
        }
    }
}

#[test]
#[ignore = "manual Release-mode full-replay versus carried-Answer report"]
fn carried_answer_matches_full_replay_with_constant_source_visits() {
    let checkpoints = configured_sizes();
    let last_checkpoint = *checkpoints.last().expect("nonempty checkpoints");
    let mut pangine = Pangine::new();
    let questions = ResearchQuestions::new(&mut pangine);

    let candidate_sources =
        CANDIDATES.iter().enumerate().map(|(index, &(action, tool))| remember_candidate(&mut pangine, index, action, tool)).collect::<Vec<_>>();
    let mut episode_sources =
        vec![remember_outcome(&mut pangine, "seed-helpful", CANDIDATES[0], "helpful"), remember_outcome(&mut pangine, "seed-failed", CANDIDATES[2], "failed")];

    let mut carried = answer_from_sources(&mut pangine, &candidate_sources, &questions.decision, &questions.decision_projection);
    for source in &episode_sources {
        carried = apply_episode(&mut pangine, &carried, source, &questions);
    }

    pangine.question_source_visits = 0;
    let mut replay_elapsed = Duration::ZERO;
    let mut carry_elapsed = Duration::ZERO;
    let mut replay_total_source_visits = 0;
    let mut carry_total_source_visits = 0;
    let mut prior_replay_source_visits = 0;
    let mut expected_carry_source_visits = None;

    println!(
        "answer_replay,cycles,replay_ms,carry_ms,replay_cycle_source_visits,carry_cycle_source_visits,replay_total_source_visits,carry_total_source_visits,concepts,rows,evidence,sources,selected"
    );

    for cycle in 1..=last_checkpoint {
        let source = remember_cycle_outcome(&mut pangine, cycle);
        episode_sources.push(source.clone());

        pangine.question_source_visits = 0;
        let replay_start = Instant::now();
        let replay = replay_answer(&mut pangine, &candidate_sources, &episode_sources, &questions);
        replay_elapsed += replay_start.elapsed();
        let replay_cycle_source_visits = pangine.question_source_visits;
        replay_total_source_visits += replay_cycle_source_visits;

        pangine.question_source_visits = 0;
        let carry_start = Instant::now();
        carried = apply_episode(&mut pangine, &carried, &source, &questions);
        carry_elapsed += carry_start.elapsed();
        let carry_cycle_source_visits = pangine.question_source_visits;
        carry_total_source_visits += carry_cycle_source_visits;

        assert_same_answer(&replay, &carried);
        let replay_choice = replay.choose(&mut pangine).expect("replay has a positive choice");
        let carry_choice = carried.choose(&mut pangine).expect("carried Answer has a positive choice");
        assert!(replay_choice.selected() == carry_choice.selected(), "replay and carry selected different values at cycle {cycle}");
        assert_same_answer(replay_choice.view(), carry_choice.view());

        let possibilities = carried.possibilities(&mut pangine).expect("carried possibilities");
        assert_eq!(possibilities.len(), CANDIDATES.len(), "the open carried Answer lost alternatives at cycle {cycle}");

        match expected_carry_source_visits {
            Some(expected) => assert_eq!(carry_cycle_source_visits, expected, "carry source visits changed at cycle {cycle}"),
            None => expected_carry_source_visits = Some(carry_cycle_source_visits),
        }
        assert!(replay_cycle_source_visits > prior_replay_source_visits, "full replay source visits did not grow at cycle {cycle}");
        prior_replay_source_visits = replay_cycle_source_visits;

        if checkpoints.binary_search(&cycle).is_ok() {
            let metrics = answer_metrics(&carried);
            let selected = pangine.format_concept(carry_choice.selected(), false);

            println!(
                "answer_replay,{},{:.3},{:.3},{},{},{},{},{},{},{},{},{}",
                cycle,
                replay_elapsed.as_secs_f64() * 1000.0,
                carry_elapsed.as_secs_f64() * 1000.0,
                replay_cycle_source_visits,
                carry_cycle_source_visits,
                replay_total_source_visits,
                carry_total_source_visits,
                pangine.concept_count(),
                metrics.rows,
                metrics.evidence,
                metrics.sources,
                selected,
            );
        }
    }

    assert!(replay_total_source_visits > carry_total_source_visits);
}

#[test]
#[ignore = "warning: inverse adjustment preserves counterevidence and is not source replacement or retraction"]
fn inverse_adjustment_matches_visible_results_but_not_replayed_provenance() {
    let mut pangine = Pangine::new();
    let questions = ResearchQuestions::new(&mut pangine);
    let candidate_sources =
        CANDIDATES.iter().enumerate().map(|(index, &(action, tool))| remember_candidate(&mut pangine, index, action, tool)).collect::<Vec<_>>();
    let episode_source = pangine.reference_percept("revisable-episode-source");
    let original_value = outcome_value(&mut pangine, "revisable-episode", CANDIDATES[0], "helpful");
    assert!(pangine.set_percept_value(&episode_source, Some(original_value)));
    let episode_sources = vec![episode_source.clone()];

    let base = answer_from_sources(&mut pangine, &candidate_sources, &questions.decision, &questions.decision_projection);
    let original = outcome_views(&mut pangine, &episode_source, &questions);
    let carried = apply_outcome_views(&mut pangine, &base, &original, Relevance::DEFAULT);

    let replacement = outcome_value(&mut pangine, "revisable-episode", CANDIDATES[1], "failed");
    assert!(pangine.set_percept_value(&episode_source, Some(replacement)));

    pangine.question_source_visits = 0;
    let replacement_replay = replay_answer(&mut pangine, &candidate_sources, &episode_sources, &questions);
    let replacement_replay_visits = pangine.question_source_visits;

    pangine.question_source_visits = 0;
    let replacement_views = outcome_views(&mut pangine, &episode_source, &questions);
    let replacement_carried = apply_outcome_views(&mut pangine, &carried, &original, Relevance::new(-1));
    let replacement_carried = apply_outcome_views(&mut pangine, &replacement_carried, &replacement_views, Relevance::DEFAULT);
    let replacement_carry_visits = pangine.question_source_visits;

    assert_ne!(visible_answer(&mut pangine, &carried), visible_answer(&mut pangine, &replacement_replay));
    let replacement_comparison = compare_visible_rewrite(&mut pangine, &replacement_replay, &replacement_carried);
    assert!(replacement_replay_visits > replacement_carry_visits);

    assert!(pangine.set_percept_value(&episode_source, None));

    pangine.question_source_visits = 0;
    let retraction_replay = replay_answer(&mut pangine, &candidate_sources, &episode_sources, &questions);
    let retraction_replay_visits = pangine.question_source_visits;
    assert_same_answer(&base, &retraction_replay);

    pangine.question_source_visits = 0;
    let retraction_carried = apply_outcome_views(&mut pangine, &carried, &original, Relevance::new(-1));
    let retraction_carry_visits = pangine.question_source_visits;

    let retraction_comparison = compare_visible_rewrite(&mut pangine, &retraction_replay, &retraction_carried);
    assert!(retraction_replay_visits > retraction_carry_visits);

    println!(
        "answer_rewrite,scenario,replay_source_visits,carry_source_visits,replay_evidence,carry_evidence,replay_sources,carry_sources,replay_bytes,carry_bytes\n\
         answer_rewrite,replacement,{replacement_replay_visits},{replacement_carry_visits},{},{},{},{},{},{}\n\
         answer_rewrite,retraction,{retraction_replay_visits},{retraction_carry_visits},{},{},{},{},{},{}",
        replacement_comparison.replay.evidence,
        replacement_comparison.carried.evidence,
        replacement_comparison.replay.sources,
        replacement_comparison.carried.sources,
        replacement_comparison.replay_bytes,
        replacement_comparison.carried_bytes,
        retraction_comparison.replay.evidence,
        retraction_comparison.carried.evidence,
        retraction_comparison.replay.sources,
        retraction_comparison.carried.sources,
        retraction_comparison.replay_bytes,
        retraction_comparison.carried_bytes,
    );
}

fn replay_answer(pangine: &mut Pangine, candidate_sources: &[ConceptId], episode_sources: &[ConceptId], questions: &ResearchQuestions) -> AnswerView {
    let answer = answer_from_sources(pangine, candidate_sources, &questions.decision, &questions.decision_projection);
    let helpful = answer_from_sources(pangine, episode_sources, &questions.helpful, &questions.helpful_projection);
    let failed = answer_from_sources(pangine, episode_sources, &questions.failed, &questions.failed_projection);
    let answer = answer.adjusted_by(pangine, &helpful, Relevance::DEFAULT).expect("helpful replay adjustment");
    answer.adjusted_by(pangine, &failed, Relevance::new(-1)).expect("failed replay adjustment")
}

fn apply_episode(pangine: &mut Pangine, answer: &AnswerView, source: &ConceptId, questions: &ResearchQuestions) -> AnswerView {
    let outcome = outcome_views(pangine, source, questions);
    apply_outcome_views(pangine, answer, &outcome, Relevance::DEFAULT)
}

fn outcome_views(pangine: &mut Pangine, source: &ConceptId, questions: &ResearchQuestions) -> OutcomeViews {
    let sources = std::slice::from_ref(source);
    let helpful = answer_from_sources(pangine, sources, &questions.helpful, &questions.helpful_projection);
    let failed = answer_from_sources(pangine, sources, &questions.failed, &questions.failed_projection);
    OutcomeViews { helpful, failed }
}

fn apply_outcome_views(pangine: &mut Pangine, answer: &AnswerView, outcome: &OutcomeViews, factor: Relevance) -> AnswerView {
    let answer = answer.adjusted_by(pangine, &outcome.helpful, factor).expect("helpful episode adjustment");
    answer.adjusted_by(pangine, &outcome.failed, factor.checked_neg().expect("unit adjustment factor")).expect("failed episode adjustment")
}

fn answer_from_sources(pangine: &mut Pangine, sources: &[ConceptId], question: &ConceptId, projection: &ConceptId) -> AnswerView {
    let result = pangine.complete_question(sources, question).expect("valid research question sources");
    AnswerView::from_result(pangine, result, projection.clone()).expect("projection belongs to research Answer")
}

fn assert_same_answer(left: &AnswerView, right: &AnswerView) {
    assert!(left.projection() == right.projection(), "replay and carry projections differ");
    assert!(same_completion_result(left.answer().result(), right.answer().result()), "replay and carry proof-bearing results differ");
}

fn compare_visible_rewrite(pangine: &mut Pangine, replay: &AnswerView, carried: &AnswerView) -> AnswerComparison {
    assert!(replay.projection() == carried.projection(), "replay and carried projections differ");
    assert_eq!(visible_answer(pangine, replay), visible_answer(pangine, carried));
    assert!(!same_completion_result(replay.answer().result(), carried.answer().result()), "inverse adjustment unexpectedly reproduced replay provenance");

    let replay_metrics = answer_metrics(replay);
    let carried_metrics = answer_metrics(carried);
    assert_eq!(replay_metrics.rows, carried_metrics.rows);
    assert!(carried_metrics.evidence > replay_metrics.evidence);
    assert!(carried_metrics.sources > replay_metrics.sources);

    let replay_bytes = encoded_answer_bytes(pangine, replay);
    let carried_bytes = encoded_answer_bytes(pangine, carried);
    assert!(carried_bytes > replay_bytes);
    AnswerComparison { replay: replay_metrics, carried: carried_metrics, replay_bytes, carried_bytes }
}

fn visible_answer(pangine: &mut Pangine, answer: &AnswerView) -> VisibleAnswer {
    let possibilities = answer
        .possibilities(pangine)
        .expect("answer possibilities")
        .into_iter()
        .map(|possibility| {
            (pangine.format_concept(possibility.value(), false), possibility.strength().weight(), possibility.complete_rows(), possibility.is_top_tie())
        })
        .collect();
    let selected = answer.choose(pangine).map(|choice| pangine.format_concept(choice.selected(), false));
    VisibleAnswer { possibilities, selected }
}

fn encoded_answer_bytes(pangine: &mut Pangine, answer: &AnswerView) -> usize {
    let concept_answer = ConceptAnswer::from_result(pangine, answer.answer().result());
    let encoded = concept_answer.encode(pangine);
    let text = pangine.format_concept(&encoded, false);
    let reparsed = must_ref(pangine, &text);
    assert_eq!(reparsed, encoded);
    let decoded = ConceptAnswer::decode(pangine, &reparsed).expect("encoded research Answer");
    let restored = decoded.to_result(pangine).expect("restored research Answer");
    let original = concept_answer.to_result(pangine).expect("original research Answer");
    assert!(same_completion_result(&restored, &original));
    text.len()
}

fn same_completion_result(left: &CompletionResult, right: &CompletionResult) -> bool {
    left.question() == right.question() && left.completions() == right.completions()
}

fn answer_metrics(answer: &AnswerView) -> AnswerMetrics {
    let result = answer.answer().result();
    let evidence = result.completions().iter().map(|completion| completion.evidence().len()).sum();
    let sources = result
        .completions()
        .iter()
        .flat_map(|completion| completion.evidence())
        .map(|evidence| (evidence.source_subject().clone(), evidence.source_concept().clone(), evidence.source_relevance(), evidence.source_contribution()))
        .collect::<BTreeSet<_>>()
        .len();

    AnswerMetrics { rows: result.completions().len(), evidence, sources }
}

fn configured_sizes() -> Vec<usize> {
    let configured = env::var("PANGINE_ANSWER_REPLAY_SIZES").unwrap_or_else(|_| DEFAULT_ANSWER_REPLAY_SIZES.to_owned());
    let mut sizes = configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<usize>().unwrap_or_else(|_| panic!("PANGINE_ANSWER_REPLAY_SIZES contains invalid size {value:?}")))
        .collect::<Vec<_>>();
    assert!(!sizes.is_empty(), "PANGINE_ANSWER_REPLAY_SIZES must contain at least one size");
    assert!(sizes.iter().all(|&size| size > 0), "PANGINE_ANSWER_REPLAY_SIZES sizes must be positive");
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

fn remember_cycle_outcome(pangine: &mut Pangine, cycle: usize) -> ConceptId {
    let candidate = CANDIDATES[(cycle - 1) % CANDIDATES.len()];
    let outcome = if ((cycle - 1) / CANDIDATES.len()).is_multiple_of(2) { "helpful" } else { "failed" };

    remember_outcome(pangine, &format!("episode-{cycle}"), candidate, outcome)
}

fn remember_candidate(pangine: &mut Pangine, index: usize, action: &str, tool: &str) -> ConceptId {
    let source = format!("candidate-source-{index}");
    must_ref(pangine, &format!("{{{source}}} ~= ([candidate-{index}]->[action]->[{action}])([candidate-{index}]->[tool]->[{tool}])"));
    pangine.reference_percept(&source)
}

fn remember_outcome(pangine: &mut Pangine, episode: &str, (action, tool): (&str, &str), outcome: &str) -> ConceptId {
    let source = format!("{episode}-source");
    must_ref(pangine, &format!("{{{source}}} ~= ([{episode}]->[action]->[{action}])([{episode}]->[tool]->[{tool}])([{episode}]->[outcome]->[{outcome}])"));
    pangine.reference_percept(&source)
}

fn outcome_value(pangine: &mut Pangine, episode: &str, (action, tool): (&str, &str), outcome: &str) -> ConceptId {
    must_ref(pangine, &format!("([{episode}]->[action]->[{action}])([{episode}]->[tool]->[{tool}])([{episode}]->[outcome]->[{outcome}])"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
