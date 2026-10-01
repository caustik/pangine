//! Seeded property checks for the merge contract: dividing experience across
//! partitions, changing its arrival order, or regrouping partial answers must
//! not change an answer.

use super::*;
use crate::engine::completion::projection_strength;
use crate::engine::ConceptMap;

const SEEDS: u64 = 48;
const NODES: [&str; 4] = ["a", "b", "c", "d"];
const RELATIONS: [&str; 2] = ["r", "s"];
/// Questions every partition answers whole: single clauses, and clause groups
/// that share no blank, which only one experience can connect.
const WHOLE_QUESTIONS: [&str; 4] = ["{x}->[r]->{y}", "[a]->{relation}->{object}", "{whole}", "({x}->[r]->{y})({z}->[s]->{w})"];
/// Clauses joined by a shared blank, answered clause by clause in each
/// partition and joined after the clause answers merge.
const JOINED_QUESTIONS: [(&str, &str); 2] = [("{x}->[r]->{y}", "{y}->[s]->{z}"), ("{x}->[r]->{y}", "{x}->[s]->{z}")];

#[test]
fn arrival_order_does_not_change_memory_or_answers() {
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let mut events = experiences.iter().flat_map(|(experience, repetitions)| std::iter::repeat_n(experience.clone(), *repetitions)).collect::<Vec<_>>();
        let mut ordered = Pangine::new();
        record_all(&mut ordered, &events);
        Rng::new(seed ^ 0xA5A5).shuffle(&mut events);
        let mut shuffled = Pangine::new();
        record_all(&mut shuffled, &events);

        let mut reducer = Pangine::new();
        assert_eq!(memory_spelling(&mut ordered), memory_spelling(&mut shuffled), "seed {seed}");
        for question in WHOLE_QUESTIONS {
            let expected = transported_answer(&mut ordered, question, &mut reducer);
            let actual = transported_answer(&mut shuffled, question, &mut reducer);
            assert!(expected == actual, "seed {seed}: {question}");
        }
        for (first, second) in JOINED_QUESTIONS {
            let question = format!("({first})({second})");
            let expected = transported_answer(&mut ordered, &question, &mut reducer);
            let actual = transported_answer(&mut shuffled, &question, &mut reducer);
            assert!(expected == actual, "seed {seed}: {question}");
        }
    }
}

#[test]
fn partitioned_answers_merge_into_the_single_engine_answer_in_any_grouping() {
    let mut rows = [0; WHOLE_QUESTIONS.len()];
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let partitions = 2 + (seed as usize % 3);
        let (mut full, mut shards) = partitioned_engines(&experiences, partitions);
        let mut reducer = Pangine::new();

        for (index, question) in WHOLE_QUESTIONS.into_iter().enumerate() {
            let expected = transported_answer(&mut full, question, &mut reducer);
            rows[index] += expected.rows.len();
            let parts = shards.iter_mut().map(|shard| transported_answer(shard, question, &mut reducer)).collect::<Vec<_>>();

            let forward = merge_all(parts.iter());
            let reverse = merge_all(parts.iter().rev());
            let mut shuffled = parts.clone();
            Rng::new(seed).shuffle(&mut shuffled);
            let tree = merge_tree(&shuffled);
            assert!(forward == expected, "seed {seed}: {question} merged forward");
            assert!(reverse == expected, "seed {seed}: {question} merged in reverse");
            assert!(tree == expected, "seed {seed}: {question} merged as a tree");
            assert_eq!(forward.encode(&mut reducer), expected.encode(&mut reducer), "seed {seed}: {question} encoding");

            for (position, part) in parts.iter().enumerate() {
                assert!(part.merge_partitions(part).as_ref() == Some(part), "seed {seed}: merging is idempotent");
                let other = &parts[(position + 1) % parts.len()];
                assert!(part.merge_partitions(other) == other.merge_partitions(part), "seed {seed}: merging is commutative");
            }
        }
    }
    assert!(rows.iter().all(|rows| *rows > 0), "every question must have rows in some generated case: {rows:?}");
}

