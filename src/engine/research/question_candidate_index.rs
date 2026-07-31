//! Research-only candidate retrieval for the current question matcher.
//!
//! The production question path currently unwraps every stored Observation
//! before grouping payloads by broad Concept shape. This oracle gives each
//! partition a disposable lookup derived only from the exact Observation
//! records in that partition. Index state never crosses the map/reduce boundary:
//! mapping returns exact source records, and reduction unions those records.
//! The authoritative partition gates every lookup result, so a stale posting
//! cannot manufacture an Observation that is no longer present.
//!
//! Shape is deliberately coarse. Today's fuzzy matcher can bind an output from
//! a structurally compatible Correlation even when every fixed Named Concept is
//! different. A literal-only index would therefore change current answers. The
//! oracle records that limitation rather than promoting shape postings as the
//! eventual production index.

use super::super::*;

#[derive(Default)]
struct PartitionCandidateIndex {
    record_keys: BTreeSet<ConceptId>,
    payload_postings: BTreeMap<ConceptShape, BTreeSet<ConceptId>>,
}

struct IndexedQuestionResult {
    answers: BTreeMap<ConceptId, Option<ConceptId>>,
    stored_record_count: usize,
    selected_record_count: usize,
    matcher_input_count: usize,
}

impl PartitionCandidateIndex {
    fn from_state(state: &ConceptId) -> Result<Self, &'static str> {
        let mut index = Self::default();
        for (record, relevance) in state.observation_records().ok_or("candidate index source is not Observation state")? {
            index.ingest_record(record, relevance)?;
        }
        Ok(index)
    }

    fn ingest_record(&mut self, record: ConceptId, relevance: Relevance) -> Result<bool, &'static str> {
        if relevance != Relevance::DEFAULT {
            return Err("candidate index source has structural relevance");
        }
        let ConceptKind::Observation { observation, .. } = &record.0.kind else {
            return Err("candidate index source contains a non-Observation");
        };
        if !self.record_keys.insert(record.clone()) {
            return Ok(false);
        }

        self.payload_postings.entry(observation.0.shape()).or_default().insert(record);
        Ok(true)
    }

    fn candidate_records(&self, pangine: &Pangine, state: &ConceptId, question: &ConceptId) -> ConceptMap {
        let authoritative_records = state.observation_records().unwrap_or_default();
        let include_records = matches!(question.0.kind, ConceptKind::Observation { .. });
        let mut patterns = ConceptMap::new();
        let mut contains_percept_cache = BTreeMap::new();
        pangine.collect_question_patterns(question, Relevance::DEFAULT, true, &mut patterns, &mut contains_percept_cache);

        let mut candidates = ConceptMap::new();
        for pattern in patterns.keys() {
            if pangine.is_percept(pattern) || (include_records && matches!(pattern.0.kind, ConceptKind::Observation { .. })) {
                return self.record_keys.iter().filter_map(|record| authoritative_records.get(record).map(|relevance| (record.clone(), *relevance))).collect();
            }
            if let Some(postings) = self.payload_postings.get(&pattern.0.shape()) {
                for record in postings {
                    if let Some(relevance) = authoritative_records.get(record) {
                        candidates.insert(record.clone(), *relevance);
                    }
                }
            }
        }
        candidates
    }

    fn ask(&self, pangine: &mut Pangine, state: &ConceptId, question: &ConceptId) -> IndexedQuestionResult {
        let records = self.candidate_records(pangine, state, question);
        let include_records = matches!(question.0.kind, ConceptKind::Observation { .. });
        let experiences = pangine.question_experiences_from_records(records.clone(), include_records);
        IndexedQuestionResult {
            answers: pangine.get_projection_results(question, &experiences),
            stored_record_count: self.record_keys.len(),
            selected_record_count: records.len(),
            matcher_input_count: experiences.len(),
        }
    }
}

fn must_reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn experience_state(pangine: &mut Pangine, memory: &str, experiences: &[String]) -> ConceptId {
    for experience in experiences {
        must_reference(pangine, &format!("['{memory}'] ~= {experience}"));
    }
    let memory = must_reference(pangine, &format!("['{memory}']"));
    pangine.get_value(&memory).unwrap()
}

fn full_scan_answers(pangine: &mut Pangine, state: &ConceptId, question: &ConceptId) -> BTreeMap<ConceptId, Option<ConceptId>> {
    let experiences = pangine.question_experience_map(state.clone(), question);
    pangine.get_projection_results(question, &experiences)
}

fn answers_from_records(pangine: &mut Pangine, records: ConceptMap, question: &ConceptId) -> BTreeMap<ConceptId, Option<ConceptId>> {
    let include_records = matches!(question.0.kind, ConceptKind::Observation { .. });
    let experiences = pangine.question_experiences_from_records(records, include_records);
    pangine.get_projection_results(question, &experiences)
}

