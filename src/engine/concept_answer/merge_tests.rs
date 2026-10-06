//! Seeded property checks for the partition contract: dividing experience
//! among partitions, changing its arrival order, or regrouping partial answers
//! must not change an answer.

use super::*;
use crate::engine::test_rng::Rng;
use crate::engine::ConceptMap;

const SEEDS: u64 = 48;
const NODES: [&str; 4] = ["a", "b", "c", "d"];
const RELATIONS: [&str; 2] = ["r", "s"];
/// Single clauses, clause groups that share no blank, and clauses joined by a
/// shared blank.
const QUESTIONS: [&str; 6] = [
    "{x}->[r]->{y}",
    "[a]->{relation}->{object}",
    "{whole}",
    "({x}->[r]->{y})({z}->[s]->{w})",
    "({x}->[r]->{y})({y}->[s]->{z})",
    "({x}->[r]->{y})({x}->[s]->{z})",
];
/// Two clause groups, one of which joins two clauses through a shared blank.
/// One experience can connect the groups while another experience supplies
/// one of the joined clauses.
const JOINED_GROUP_QUESTIONS: [&str; 2] = ["({x}->[r]->{y})({y}->[s]->{z})({w}->[r]->{v})", "({x}->[r]->{y})({y}->[s]->{z})({w}->[s]->{v})"];

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
        for question in all_questions() {
            for graded in [false, true] {
                let expected = transported_answer(&mut ordered, question, graded, &mut reducer);
                let actual = transported_answer(&mut shuffled, question, graded, &mut reducer);
                assert!(expected == actual, "seed {seed}: {question}, graded {graded}");
            }
        }
    }
}

#[test]
fn partial_answers_reduce_to_the_single_engine_answer_in_any_grouping() {
    let mut coverage = Coverage::default();
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let partitions = 2 + (seed as usize % 6);
        let (mut full, mut shards) = partitioned_engines(&experiences, partitions);
        let mut reducer = Pangine::new();

        for question in all_questions() {
            for graded in [false, true] {
                let expected = transported_answer(&mut full, question, graded, &mut reducer);
                let parts = shards.iter_mut().map(|shard| transported_part(shard, question, graded, &mut reducer)).collect::<Vec<_>>();
                assert!(reduce(&mut reducer, question, parts.clone(), graded) == expected, "seed {seed}: {question}, graded {graded}");

                let mut shuffled = parts.clone();
                Rng::new(seed).shuffle(&mut shuffled);
                assert!(reduce(&mut reducer, question, [merge_tree(&shuffled)], graded) == expected, "seed {seed}: {question} merged as a tree");
                for (position, part) in parts.iter().enumerate() {
                    let other = &parts[(position + 1) % parts.len()];
                    assert!(part.clone().merge(part.clone()) == *part, "seed {seed}: merging is idempotent");
                    assert!(part.clone().merge(other.clone()) == other.clone().merge(part.clone()), "seed {seed}: merging is commutative");
                }
                coverage.count(&reducer, &expected, partitions, JOINED_GROUP_QUESTIONS.contains(&question));
            }
        }
    }
    coverage.assert_complete();
}

#[test]
fn partial_answers_keep_an_exact_row_that_whole_question_answers_lose() {
    let question = "({x}->[r]->{y})({y}->[s]->{z})({w}->[t]->[q])";
    let mut full = Pangine::new();
    let mut shards = ["[a]->[r]->[b]", "([b]->[s]->[c])([d]->[t]->[q])"].map(|experience| {
        remember(&mut full, "memory", experience);
        let mut shard = Pangine::new();
        remember(&mut shard, "memory", experience);
        shard
    });
    let mut reducer = Pangine::new();

    // The second experience connects the two clause groups, and the first
    // supplies the [r] clause through the shared blank.
    let expected = transported_answer(&mut full, question, false, &mut reducer);
    assert_eq!(expected.rows.iter().map(Completion::grade).collect::<Vec<_>>(), [CompletionGrade::Exact]);
    for shard in &mut shards {
        assert!(transported_answer(shard, question, false, &mut reducer).rows.is_empty(), "neither partition alone can answer the whole question");
    }
    let parts = shards.iter_mut().map(|shard| transported_part(shard, question, false, &mut reducer)).collect::<Vec<_>>();
    assert!(reduce(&mut reducer, question, parts, false) == expected);
}