#[test]
fn joined_questions_reduce_from_partitioned_clause_answers() {
    let mut rows_across_partitions = 0;
    let mut rows_within_one_experience = 0;
    for seed in 0..SEEDS {
        let partitions = 2 + (seed as usize % 3);
        let experiences = generate(seed);
        let (mut full, mut shards) = partitioned_engines(&experiences, partitions);
        let mut reducer = Pangine::new();

        for (first, second) in JOINED_QUESTIONS {
            let question = format!("({first})({second})");
            let expected = transported_answer(&mut full, &question, &mut reducer);
            for row in &expected.rows {
                let sources = row.evidence().iter().map(|evidence| reducer.format_concept(evidence.source_concept(), false)).collect::<BTreeSet<_>>();
                if sources.len() == 1 {
                    rows_within_one_experience += 1;
                } else if sources.iter().map(|source| route(source, partitions)).collect::<BTreeSet<_>>().len() > 1 {
                    rows_across_partitions += 1;
                }
            }
            let first_parts = shards.iter_mut().map(|shard| transported_answer(shard, first, &mut reducer)).collect::<Vec<_>>();
            let second_parts = shards.iter_mut().map(|shard| transported_answer(shard, second, &mut reducer)).collect::<Vec<_>>();
            let first_answer = merge_all(first_parts.iter());
            let second_answer = merge_all(second_parts.iter());
            let Some(joined) = first_answer.join(&mut reducer, &second_answer) else {
                assert!(expected.rows.is_empty(), "seed {seed}: {question} joined to nothing");
                continue;
            };

            let shape = must_ref(&mut reducer, &question);
            let mut templates = vec![shape];
            templates.extend(expected.outputs.iter().cloned());
            for template in &templates {
                let label = reducer.format_concept(template, false);
                assert_eq!(readings(&mut reducer, &joined, template), readings(&mut reducer, &expected, template), "seed {seed}: {question} read as {label}");
            }
            assert_eq!(row_assignments(&reducer, &joined), row_assignments(&reducer, &expected), "seed {seed}: {question} rows");
        }
    }
    assert!(
        rows_across_partitions > 0 && rows_within_one_experience > 0,
        "the generated cases must join experiences across partitions and within one experience"
    );
}

#[test]
fn losing_a_partition_leaves_the_answer_to_the_remaining_experience() {
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let partitions = 2 + (seed as usize % 3);
        let lost = seed as usize % partitions;
        let (_, mut shards) = partitioned_engines(&experiences, partitions);
        let remaining = experiences.iter().filter(|(experience, _)| route(experience, partitions) != lost).cloned().collect::<Vec<_>>();
        let mut survivor = Pangine::new();
        record_experiences(&mut survivor, &remaining);
        let mut reducer = Pangine::new();

        for question in WHOLE_QUESTIONS {
            let expected = transported_answer(&mut survivor, question, &mut reducer);
            let parts = shards
                .iter_mut()
                .enumerate()
                .filter(|(index, _)| *index != lost)
                .map(|(_, shard)| transported_answer(shard, question, &mut reducer))
                .collect::<Vec<_>>();
            assert!(merge_all(parts.iter()) == expected, "seed {seed}: {question} without partition {lost}");
        }
    }
}

#[test]
fn adjustment_by_merged_evidence_matches_adjustment_by_each_partition_in_any_order() {
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let partitions = 2 + (seed as usize % 3);
        let (mut full, mut shards) = partitioned_engines(&experiences, partitions);
        let mut reducer = Pangine::new();

        let target = transported_answer(&mut full, "{x}->[r]->{y}", &mut reducer);
        let parts = shards.iter_mut().map(|shard| transported_answer(shard, "{p}->[r]->{q}", &mut reducer)).collect::<Vec<_>>();
        let target_template = must_ref(&mut reducer, "{x}->{y}");
        let adjustment_template = must_ref(&mut reducer, "{p}->{q}");
        let factor = if seed % 2 == 0 { Relevance::DEFAULT } else { Relevance::new(-1) };

        let merged = merge_all(parts.iter());
        let at_once = target.adjust(&mut reducer, &target_template, &merged, &adjustment_template, factor).expect("adjustment by merged evidence");
        let mut forward = target.clone();
        for part in &parts {
            forward = forward.adjust(&mut reducer, &target_template, part, &adjustment_template, factor).expect("adjustment by one partition");
        }
        let mut reverse = target.clone();
        for part in parts.iter().rev() {
            reverse = reverse.adjust(&mut reducer, &target_template, part, &adjustment_template, factor).expect("adjustment by one partition");
        }
        assert!(forward == at_once, "seed {seed}: forward adjustment");
        assert!(reverse == at_once, "seed {seed}: reverse adjustment");
    }
}