fn reduce_candidate_records(partials: &[ConceptMap]) -> ConceptMap {
    let mut records = ConceptMap::new();
    for partial in partials {
        for (record, relevance) in partial {
            records.insert(record.clone(), *relevance);
        }
    }
    records
}

#[test]
fn shape_postings_reconstruct_current_matcher_answers() {
    for (experiences, question_source) in [
        (vec!["?[event-1]:{[weather]->[rain]}", "?[noise]:[unrelated]"], "{[weather]->['answer']}"),
        (
            vec!["?[event-1]:{{{[choice]->[morning]}->[workday]}->[tea]}", "?[event-2]:{{[choice]->[evening]}->[coffee]}"],
            "{{{[choice]->[morning]}->[workday]}->['answer']}",
        ),
        (vec!["?[event-1]:[A]*[B]", "?[noise]:[C]"], "['answer']*[B]"),
        (vec!["?[event-1]:[A][B][C]", "?[noise]:[D]"], "[A][B]['answer']"),
        (vec!["?[event-1]:{[A]->[A]}", "?[noise]:{[A]->[B]}"], "{['same']->['same']}"),
        (vec!["?[event-1]:{[A]->[B]}", "?[noise]:[C]"], "{['left']->['right']}"),
        (vec!["?[camera]:{[weather]->[rain]}", "?[thermometer]:{[temperature]->[warm]}"], "?['observer']:{[weather]->['condition']}"),
    ] {
        let mut pangine = Pangine::new();
        let experiences = experiences.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let state = experience_state(&mut pangine, "memory", &experiences);
        let question = must_reference(&mut pangine, question_source);
        let expected = full_scan_answers(&mut pangine, &state, &question);
        let indexed = PartitionCandidateIndex::from_state(&state).unwrap().ask(&mut pangine, &state, &question);
        assert_eq!(indexed.answers, expected);
    }
}

#[test]
fn shape_postings_avoid_opening_heterogeneous_noise() {
    let mut pangine = Pangine::new();
    let mut experiences = (0..256).map(|index| format!("?[noise-{index}]:[noise-{index}]")).collect::<Vec<_>>();
    experiences.push("?[event]:{[weather]->[rain]}".to_owned());
    let state = experience_state(&mut pangine, "memory", &experiences);
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let expected = full_scan_answers(&mut pangine, &state, &question);
    let indexed = PartitionCandidateIndex::from_state(&state).unwrap().ask(&mut pangine, &state, &question);

    assert_eq!(indexed.answers, expected);
    assert_eq!(indexed.stored_record_count, 259);
    assert_eq!(indexed.selected_record_count, 1);
    assert_eq!(indexed.matcher_input_count, 1);
}

#[test]
fn current_fuzzy_semantics_still_require_every_same_shape_record() {
    let mut pangine = Pangine::new();
    let experiences = (0..128).map(|index| format!("?[event-{index}]:{{[key-{index}]->[value-{index}]}}")).collect::<Vec<_>>();
    let state = experience_state(&mut pangine, "memory", &experiences);
    let question = must_reference(&mut pangine, "{[wanted]->['answer']}");
    let expected = full_scan_answers(&mut pangine, &state, &question);
    let indexed = PartitionCandidateIndex::from_state(&state).unwrap().ask(&mut pangine, &state, &question);

    assert_eq!(indexed.answers, expected);
    assert_eq!(indexed.selected_record_count, 128);
    assert_eq!(indexed.matcher_input_count, 128);
}

#[test]
fn fixed_literal_only_retrieval_changes_current_fuzzy_answers() {
    let mut pangine = Pangine::new();
    let experiences = vec!["?[weather-event]:{[weather]->[rain]}".to_owned(), "?[temperature-event]:{[temperature]->[warm]}".to_owned()];
    let state = experience_state(&mut pangine, "memory", &experiences);
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let answer = must_reference(&mut pangine, "['answer']");
    let target = must_reference(&mut pangine, "{[weather]->[rain]}");
    let all_records = state.observation_records().unwrap();
    let literal_records = all_records
        .into_iter()
        .filter(|(record, _)| matches!(&record.0.kind, ConceptKind::Observation { observation, .. } if *observation == target))
        .collect::<ConceptMap>();

    let complete = full_scan_answers(&mut pangine, &state, &question);
    let literal_only = answers_from_records(&mut pangine, literal_records, &question);
    let complete = pangine.format_concept(complete[&answer].as_ref().unwrap(), false);
    let literal_only = pangine.format_concept(literal_only[&answer].as_ref().unwrap(), false);

    assert_eq!(complete, "x2[rain][warm]");
    assert_eq!(literal_only, "x2[rain]");
}

