use super::super::{ConceptId, ConceptKind, ConceptMap, ExperienceRoots, Pangine, QuestionSnapshot};
use crate::Relevance;
use std::env;
use std::time::{Duration, Instant};

const DEFAULT_INGEST_SIZES: &str = "100,1000";
const DEFAULT_QUERY_SIZES: &str = "1000,10000,30000";
const DEFAULT_SAMPLES: usize = 3;

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Exact,
    Nested,
    Contextual,
}

impl Scenario {
    const ALL: [Self; 3] = [Self::Exact, Self::Nested, Self::Contextual];

    fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Nested => "nested",
            Self::Contextual => "contextual",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SnapshotWork {
    exact_roots: usize,
    match_views: usize,
    source_concepts: usize,
    graph_nodes: usize,
    graph_steps: usize,
    shape_candidates: usize,
}

struct Corpus {
    pangine: Pangine,
    source: ConceptId,
    question: ConceptId,
    output: ConceptId,
    expected: ConceptId,
    cases: usize,
    exact_roots: usize,
    fixture_time: Duration,
}

struct Measurement {
    scenario: Scenario,
    cases: usize,
    fixture_time: Duration,
    snapshot_time: Duration,
    projection_time: Duration,
    work: SnapshotWork,
}

#[test]
fn scaling_fixtures_preserve_expected_answers() {
    for scenario in Scenario::ALL {
        let corpus = build_corpus(scenario, 8);
        let measurement = measure(corpus, scenario, 1);
        assert!(measurement.work.exact_roots >= 8);
        assert!(measurement.work.shape_candidates >= 8);
    }
}

#[test]
#[ignore = "manual Release-mode scaling baseline"]
fn reports_question_scaling_baseline() {
    let ingest_sizes = configured_sizes("PANGINE_SCALING_INGEST_SIZES", DEFAULT_INGEST_SIZES);
    let query_sizes = configured_sizes("PANGINE_SCALING_QUERY_SIZES", DEFAULT_QUERY_SIZES);
    let samples = env::var("PANGINE_SCALING_SAMPLES")
        .ok()
        .map(|value| value.parse::<usize>().expect("PANGINE_SCALING_SAMPLES must be a positive integer"))
        .unwrap_or(DEFAULT_SAMPLES);
    assert!(samples > 0, "PANGINE_SCALING_SAMPLES must be positive");

    println!("incremental_ingest,experiences,total_ms,microseconds_per_experience");
    for &experiences in &ingest_sizes {
        let elapsed = measure_incremental_ingest(experiences);
        println!("incremental_ingest,{experiences},{:.3},{:.3}", milliseconds(elapsed), elapsed.as_secs_f64() * 1_000_000.0 / experiences as f64);
    }

    println!("repeat_after_unique,unique_roots,repeat_ms");
    for unique_roots in ingest_sizes {
        let elapsed = measure_repeat_after_unique(unique_roots);
        println!("repeat_after_unique,{unique_roots},{:.3}", milliseconds(elapsed));
    }

    println!(
        "question,cases,exact_roots,fixture_ms,snapshot_median_ms,projection_median_ms,match_views,source_concepts,graph_nodes,graph_steps,shape_candidates"
    );
    for cases in query_sizes {
        for scenario in Scenario::ALL {
            let measurement = measure(build_corpus(scenario, cases), scenario, samples);
            println!(
                "{},{},{},{:.3},{:.3},{:.3},{},{},{},{},{}",
                measurement.scenario.name(),
                measurement.cases,
                measurement.work.exact_roots,
                milliseconds(measurement.fixture_time),
                milliseconds(measurement.snapshot_time),
                milliseconds(measurement.projection_time),
                measurement.work.match_views,
                measurement.work.source_concepts,
                measurement.work.graph_nodes,
                measurement.work.graph_steps,
                measurement.work.shape_candidates
            );
        }
    }
}

fn configured_sizes(variable: &str, default: &str) -> Vec<usize> {
    let configured = env::var(variable).unwrap_or_else(|_| default.to_owned());
    let mut sizes = configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<usize>().unwrap_or_else(|_| panic!("{variable} contains invalid size {value:?}")))
        .collect::<Vec<_>>();
    assert!(!sizes.is_empty(), "{variable} must contain at least one size");
    assert!(sizes.iter().all(|&size| size > 0), "{variable} sizes must be positive");
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

fn measure_incremental_ingest(experiences: usize) -> Duration {
    let mut pangine = Pangine::new();
    let start = Instant::now();
    for index in 0..experiences {
        let input = format!("['world'] ~= [item-{index}]->[answer-{index}]");
        pangine
            .reference_concept(&input)
            .unwrap_or_else(|error| panic!("failed to parse scaling input {input:?}: {error}"))
            .unwrap_or_else(|| panic!("scaling input unexpectedly produced null: {input:?}"));
    }
    let elapsed = start.elapsed();
    let world = pangine.reference_percept("world");
    assert_eq!(pangine.get_percept_roots(&world).map(|roots| roots.len()), Some(experiences));
    elapsed
}

fn measure_repeat_after_unique(unique_roots: usize) -> Duration {
    let mut pangine = Pangine::new();
    for index in 0..unique_roots {
        let input = format!("['world'] ~= [item-{index}]->[answer-{index}]");
        pangine
            .reference_concept(&input)
            .unwrap_or_else(|error| panic!("failed to parse scaling input {input:?}: {error}"))
            .unwrap_or_else(|| panic!("scaling input unexpectedly produced null: {input:?}"));
    }

    let target = unique_roots / 2;
    let root_input = format!("[item-{target}]->[answer-{target}]");
    let root = pangine.reference_concept(&root_input).unwrap().unwrap();
    let repeat_input = format!("['world'] ~= {root_input}");
    let start = Instant::now();
    pangine.reference_concept(&repeat_input).unwrap().unwrap();
    let elapsed = start.elapsed();
    let world = pangine.reference_percept("world");
    assert_eq!(pangine.get_percept_root_count(&world, &root), Some(2));
    elapsed
}

fn measure(mut corpus: Corpus, scenario: Scenario, samples: usize) -> Measurement {
    let mut snapshot_times = Vec::with_capacity(samples);
    let mut projection_times = Vec::with_capacity(samples);
    let mut expected_work = None;

    for _ in 0..samples {
        let snapshot_start = Instant::now();
        let snapshot = corpus.pangine.question_snapshot(std::slice::from_ref(&corpus.source), &corpus.question);
        snapshot_times.push(snapshot_start.elapsed());

        let work = inspect_snapshot(&snapshot, &corpus.question, corpus.exact_roots);
        if let Some(expected_work) = &expected_work {
            assert_eq!(&work, expected_work, "snapshot work changed between identical questions");
        } else {
            expected_work = Some(work);
        }

        let projection_start = Instant::now();
        let results = corpus.pangine.get_projection_results(&corpus.question, &snapshot);
        projection_times.push(projection_start.elapsed());
        assert_eq!(results.get(&corpus.output).and_then(Option::as_ref), Some(&corpus.expected));
    }

    Measurement {
        scenario,
        cases: corpus.cases,
        fixture_time: corpus.fixture_time,
        snapshot_time: median(snapshot_times),
        projection_time: median(projection_times),
        work: expected_work.unwrap(),
    }
}

fn inspect_snapshot(snapshot: &QuestionSnapshot, question: &ConceptId, exact_roots: usize) -> SnapshotWork {
    SnapshotWork {
        exact_roots,
        match_views: snapshot.experiences.len(),
        source_concepts: snapshot.source_concepts.len(),
        graph_nodes: snapshot.graph.steps.len(),
        graph_steps: snapshot.graph.steps.values().map(Vec::len).sum(),
        shape_candidates: snapshot.experiences.iter().filter(|experience| experience.matched.0.shape() == question.0.shape()).count(),
    }
}

fn build_corpus(scenario: Scenario, cases: usize) -> Corpus {
    let start = Instant::now();
    let mut pangine = Pangine::new();
    let source = pangine.reference_percept("world");
    let output = pangine.reference_percept("answer");
    let target = cases / 2;
    let mut roots = ExperienceRoots::new();
    let mut target_first = None;
    let mut target_second = None;
    let mut expected = None;

    for index in 0..cases {
        let first = named(&mut pangine, &format!("item-{index}"));
        let second = named(&mut pangine, &format!("relation-{index}"));
        let answer = named(&mut pangine, &format!("answer-{index}"));

        match scenario {
            Scenario::Exact => {
                roots.insert(unordered(&pangine, [first.clone(), answer.clone()]), 1);
            }
            Scenario::Nested => {
                let relationship = ordered(&pangine, [first.clone(), second.clone(), answer.clone()]);
                let marker = named(&mut pangine, &format!("wrapper-{index}"));
                roots.insert(unordered(&pangine, [marker, relationship]), 1);
            }
            Scenario::Contextual => {
                let room = named(&mut pangine, &format!("room-{index}"));
                let sound = named(&mut pangine, &format!("sound-{index}"));
                roots.insert(ordered(&pangine, [first.clone(), second.clone(), room.clone()]), 1);
                roots.insert(ordered(&pangine, [room, sound.clone(), answer.clone()]), 1);
                if index == target {
                    target_second = Some(sound);
                }
            }
        }

        if index == target {
            target_first = Some(first);
            if !matches!(scenario, Scenario::Contextual) {
                target_second = Some(second);
            }
            expected = Some(answer);
        }
    }

    let exact_roots = roots.len();
    pangine.set_percept_roots(&source, roots).expect("scaling roots should produce a materialized source value");
    let target_first = target_first.unwrap();
    let target_second = target_second.unwrap();
    let expected = expected.unwrap();
    let question = match scenario {
        Scenario::Exact => unordered(&pangine, [target_first, output.clone()]),
        Scenario::Nested | Scenario::Contextual => ordered(&pangine, [target_first, target_second, output.clone()]),
    };

    Corpus { pangine, source, question, output, expected, cases, exact_roots, fixture_time: start.elapsed() }
}

fn named(pangine: &mut Pangine, name: &str) -> ConceptId {
    pangine.reference_named(name).unwrap_or_else(|| panic!("non-empty scaling name {name:?} should produce a Concept"))
}

fn ordered<const N: usize>(pangine: &Pangine, components: [ConceptId; N]) -> ConceptId {
    pangine.alloc(ConceptKind::Ordered { components: components.into() }, ConceptMap::new())
}

fn unordered<const N: usize>(pangine: &Pangine, concepts: [ConceptId; N]) -> ConceptId {
    let map = concepts.into_iter().map(|concept| (concept, Relevance::DEFAULT)).collect();
    pangine.alloc(ConceptKind::Unordered, map)
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