#[test]
fn losing_a_partition_leaves_the_answer_to_the_remaining_experience() {
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let partitions = 2 + (seed as usize % 6);
        let lost = seed as usize % partitions;
        let (_, mut shards) = partitioned_engines(&experiences, partitions);
        let remaining = experiences.iter().filter(|(experience, _)| route(experience, partitions) != lost).cloned().collect::<Vec<_>>();
        let mut survivor = Pangine::new();
        record_experiences(&mut survivor, &remaining);
        let mut reducer = Pangine::new();

        for question in all_questions() {
            for graded in [false, true] {
                let expected = transported_answer(&mut survivor, question, graded, &mut reducer);
                let parts = shards
                    .iter_mut()
                    .enumerate()
                    .filter(|(index, _)| *index != lost)
                    .map(|(_, shard)| transported_part(shard, question, graded, &mut reducer))
                    .collect::<Vec<_>>();
                assert!(reduce(&mut reducer, question, parts, graded) == expected, "seed {seed}: {question}, graded {graded}, without partition {lost}");
            }
        }
    }
}

#[test]
fn adjustment_by_merged_evidence_matches_adjustment_by_each_partition_in_any_order() {
    for seed in 0..SEEDS {
        let experiences = generate(seed);
        let partitions = 2 + (seed as usize % 6);
        let (mut full, mut shards) = partitioned_engines(&experiences, partitions);
        let mut reducer = Pangine::new();

        let target = transported_answer(&mut full, "{x}->[r]->{y}", false, &mut reducer);
        let parts = shards.iter_mut().map(|shard| transported_part(shard, "{p}->[r]->{q}", false, &mut reducer)).collect::<Vec<_>>();
        let each = parts.iter().map(|part| reduce(&mut reducer, "{p}->[r]->{q}", [part.clone()], false)).collect::<Vec<_>>();
        let merged = reduce(&mut reducer, "{p}->[r]->{q}", parts, false);
        let target_template = must_ref(&mut reducer, "{x}->{y}");
        let adjustment_template = must_ref(&mut reducer, "{p}->{q}");
        let factor = if seed % 2 == 0 { Relevance::DEFAULT } else { Relevance::new(-1) };

        let at_once = target.adjust(&mut reducer, &target_template, &merged, &adjustment_template, factor).expect("adjustment by merged evidence");
        let mut forward = target.clone();
        for part in &each {
            forward = forward.adjust(&mut reducer, &target_template, part, &adjustment_template, factor).expect("adjustment by one partition");
        }
        let mut reverse = target.clone();
        for part in each.iter().rev() {
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
        let partitions = 2 + (seed as usize % 6);
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
                let experience = transport(shard, &experience, &mut merged);
                let current = subconcepts.get(&experience).copied().unwrap_or(Relevance::EMPTY);
                subconcepts.insert(experience, current.checked_add(relevance).expect("merged count"));
            }
        }
        assert!(merged.set_percept_subconcepts(&memory, subconcepts).is_some(), "seed {seed}: merged memory");
        assert_eq!(memory_spelling(&mut merged), memory_spelling(&mut full), "seed {seed}: merged memory value");

        // The merged engine rebuilds its question index from the merged counts.
        let mut reducer = Pangine::new();
        for question in all_questions() {
            for graded in [false, true] {
                let expected = transported_answer(&mut full, question, graded, &mut reducer);
                let actual = transported_answer(&mut merged, question, graded, &mut reducer);
                assert!(actual == expected, "seed {seed}: {question}, graded {graded}, over merged memory");
            }
        }
    }
}

#[test]
fn partial_answers_require_each_experience_in_one_partition() {
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
    let question = "{x}->[r]->{y}";
    let mut reducer = Pangine::new();
    let y = reducer.reference_percept("y");
    let expected = transported_answer(&mut full, question, false, &mut reducer);
    assert_eq!(expected.materialize(&mut reducer, &y), Some(must_ref(&mut reducer, "x4[b]")));

    // Routed whole, the count stays in one partition and the reduction keeps it.
    let whole = [transported_part(&mut routed, question, false, &mut reducer), transported_part(&mut Pangine::new(), question, false, &mut reducer)];
    assert_eq!(reduce(&mut reducer, question, whole, false).materialize(&mut reducer, &y), Some(must_ref(&mut reducer, "x4[b]")));

    // Split evenly, both halves prove the clause from the same source with the
    // same count, so the union keeps one fragment and undercounts the evidence.
    let halves = halves.each_mut().map(|half| transported_part(half, question, false, &mut reducer));
    assert_eq!(reduce(&mut reducer, question, halves, false).materialize(&mut reducer, &y), Some(must_ref(&mut reducer, "x2[b]")));
}

#[test]
fn checked_overflow_is_decided_on_merged_rows_regardless_of_grouping() {
    let near_half = i64::MAX / 2 + 1;
    let question = "{x}->[r]->{y}";
    let mut shards = ["[a]->[r]->[b]", "[c]->[r]->[b]", "[d]->[r]->[b]"]
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
    let parts = shards.iter_mut().map(|shard| transported_part(shard, question, false, &mut reducer)).collect::<Vec<_>>();
    let y = reducer.reference_percept("y");

    for part in &parts {
        assert!(reduce(&mut reducer, question, [part.clone()], false).materialize(&mut reducer, &y).is_some(), "one partition's evidence fits");
    }
    let groupings = [
        reduce(&mut reducer, question, parts.clone(), false),
        reduce(&mut reducer, question, parts.iter().rev().cloned(), false),
        reduce(&mut reducer, question, [parts[1].clone().merge(parts[0].clone().merge(parts[2].clone()))], false),
    ];
    for merged in &groupings {
        assert!(merged == &groupings[0]);
        assert!(merged.materialize(&mut reducer, &y).is_none(), "the merged count for [b] exceeds the signed 64-bit range");
    }
}