#[test]
fn partition_local_indexes_reduce_source_records_not_index_state() {
    let mut pangine = Pangine::new();
    let partition_a = experience_state(&mut pangine, "a", &["?[event-1]:{[weather]->[rain]}".to_owned()]);
    let partition_b = experience_state(&mut pangine, "b", &["?[event-2]:{[temperature]->[warm]}".to_owned()]);
    let partition_c = experience_state(&mut pangine, "c", &["?[noise]:[unrelated]".to_owned()]);
    let combined = experience_state(
        &mut pangine,
        "combined",
        &["?[event-1]:{[weather]->[rain]}".to_owned(), "?[event-2]:{[temperature]->[warm]}".to_owned(), "?[noise]:[unrelated]".to_owned()],
    );
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let a = PartitionCandidateIndex::from_state(&partition_a).unwrap();
    let b = PartitionCandidateIndex::from_state(&partition_b).unwrap();
    let c = PartitionCandidateIndex::from_state(&partition_c).unwrap();
    let rebuilt = PartitionCandidateIndex::from_state(&combined).unwrap();
    let mapped_a = a.candidate_records(&pangine, &partition_a, &question);
    let mapped_b = b.candidate_records(&pangine, &partition_b, &question);
    let mapped_c = c.candidate_records(&pangine, &partition_c, &question);
    let rebuilt_records = rebuilt.candidate_records(&pangine, &combined, &question);
    let reduced = reduce_candidate_records(&[mapped_a.clone(), mapped_b.clone(), mapped_c.clone()]);
    let reversed = reduce_candidate_records(&[mapped_c.clone(), mapped_b.clone(), mapped_a.clone()]);
    let left_grouped = reduce_candidate_records(&[reduce_candidate_records(&[mapped_a.clone(), mapped_b]), mapped_c]);
    let replayed = reduce_candidate_records(&[reduced.clone(), mapped_a]);

    assert_eq!(reduced, rebuilt_records);
    assert_eq!(reversed, rebuilt_records);
    assert_eq!(left_grouped, rebuilt_records);
    assert_eq!(replayed, rebuilt_records);
    assert_eq!(answers_from_records(&mut pangine, reduced, &question), full_scan_answers(&mut pangine, &combined, &question));
}

#[test]
fn discarding_a_partition_index_changes_only_lookup_work() {
    let mut pangine = Pangine::new();
    let state = experience_state(&mut pangine, "memory", &["?[event]:{[weather]->[rain]}".to_owned(), "?[noise]:[unrelated]".to_owned()]);
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let first = PartitionCandidateIndex::from_state(&state).unwrap().ask(&mut pangine, &state, &question);
    let rebuilt = PartitionCandidateIndex::from_state(&state).unwrap().ask(&mut pangine, &state, &question);

    assert_eq!(first.answers, rebuilt.answers);
    assert_eq!(first.stored_record_count, rebuilt.stored_record_count);
    assert_eq!(first.selected_record_count, rebuilt.selected_record_count);
    assert_eq!(first.matcher_input_count, rebuilt.matcher_input_count);
    assert_eq!(rebuilt.answers, full_scan_answers(&mut pangine, &state, &question));
}

#[test]
fn a_lost_partition_cannot_leave_a_ghost_answer_in_a_stale_index() {
    let mut pangine = Pangine::new();
    let remaining = experience_state(&mut pangine, "remaining", &["?[weather-event]:{[weather]->[rain]}".to_owned()]);
    let lost = experience_state(&mut pangine, "lost", &["?[temperature-event]:{[temperature]->[warm]}".to_owned()]);
    let combined = experience_state(
        &mut pangine,
        "combined",
        &["?[weather-event]:{[weather]->[rain]}".to_owned(), "?[temperature-event]:{[temperature]->[warm]}".to_owned()],
    );
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let answer = must_reference(&mut pangine, "['answer']");
    let combined_index = PartitionCandidateIndex::from_state(&combined).unwrap();
    let remaining_index = PartitionCandidateIndex::from_state(&remaining).unwrap();
    let lost_index = PartitionCandidateIndex::from_state(&lost).unwrap();
    let combined_records = combined_index.candidate_records(&pangine, &combined, &question);
    let remaining_records = remaining_index.candidate_records(&pangine, &remaining, &question);
    let lost_records = lost_index.candidate_records(&pangine, &lost, &question);
    let stale_index_records = combined_index.candidate_records(&pangine, &remaining, &question);

    assert_eq!(reduce_candidate_records(&[remaining_records.clone(), lost_records.clone()]), combined_records);
    assert_eq!(stale_index_records, remaining_records);
    assert!(lost_records.keys().all(|record| !stale_index_records.contains_key(record)));

    let complete = answers_from_records(&mut pangine, combined_records, &question);
    let degraded = answers_from_records(&mut pangine, stale_index_records, &question);
    assert_eq!(pangine.format_concept(complete[&answer].as_ref().unwrap(), false), "x2[rain][warm]");
    assert_eq!(pangine.format_concept(degraded[&answer].as_ref().unwrap(), false), "x2[rain]");
    assert_eq!(degraded, full_scan_answers(&mut pangine, &remaining, &question));
}