#[test]
fn memories_split_in_any_way_merge_by_adding_counts() {
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let mut full = Pangine::new();
        record_experiences(&mut full, &experiences);
        // Unlike answers, memories need no routing: every repetition may land
        // in any partition, because merging adds the counts back together.
        let partitions = 2 + (seed as usize % 3);
        let mut rng = Rng::new(seed ^ 0x5A5A);
        let mut shards = (0..partitions).map(|_| Pangine::new()).collect::<Vec<_>>();
        for (experience, repetitions) in &experiences {
            for _ in 0..*repetitions {
                remember(&mut shards[rng.below(partitions)], "memory", experience);
            }
        }
        let mut merged = Pangine::new();
        let memory = merged.reference_percept("memory");
        let mut subconcepts = ConceptMap::new();
        for shard in &mut shards {
            let shard_memory = shard.reference_percept("memory");
            for (relevance, experience) in shard.get_relevance_map(&shard_memory) {
                let spelling = shard.format_concept(&experience, false);
                let experience = must_ref(&mut merged, &spelling);
                let current = subconcepts.get(&experience).copied().unwrap_or(Relevance::EMPTY);
                subconcepts.insert(experience, current.checked_add(relevance).expect("merged count"));
            }
        }
        assert!(merged.set_percept_subconcepts(&memory, subconcepts).is_some(), "seed {seed}: merged memory");
        assert_eq!(memory_spelling(&mut merged), memory_spelling(&mut full), "seed {seed}: merged memory value");

        // The merged engine rebuilds its question index from the merged counts.
        let mut reducer = Pangine::new();
        for question in WHOLE_QUESTIONS.into_iter().chain(["({x}->[r]->{y})({y}->[s]->{z})"]) {
            let expected = transported_answer(&mut full, question, &mut reducer);
            let actual = transported_answer(&mut merged, question, &mut reducer);
            assert!(actual == expected, "seed {seed}: {question} over merged memory");
        }
    }
}

#[test]
fn answer_merging_requires_each_experience_in_one_partition() {
    let mut full = Pangine::new();
    let mut routed = Pangine::new();
    let mut halves = [Pangine::new(), Pangine::new()];
    for _ in 0..2 {
        for half in &mut halves {
            remember(half, "memory", "[a]->[r]->[b]");
            remember(&mut full, "memory", "[a]->[r]->[b]");
            remember(&mut routed, "memory", "[a]->[r]->[b]");
        }
    }
    let mut reducer = Pangine::new();
    let expected = transported_answer(&mut full, "{x}->[r]->{y}", &mut reducer);
    let y = reducer.reference_percept("y");
    assert_eq!(expected.materialize(&mut reducer, &y), Some(must_ref(&mut reducer, "x4[b]")));

    // Routed whole, the count stays in one partition and the merge keeps it.
    let empty = transported_answer(&mut Pangine::new(), "{x}->[r]->{y}", &mut reducer);
    let whole = transported_answer(&mut routed, "{x}->[r]->{y}", &mut reducer);
    assert_eq!(whole.merge_partitions(&empty).unwrap().materialize(&mut reducer, &y), Some(must_ref(&mut reducer, "x4[b]")));

    // Split evenly, both halves prove the same row from the same source, so
    // the set union keeps one of them and undercounts the evidence.
    let [first, second] = halves.each_mut().map(|half| transported_answer(half, "{x}->[r]->{y}", &mut reducer));
    assert_eq!(first.merge_partitions(&second).unwrap().materialize(&mut reducer, &y), Some(must_ref(&mut reducer, "x2[b]")));
}

#[test]
fn checked_overflow_is_decided_on_merged_rows_regardless_of_grouping() {
    let near_half = i64::MAX / 2 + 1;
    let experiences = ["[a]->[r]->[b]", "[c]->[r]->[b]", "[d]->[r]->[b]"];
    let mut shards = experiences
        .iter()
        .map(|experience| {
            let mut shard = Pangine::new();
            let memory = shard.reference_percept("memory");
            let experience = must_ref(&mut shard, experience);
            assert!(shard.set_percept_subconcepts(&memory, ConceptMap::from([(experience, Relevance::new(near_half))])).is_some());
            shard
        })
        .collect::<Vec<_>>();
    let mut reducer = Pangine::new();
    let parts = shards.iter_mut().map(|shard| transported_answer(shard, "{x}->[r]->{y}", &mut reducer)).collect::<Vec<_>>();
    let y = reducer.reference_percept("y");

    for part in &parts {
        assert!(part.materialize(&mut reducer, &y).is_some(), "one partition's evidence fits");
    }
    let groupings =
        [merge_all(parts.iter()), merge_all(parts.iter().rev()), parts[1].merge_partitions(&parts[0].merge_partitions(&parts[2]).unwrap()).unwrap()];
    for merged in &groupings {
        assert!(merged == &groupings[0]);
        assert!(merged.materialize(&mut reducer, &y).is_none(), "the merged count for [b] exceeds the signed 64-bit range");
    }
}

/// Generates a small memory: single relations, relations observed together,
/// ordered paths whose windows are relations, nested ordered paths that repeat
/// a relation at two positions, and inverted relations.
fn generate(seed: u64) -> Vec<(String, usize)> {
    let mut rng = Rng::new(seed);
    let count = 6 + rng.below(7);
    (0..count)
        .map(|_| {
            let experience = match rng.below(9) {
                0 | 1 => format!("({})({})", relation(&mut rng), relation(&mut rng)),
                2 => format!("[{}]->[r]->[{}]->[s]->[{}]", node(&mut rng), node(&mut rng), node(&mut rng)),
                3 => {
                    let repeated = relation(&mut rng);
                    format!("[{}]->({repeated})->({repeated})->({})", node(&mut rng), relation(&mut rng))
                }
                4 => format!("!({})", relation(&mut rng)),
                _ => relation(&mut rng),
            };
            (experience, 1 + rng.below(3))
        })
        .collect()
}