/// Counts the kinds of rows the generated cases produce, so the properties
/// cannot hold vacuously.
#[derive(Debug, Default)]
struct Coverage {
    /// Exact rows proven by experiences in different partitions.
    exact_across_partitions: usize,
    /// Exact rows of a joined-group question proven across partitions: the
    /// rows that answering whole questions in each partition loses.
    joined_group_across_partitions: usize,
    composed: usize,
    generalized: usize,
}

impl Coverage {
    fn count(&mut self, reducer: &Pangine, answer: &ConceptAnswer, partitions: usize, joined_group: bool) {
        for row in &answer.rows {
            match row.grade() {
                CompletionGrade::Composed => self.composed += 1,
                CompletionGrade::Generalized { .. } => self.generalized += 1,
                CompletionGrade::Exact => {
                    let routes = row
                        .evidence()
                        .iter()
                        .map(|evidence| route(&reducer.format_concept(evidence.source_concept(), false), partitions))
                        .collect::<BTreeSet<_>>();
                    if routes.len() > 1 {
                        self.exact_across_partitions += 1;
                        self.joined_group_across_partitions += usize::from(joined_group);
                    }
                }
            }
        }
    }

    fn assert_complete(&self) {
        assert!(
            self.exact_across_partitions > 0 && self.joined_group_across_partitions > 0 && self.composed > 0 && self.generalized > 0,
            "the generated cases must produce every kind of row: {self:?}"
        );
    }
}

fn all_questions() -> impl Iterator<Item = &'static str> {
    QUESTIONS.into_iter().chain(JOINED_GROUP_QUESTIONS)
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

// Routes as a partitioned engine does, by the experience's canonical spelling.
fn route(experience: &str, partitions: usize) -> usize {
    let mut canonical = Pangine::new();
    let concept = must_ref(&mut canonical, experience);
    crate::engine::partition::route(&canonical.format_concept(&concept, false), partitions)
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

/// Answers a question over one engine's whole memory and carries the answer
/// into the reducer.
fn transported_answer(from: &mut Pangine, question: &str, graded: bool, to: &mut Pangine) -> ConceptAnswer {
    let memory = from.reference_percept("memory");
    let question = must_ref(from, question);
    let result = if graded { from.complete_graded(&memory, &question) } else { from.complete(&memory, &question) }.expect("valid question");
    let encoded = ConceptAnswer::from_result(from, &result).encode(from);
    let transported = transport(from, &encoded, to);
    ConceptAnswer::decode(to, &transported).expect("transported answer")
}

/// Answers a question as one partition and carries the partial answer into
/// the reducer.
fn transported_part(from: &mut Pangine, question: &str, graded: bool, to: &mut Pangine) -> PartialAnswer {
    let memory = from.reference_percept("memory");
    let question = must_ref(from, question);
    let encoded = from.partial_answer(&[memory], &question, graded).expect("valid question").encode(from);
    let transported = transport(from, &encoded, to);
    PartialAnswer::decode(to, &transported).expect("transported partial answer")
}

fn transport(from: &Pangine, concept: &ConceptId, to: &mut Pangine) -> ConceptId {
    to.import_graph(&from.export_graph(concept).expect("owned Concept")).expect("well-formed graph")
}

/// Reduces partial answers in the reducer, through the codec so rows compare
/// in canonical order.
fn reduce(reducer: &mut Pangine, question: &str, parts: impl IntoIterator<Item = PartialAnswer>, graded: bool) -> ConceptAnswer {
    let question = must_ref(reducer, question);
    let result = reducer.reduce_partial_answers(&question, parts, graded).expect("owned question");
    let encoded = ConceptAnswer::from_result(reducer, &result).encode(reducer);
    ConceptAnswer::decode(reducer, &encoded).expect("reduced answer")
}

fn merge_tree(parts: &[PartialAnswer]) -> PartialAnswer {
    match parts {
        [] => PartialAnswer::default(),
        [part] => part.clone(),
        _ => {
            let (left, right) = parts.split_at(parts.len() / 2);
            merge_tree(left).merge(merge_tree(right))
        }
    }
}

// An experience and its inversion can cancel in a memory's value, so
// remembering may return no Concept.
fn remember(pangine: &mut Pangine, source: &str, concept: &str) {
    pangine.reference_concept(&format!("{{{source}}} ~= {concept}")).unwrap_or_else(|error| panic!("failed to parse {concept:?}: {error}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
