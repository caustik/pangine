use super::super::{ConceptId, LiveConceptAnswer, Pangine};
use std::collections::BTreeSet;
use std::env;
use std::time::Instant;

const DEFAULT_ANSWER_CYCLE_SIZES: &str = "10,100,1000";

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
    concepts: usize,
    nodes: usize,
    edges: usize,
    bytes: usize,
    rows: usize,
    evidence: usize,
    sources: usize,
    inspect_lines: usize,
    inspect_bytes: usize,
    revision: usize,
}

#[test]
#[ignore = "manual Release-mode repeated Concept answer lifecycle report"]
fn reports_repeated_answer_lifecycle() {
    let checkpoints = configured_sizes();
    let last_checkpoint = *checkpoints.last().expect("nonempty checkpoints");
    let mut pangine = decision_fixture();
    let mut total_source_visits = 0;
    let start = Instant::now();

    println!(
        "answer_lifecycle,cycles,total_ms,cycle_source_visits,total_source_visits,open_concepts,open_nodes,open_edges,open_bytes,open_rows,open_evidence,open_sources,open_inspect_lines,open_inspect_bytes,open_revision,chosen_concepts,chosen_nodes,chosen_edges,chosen_bytes,chosen_rows,chosen_evidence,chosen_sources,chosen_inspect_lines,chosen_inspect_bytes,chosen_revision"
    );

    for cycle in 1..=last_checkpoint {
        remember_cycle_outcome(&mut pangine, cycle);
        pangine.question_source_visits = 0;
        ask_and_adjust(&mut pangine);

        let cycle_source_visits = pangine.question_source_visits;
        total_source_visits += cycle_source_visits;
        let checkpoint = checkpoints.binary_search(&cycle).is_ok();
        let open = checkpoint.then(|| measure_answer(&mut pangine));

        choose(&mut pangine);

        if let Some(open) = open {
            let chosen = measure_answer(&mut pangine);
            assert_eq!(open.rows, CANDIDATES.len());
            assert_eq!(chosen.rows, 1);
            assert!(chosen.revision > open.revision);

            println!(
                concat!("answer_lifecycle,{},{:.3},{},{},", "{},{},{},{},{},{},{},{},{},{},", "{},{},{},{},{},{},{},{},{},{}"),
                cycle,
                start.elapsed().as_secs_f64() * 1000.0,
                cycle_source_visits,
                total_source_visits,
                open.concepts,
                open.nodes,
                open.edges,
                open.bytes,
                open.rows,
                open.evidence,
                open.sources,
                open.inspect_lines,
                open.inspect_bytes,
                open.revision,
                chosen.concepts,
                chosen.nodes,
                chosen.edges,
                chosen.bytes,
                chosen.rows,
                chosen.evidence,
                chosen.sources,
                chosen.inspect_lines,
                chosen.inspect_bytes,
                chosen.revision,
            );
        }
    }
}

fn configured_sizes() -> Vec<usize> {
    let configured = env::var("PANGINE_ANSWER_CYCLE_SIZES").unwrap_or_else(|_| DEFAULT_ANSWER_CYCLE_SIZES.to_owned());
    let mut sizes = configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<usize>().unwrap_or_else(|_| panic!("PANGINE_ANSWER_CYCLE_SIZES contains invalid size {value:?}")))
        .collect::<Vec<_>>();
    assert!(!sizes.is_empty(), "PANGINE_ANSWER_CYCLE_SIZES must contain at least one size");
    assert!(sizes.iter().all(|&size| size > 0), "PANGINE_ANSWER_CYCLE_SIZES sizes must be positive");
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

fn decision_fixture() -> Pangine {
    let mut pangine = Pangine::new();
    for (index, &(action, tool)) in CANDIDATES.iter().enumerate() {
        remember_candidate(&mut pangine, index, action, tool);
    }
    remember_outcome(&mut pangine, "seed-helpful", CANDIDATES[0], "helpful");
    remember_outcome(&mut pangine, "seed-failed", CANDIDATES[2], "failed");
    pangine
}

fn remember_cycle_outcome(pangine: &mut Pangine, cycle: usize) {
    let candidate = CANDIDATES[(cycle - 1) % CANDIDATES.len()];
    let outcome = if ((cycle - 1) / CANDIDATES.len()).is_multiple_of(2) { "helpful" } else { "failed" };

    remember_outcome(pangine, &format!("episode-{cycle}"), candidate, outcome);
}

fn remember_candidate(pangine: &mut Pangine, index: usize, action: &str, tool: &str) {
    must_ref(pangine, &format!("{{candidates}} ~= ([candidate-{index}]->[action]->[{action}])([candidate-{index}]->[tool]->[{tool}])"));
}

fn remember_outcome(pangine: &mut Pangine, episode: &str, (action, tool): (&str, &str), outcome: &str) {
    must_ref(pangine, &format!("{{episodes}} ~= ([{episode}]->[action]->[{action}])([{episode}]->[tool]->[{tool}])([{episode}]->[outcome]->[{outcome}])"));
}

fn ask_and_adjust(pangine: &mut Pangine) {
    must_ref(pangine, &format!("{{candidates}} @ {DECISION_QUESTION}"));
    must_ref(pangine, &format!("{{episodes}} @ {HELPFUL_QUESTION}"));
    must_ref(pangine, &format!("{{episodes}} @ {FAILED_QUESTION}"));
    must_ref(pangine, "{action}->{tool} @+= {helpful-action}->{helpful-tool}");
    must_ref(pangine, "{action}->{tool} @-= {failed-action}->{failed-tool}");
}

fn choose(pangine: &mut Pangine) {
    must_ref(pangine, "^({action}->{tool})");
}

fn measure_answer(pangine: &mut Pangine) -> AnswerMetrics {
    let shape = must_ref(pangine, "{action}->{tool}");
    let value = pangine.linked_answer_value(&shape).expect("linked answer value");
    let live = LiveConceptAnswer::decode(pangine, &value).expect("encoded live answer");
    let answer = pangine.answer_snapshot(&shape).expect("answer snapshot");
    let rows = answer.result().completions().len();
    let evidence = answer.result().completions().iter().map(|completion| completion.evidence().len()).sum();
    let sources = answer
        .result()
        .completions()
        .iter()
        .map(|completion| {
            completion
                .evidence()
                .iter()
                .map(|evidence| (evidence.source_percept().cloned(), evidence.source_concept().clone(), evidence.source_relevance()))
                .collect::<BTreeSet<_>>()
                .len()
        })
        .sum();
    let encoded = pangine.format_concept(&value, false);
    let inspection = pangine.debug_answer_inspection_lines("{action}->{tool}").expect("inspectable answer");
    let (nodes, edges) = reachable_shape(&value);

    AnswerMetrics {
        concepts: pangine.concept_count(),
        nodes,
        edges,
        bytes: encoded.len(),
        rows,
        evidence,
        sources,
        inspect_lines: inspection.len(),
        inspect_bytes: inspection.iter().map(String::len).sum(),
        revision: live.revision,
    }
}

fn reachable_shape(root: &ConceptId) -> (usize, usize) {
    let mut visited = BTreeSet::new();
    let mut pending = vec![root.clone()];
    let mut edges = 0;
    while let Some(concept) = pending.pop() {
        if !visited.insert(concept.clone()) {
            continue;
        }
        let children = concept.0.children().map(|(child, _)| child.clone()).collect::<Vec<_>>();
        edges += children.len();
        pending.extend(children);
    }
    (visited.len(), edges)
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