fn relation(rng: &mut Rng) -> String {
    format!("[{}]->[{}]->[{}]", node(rng), RELATIONS[rng.below(RELATIONS.len())], node(rng))
}

fn node(rng: &mut Rng) -> &'static str {
    NODES[rng.below(NODES.len())]
}

/// Builds one engine holding every experience and one engine per partition.
/// Each experience is routed by its canonical spelling, so its whole count
/// lives in exactly one partition.
fn partitioned_engines(experiences: &[(String, usize)], partitions: usize) -> (Pangine, Vec<Pangine>) {
    let mut full = Pangine::new();
    record_experiences(&mut full, experiences);
    let mut shards = (0..partitions).map(|_| Pangine::new()).collect::<Vec<_>>();
    for (experience, repetitions) in experiences {
        let shard = &mut shards[route(experience, partitions)];
        record_experiences(shard, &[(experience.clone(), *repetitions)]);
    }
    (full, shards)
}

fn route(experience: &str, partitions: usize) -> usize {
    let mut canonical = Pangine::new();
    let concept = must_ref(&mut canonical, experience);
    let spelling = canonical.format_concept(&concept, false);
    let hash = spelling.bytes().fold(0xCBF2_9CE4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3));
    (hash % partitions as u64) as usize
}

fn record_experiences(pangine: &mut Pangine, experiences: &[(String, usize)]) {
    for (experience, repetitions) in experiences {
        for _ in 0..*repetitions {
            remember(pangine, "memory", experience);
        }
    }
}

fn record_all(pangine: &mut Pangine, events: &[String]) {
    for experience in events {
        remember(pangine, "memory", experience);
    }
}

fn memory_spelling(pangine: &mut Pangine) -> String {
    let memory = pangine.reference_percept("memory");
    let value = pangine.get_value(&memory);
    let mut entries =
        pangine.get_relevance_map(&memory).into_iter().map(|(relevance, concept)| (pangine.format_concept(&concept, false), relevance)).collect::<Vec<_>>();
    entries.sort();
    format!("{} {entries:?}", value.map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(&value, false)))
}

fn transported_answer(from: &mut Pangine, question: &str, to: &mut Pangine) -> ConceptAnswer {
    let encoded = complete_answer(from, &["memory"], question);
    let spelling = from.format_concept(&encoded, false);
    let transported = must_ref(to, &spelling);
    ConceptAnswer::decode(to, &transported).expect("transported answer")
}

fn merge_all<'a>(parts: impl Iterator<Item = &'a ConceptAnswer>) -> ConceptAnswer {
    parts.cloned().reduce(|merged, part| merged.merge_partitions(&part).expect("partitions of one question")).expect("at least one partition")
}

fn merge_tree(parts: &[ConceptAnswer]) -> ConceptAnswer {
    match parts {
        [part] => part.clone(),
        _ => {
            let (left, right) = parts.split_at(parts.len() / 2);
            merge_tree(left).merge_partitions(&merge_tree(right)).expect("partitions of one question")
        }
    }
}

fn readings(pangine: &mut Pangine, answer: &ConceptAnswer, template: &ConceptId) -> Vec<(String, i64)> {
    let result = answer.to_result(pangine).expect("answer result");
    let support = pangine.completion_projection_support(&result, template).expect("projection support");
    support.iter().map(|(value, derivations)| (pangine.format_concept(value, false), projection_strength(derivations).expect("strength").count())).collect()
}

fn row_assignments(pangine: &Pangine, answer: &ConceptAnswer) -> Vec<String> {
    let mut rows = answer
        .rows
        .iter()
        .map(|row| {
            row.bindings()
                .map(|(percept, value)| format!("{}={}", pangine.format_concept(percept, false), pangine.format_concept(value, false)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn complete_answer(pangine: &mut Pangine, sources: &[&str], question: &str) -> ConceptId {
    let sources = sources.iter().map(|source| pangine.reference_percept(source)).collect::<Vec<_>>();
    let question = must_ref(pangine, question);
    let result = pangine.complete_question(&sources, &question).expect("valid question");
    ConceptAnswer::from_result(pangine, &result).encode(pangine)
}

fn remember(pangine: &mut Pangine, source: &str, concept: &str) {
    must_ref(pangine, &format!("{{{source}}} ~= {concept}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}

/// A small deterministic generator (splitmix64), so every case is reproducible
/// from its seed without a property-testing dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            items.swap(index, self.below(index + 1));
        }
    }
}
