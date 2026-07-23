//! Grammar oracle for an explicit multi-entry Observation collection.
//!
//! The oracle compares shared braces, split collection delimiters, and direct
//! canonical Correlation arrows while keeping singleton `?observer:payload`.
//! The split scheme is now implemented in production. The other two parsers
//! remain test-local records of the alternatives that were rejected.

use super::classified_collections::{correlation, fold_experiences, named, observation, relevance, DirectedPair, ModelConcept, ObservedPair};
use pangine::{ConceptKind, Pangine};
use std::collections::{BTreeMap, BTreeSet};

struct CandidateParser<'a> {
    input: &'a str,
    position: usize,
    delimiters: Delimiters,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Delimiters {
    SharedBraces,
    SplitCollections,
    CanonicalArrows,
}

impl<'a> CandidateParser<'a> {
    fn parse(input: &'a str) -> Result<ModelConcept, String> {
        Self::parse_with(input, Delimiters::SharedBraces)
    }

    fn parse_split(input: &'a str) -> Result<ModelConcept, String> {
        Self::parse_with(input, Delimiters::SplitCollections)
    }

    fn parse_arrows(input: &'a str) -> Result<ModelConcept, String> {
        Self::parse_with(input, Delimiters::CanonicalArrows)
    }

    fn parse_with(input: &'a str, delimiters: Delimiters) -> Result<ModelConcept, String> {
        let mut parser = Self { input, position: 0, delimiters };
        let concept = parser.parse_expression()?;
        parser.skip_whitespace();
        if parser.position == parser.input.len() {
            Ok(concept)
        } else {
            Err(format!("unexpected input at byte {}", parser.position))
        }
    }

    fn parse_expression(&mut self) -> Result<ModelConcept, String> {
        let left = self.parse_union_expression()?;
        if !self.consume(b'-') {
            return Ok(left);
        }

        self.expect(b'>')?;
        let right = self.parse_expression()?;
        Ok(correlation([(DirectedPair { source: left, target: right }, 1)]))
    }

    fn parse_union_expression(&mut self) -> Result<ModelConcept, String> {
        self.skip_whitespace();
        let mut terms = Vec::new();
        while self.peek().is_some_and(|byte| matches!(byte, b'[' | b'(' | b'{' | b'?' | b'<')) {
            terms.push((self.parse_operand()?, 1));
            self.skip_whitespace();
        }
        if terms.is_empty() {
            return Err(format!("expected Concept at byte {}", self.position));
        }
        Ok(relevance(terms.into_iter().filter(|(concept, _)| *concept != ModelConcept::Null)))
    }

    fn parse_operand(&mut self) -> Result<ModelConcept, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'[') => self.parse_bracket(),
            Some(b'(') => self.parse_parenthesized(),
            Some(b'{') => self.parse_braced(),
            Some(b'?') => self.parse_observation(),
            Some(b'<') => self.parse_angled(),
            _ => Err(format!("expected operand at byte {}", self.position)),
        }
    }

    fn parse_parenthesized(&mut self) -> Result<ModelConcept, String> {
        self.expect(b'(')?;
        if self.delimiters == Delimiters::SplitCollections {
            self.parse_relevance_entries(b')')
        } else {
            let concept = self.parse_expression()?;
            self.expect(b')')?;
            Ok(concept)
        }
    }

    fn parse_bracket(&mut self) -> Result<ModelConcept, String> {
        self.expect(b'[')?;
        let start = self.position;
        while self.peek().is_some_and(|byte| byte != b']') {
            self.position += 1;
        }
        let value = self.input.get(start..self.position).ok_or_else(|| "invalid UTF-8 boundary".to_owned())?;
        self.expect(b']')?;
        Ok(if value.is_empty() { ModelConcept::Null } else { named(value) })
    }

    fn parse_braced(&mut self) -> Result<ModelConcept, String> {
        self.expect(b'{')?;
        let first = self.parse_union_expression()?;

        if self.consume(b'-') {
            self.expect(b'>')?;
            let target = self.parse_union_expression()?;
            self.expect(b'}')?;
            return Ok(correlation([(DirectedPair { source: first, target }, 1)]));
        }

        if self.delimiters == Delimiters::SplitCollections {
            return Err(format!("expected correlation arrow at byte {}", self.position));
        }

        self.parse_observation_entries(first, b'}')
    }

    fn parse_angled(&mut self) -> Result<ModelConcept, String> {
        self.expect(b'<')?;
        if self.delimiters == Delimiters::SplitCollections {
            if self.consume(b'>') {
                return Ok(ModelConcept::Null);
            }
            let first = self.parse_expression()?;
            self.parse_observation_entries(first, b'>')
        } else {
            self.parse_relevance_entries(b'>')
        }
    }

    fn parse_observation_entries(&mut self, first: ModelConcept, close: u8) -> Result<ModelConcept, String> {
        let mut entries = vec![Self::take_single_observation(first)?];
        while self.consume(b',') {
            entries.push(Self::take_single_observation(self.parse_expression()?)?);
        }
        self.expect(close)?;
        Ok(observation(entries))
    }

    fn parse_observation(&mut self) -> Result<ModelConcept, String> {
        self.expect(b'?')?;
        let observer = self.parse_expression()?;
        self.expect(b':')?;
        let observed = self.parse_expression()?;
        if observed == ModelConcept::Null {
            return Err("an Observation payload cannot be null".to_owned());
        }
        Ok(observation([ObservedPair { observer: (observer != ModelConcept::Null).then_some(observer), observation: observed }]))
    }

    fn parse_relevance_entries(&mut self, close: u8) -> Result<ModelConcept, String> {
        let mut entries = Vec::new();
        loop {
            let coefficient = if self.consume(b'x') { self.parse_integer()? } else { 1 };
            entries.push((self.parse_expression()?, coefficient));
            if !self.consume(b',') {
                break;
            }
        }
        self.expect(close)?;
        Ok(relevance(entries))
    }

    fn take_single_observation(concept: ModelConcept) -> Result<ObservedPair, String> {
        let ModelConcept::Observation(entries) = concept else {
            return Err("an Observation collection may contain only Observation entries".to_owned());
        };
        if entries.len() != 1 {
            return Err("each collection entry must be one Observation".to_owned());
        }
        Ok(entries.into_iter().next().unwrap())
    }

    fn parse_integer(&mut self) -> Result<i32, String> {
        self.skip_whitespace();
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        let digit_start = self.position;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        if digit_start == self.position {
            return Err(format!("expected integer at byte {start}"));
        }
        self.input[start..self.position].parse().map_err(|_| format!("invalid integer at byte {start}"))
    }

    fn expect(&mut self, expected: u8) -> Result<(), String> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", char::from(expected), self.position))
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.position).copied()
    }
}

fn format_candidate(concept: &ModelConcept) -> Result<String, String> {
    format_candidate_with(concept, Delimiters::SharedBraces)
}

fn format_split_candidate(concept: &ModelConcept) -> Result<String, String> {
    format_candidate_with(concept, Delimiters::SplitCollections)
}

fn format_arrow_candidate(concept: &ModelConcept) -> Result<String, String> {
    format_candidate_with(concept, Delimiters::CanonicalArrows)
}

fn format_candidate_with(concept: &ModelConcept, delimiters: Delimiters) -> Result<String, String> {
    match concept {
        ModelConcept::Null => Ok("[]".to_owned()),
        ModelConcept::Named(name) => Ok(format!("[{name}]")),
        ModelConcept::Relevance(entries) => {
            if delimiters != Delimiters::SharedBraces && entries.len() > 1 && entries.values().all(|coefficient| *coefficient == 1) {
                let entries = entries
                    .keys()
                    .map(|concept| {
                        let concept = format_candidate_with(concept, delimiters)?;
                        Ok(if matches!(concept.as_bytes().first(), Some(b'?')) { format!("({concept})") } else { concept })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                return Ok(format!("({})", entries.join("")));
            }

            let entries = entries
                .iter()
                .map(|(concept, coefficient)| {
                    let concept = format_candidate_with(concept, delimiters)?;
                    Ok(if *coefficient == 1 { concept } else { format!("x{coefficient}{concept}") })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let (open, close) = if delimiters == Delimiters::SplitCollections { ('(', ')') } else { ('<', '>') };
            Ok(format!("{open}{}{close}", entries.join(", ")))
        }
        ModelConcept::Correlation(entries) => {
            if entries.len() != 1 {
                return Err("the PR-6 oracle does not introduce multi-entry Correlation syntax".to_owned());
            }
            let (pair, coefficient) = entries.first_key_value().unwrap();
            if *coefficient != 1 {
                return Err("the PR-6 oracle does not move relevance inside Correlation".to_owned());
            }
            let source = format_candidate_with(&pair.source, delimiters)?;
            let source = if matches!(&pair.source, ModelConcept::Observation(_)) { format!("({source})") } else { source };
            let target = format_candidate_with(&pair.target, delimiters)?;
            if delimiters == Delimiters::CanonicalArrows {
                Ok(format!("({source}->{target})"))
            } else {
                Ok(format!("{{{source}->{target}}}"))
            }
        }
        ModelConcept::Observation(entries) => {
            let entries = entries.iter().map(|entry| format_observation_entry(entry, delimiters)).collect::<Result<Vec<_>, String>>()?;
            if entries.len() == 1 {
                Ok(entries.into_iter().next().unwrap())
            } else {
                let (open, close) = if delimiters == Delimiters::SplitCollections { ('<', '>') } else { ('{', '}') };
                Ok(format!("{open}{}{close}", entries.join(", ")))
            }
        }
    }
}

fn format_observation_entry(entry: &ObservedPair, delimiters: Delimiters) -> Result<String, String> {
    let observer = entry.observer.as_ref().map(|concept| format_candidate_with(concept, delimiters)).transpose()?.unwrap_or_else(|| "[]".to_owned());
    let observation = format_candidate_with(&entry.observation, delimiters)?;
    let observer =
        if entry.observer.as_ref().is_some_and(|concept| matches!(concept, ModelConcept::Observation(_))) { format!("({observer})") } else { observer };
    let observation = if matches!(&entry.observation, ModelConcept::Observation(_)) { format!("({observation})") } else { observation };
    Ok(format!("?{observer}:{observation}"))
}

fn payloads_for_observer(state: &ModelConcept, observer: Option<&ModelConcept>) -> BTreeSet<ModelConcept> {
    let ModelConcept::Observation(entries) = state else {
        return BTreeSet::new();
    };
    entries.iter().filter(|entry| entry.observer.as_ref() == observer).map(|entry| entry.observation.clone()).collect()
}

fn all_payloads(state: &ModelConcept) -> BTreeSet<ModelConcept> {
    let ModelConcept::Observation(entries) = state else {
        return BTreeSet::new();
    };
    entries.iter().map(|entry| entry.observation.clone()).collect()
}

fn experience_collection_entries(pangine: &mut Pangine, state: &ModelConcept) {
    let ModelConcept::Observation(entries) = state else {
        panic!("expected an Observation collection");
    };
    for entry in entries {
        let singleton = observation([entry.clone()]);
        let script = format!("['memory'] ~= {}", format_candidate(&singleton).unwrap());
        pangine.reference_concept(&script).unwrap().unwrap();
    }
}

#[test]
fn brace_list_uses_previously_invalid_syntax_without_changing_existing_observations() {
    let mut pangine = Pangine::new();
    assert!(pangine.reference_concept("{?[event-1]:[A], ?[]:[B]}").is_err());

    let existing = pangine.reference_concept("?{[camera]->[event-1]}:[rain]").unwrap().unwrap();
    assert_eq!(pangine.format_concept(&existing, false), "?{[camera]->[event-1]}:[rain]");

    let candidate = CandidateParser::parse("{?[event-1]:[A], ?[]:[B]}").unwrap();
    assert_eq!(CandidateParser::parse(&format_candidate(&candidate).unwrap()).unwrap(), candidate);
}

#[test]
fn collection_rejects_mixed_or_unclassified_entries() {
    for script in ["{}", "{[A], [B]}", "{?[event-1]:[A], {[B]->[C]}}"] {
        assert!(CandidateParser::parse(script).is_err(), "{script}");
    }
}

#[test]
fn candidate_round_trips_singletons_collections_nesting_and_outer_relevance() {
    for script in [
        "?[event-1]:[rain]",
        "{?[]:[A], ?[event-1]:[B]}",
        "?{[camera]->[event-1]}:[rain]",
        "{?{[camera]->[event-1]}:[rain], ?[]:{[rain]->[wet]}}",
        "?[outer]:{?[]:[A], ?[inner]:[B]}",
        "?({?[]:[source], ?[inner]:[source-2]}):[payload]",
        "{{?[]:[A], ?[event-1]:[B]}->[C]}",
        "{?[outer]:{?[]:[A], ?[inner]:[B]}, ?[]:<x2[C], [D]>}",
        "<x2{?[]:[A], ?[event-1]:[B]}>",
        "<x-2{?[]:[A], ?[event-1]:[B]}>",
    ] {
        let parsed = CandidateParser::parse(script).unwrap_or_else(|error| panic!("{script}: {error}"));
        let canonical = format_candidate(&parsed).unwrap();
        let reparsed = CandidateParser::parse(&canonical).unwrap_or_else(|error| panic!("{canonical}: {error}"));
        assert_eq!(reparsed, parsed, "{script} formatted as {canonical}");
        assert_eq!(format_candidate(&reparsed).unwrap(), canonical);
    }
}

#[test]
fn explicit_collection_remains_distinct_from_composition_and_weighting() {
    let collection = CandidateParser::parse("{?[event-1]:[A], ?[event-2]:[B]}").unwrap();
    let composition = CandidateParser::parse("(?[event-1]:[A])(?[event-2]:[B])").unwrap();
    let weighted = CandidateParser::parse("<x2{?[event-1]:[A], ?[event-2]:[B]}>").unwrap();

    assert_ne!(collection, composition);
    assert_ne!(collection, weighted);
    assert!(matches!(collection, ModelConcept::Observation(_)));
    assert!(matches!(composition, ModelConcept::Relevance(_)));
    assert!(matches!(weighted, ModelConcept::Relevance(_)));
}

#[test]
fn collection_is_order_independent_and_idempotent_without_changing_composition() {
    let forward = CandidateParser::parse("{?[event-1]:[A], ?[event-2]:[B]}").unwrap();
    let reverse = CandidateParser::parse("{?[event-2]:[B], ?[event-1]:[A]}").unwrap();
    let duplicate_set = CandidateParser::parse("{?[event-1]:[A], ?[event-1]:[A]}").unwrap();
    let singleton = CandidateParser::parse("?[event-1]:[A]").unwrap();
    let duplicate_composition = CandidateParser::parse("(?[event-1]:[A])(?[event-1]:[A])").unwrap();

    assert_eq!(forward, reverse);
    assert_eq!(format_candidate(&forward).unwrap(), format_candidate(&reverse).unwrap());
    assert_eq!(format_candidate(&forward).unwrap(), "{?[event-1]:[A], ?[event-2]:[B]}");
    assert_eq!(duplicate_set, singleton);
    assert_ne!(duplicate_composition, singleton);
    assert_eq!(format_candidate(&duplicate_set).unwrap(), "?[event-1]:[A]");
    assert_eq!(format_candidate(&duplicate_composition).unwrap(), "<x2?[event-1]:[A]>");
}

#[test]
fn syntax_preserves_replay_partition_and_fresh_payload_boundaries() {
    let collection = CandidateParser::parse("{?[event-1]:[A], ?[event-2]:[B]}").unwrap();
    let left = CandidateParser::parse("?[event-1]:[A]").unwrap();
    let right = CandidateParser::parse("?[event-2]:[B]").unwrap();
    let composition = CandidateParser::parse("(?[event-1]:[A])(?[event-2]:[B])").unwrap();

    let direct = fold_experiences([collection.clone()]);
    let replay = fold_experiences([direct.clone(), direct.clone()]);
    let partitioned = fold_experiences([fold_experiences([left]), fold_experiences([right])]);
    let fresh_composition = fold_experiences([composition]);

    assert_eq!(direct, replay);
    assert_eq!(direct, partitioned);
    assert_ne!(direct, fresh_composition);
}

#[test]
fn plain_and_observer_aware_questions_read_the_same_explicit_entries() {
    let state = CandidateParser::parse("{?[]:[rain], ?[event-1]:[rain], ?[event-1]:[snow]}").unwrap();
    let event = named("event-1");

    assert_eq!(all_payloads(&state), BTreeSet::from([named("rain"), named("snow")]));
    assert_eq!(payloads_for_observer(&state, Some(&event)), BTreeSet::from([named("rain"), named("snow")]));
    assert_eq!(payloads_for_observer(&state, None), BTreeSet::from([named("rain")]));
}

#[test]
fn candidate_entries_preserve_current_plain_and_observer_question_behavior() {
    let mut observer_question = Pangine::new();
    let observer_state = CandidateParser::parse("{?[C]:[A], ?[B]:[D]}").unwrap();
    experience_collection_entries(&mut observer_question, &observer_state);
    observer_question.reference_concept("['memory'] @ ?['X']:[A]").unwrap();
    let observer_answers = observer_question.reference_concept("$['X']").unwrap().unwrap();
    let observer_weights = observer_question
        .get_relevance_map(&observer_answers)
        .into_iter()
        .filter_map(|(relevance, concept)| observer_question.get_name(&concept).map(|name| (name.to_owned(), relevance.weight())))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observer_weights.get("C"), Some(&2.0));
    assert_eq!(observer_weights.get("B"), Some(&1.0));

    let mut plain_question = Pangine::new();
    let plain_state = CandidateParser::parse("{?[]:[A], ?[]:[B]}").unwrap();
    experience_collection_entries(&mut plain_question, &plain_state);
    plain_question.reference_concept("['memory'] @ ['Y']").unwrap();
    let plain_answers = plain_question.reference_concept("$['Y']").unwrap().unwrap();
    let names = plain_question
        .get_relevance_map(&plain_answers)
        .into_iter()
        .filter_map(|(_, concept)| plain_question.get_name(&concept).map(str::to_owned))
        .collect::<BTreeSet<_>>();
    assert_eq!(names, BTreeSet::from(["A".to_owned(), "B".to_owned()]));
}

#[test]
fn split_delimiters_give_each_collection_class_one_surface() {
    let grouped = CandidateParser::parse_split("([A])").unwrap();
    let relevance = CandidateParser::parse_split("(x2[A], [B])").unwrap();
    let correlation = CandidateParser::parse_split("{[A]->[B]}").unwrap();
    let observations = CandidateParser::parse_split("<?[]:[A], ?[event-1]:[B]>").unwrap();

    assert_eq!(grouped, named("A"));
    assert!(matches!(relevance, ModelConcept::Relevance(_)));
    assert!(matches!(correlation, ModelConcept::Correlation(_)));
    assert!(matches!(observations, ModelConcept::Observation(_)));
    assert_eq!(format_split_candidate(&grouped).unwrap(), "[A]");
    assert_eq!(format_split_candidate(&relevance).unwrap(), "(x2[A], [B])");
    assert_eq!(format_split_candidate(&correlation).unwrap(), "{[A]->[B]}");
    assert_eq!(format_split_candidate(&observations).unwrap(), "<?[]:[A], ?[event-1]:[B]>");
}

#[test]
fn parenthesized_relevance_preserves_grouping_and_implicit_union() {
    let named_a = CandidateParser::parse_split("[A]").unwrap();
    let grouped_a = CandidateParser::parse_split("((([A])))").unwrap();
    let implicit_union = CandidateParser::parse_split("([A][B])").unwrap();
    let explicit_default_union = CandidateParser::parse_split("([A], [B])").unwrap();
    let weighted = CandidateParser::parse_split("(x2[A])").unwrap();
    let nested_weight = CandidateParser::parse_split("(x2(x3[A]))").unwrap();

    assert_eq!(named_a, grouped_a);
    assert_eq!(implicit_union, explicit_default_union);
    assert_ne!(named_a, weighted);
    assert_eq!(format_split_candidate(&implicit_union).unwrap(), "([A][B])");
    assert_eq!(format_split_candidate(&weighted).unwrap(), "(x2[A])");
    assert_eq!(format_split_candidate(&nested_weight).unwrap(), "(x2(x3[A]))");
}

#[test]
fn split_candidate_round_trips_recursive_and_operator_boundary_cases() {
    for script in [
        "?[event-1]:[rain]",
        "<?[]:[A], ?[event-1]:[B]>",
        "?{[camera]->[event-1]}:[rain]",
        "<?{[camera]->[event-1]}:[rain], ?[]:{[rain]->[wet]}>",
        "?[outer]:<?[]:[A], ?[inner]:[B]>",
        "?<?[]:[source], ?[inner]:[source-2]>:[payload]",
        "{<?[]:[A], ?[event-1]:[B]>->[C]}",
        "<?[outer]:<?[]:[A], ?[inner]:[B]>, ?[]:(x2[C], [D])>",
        "(x2<?[]:[A], ?[event-1]:[B]>)",
        "(x-2<?[]:[A], ?[event-1]:[B]>)",
        "(x2([A][B]), {[C]->[D]})",
        "((?[event-1]:[A])(?[event-2]:[B]))",
    ] {
        let parsed = CandidateParser::parse_split(script).unwrap_or_else(|error| panic!("{script}: {error}"));
        let canonical = format_split_candidate(&parsed).unwrap();
        let reparsed = CandidateParser::parse_split(&canonical).unwrap_or_else(|error| panic!("{canonical}: {error}"));
        assert_eq!(reparsed, parsed, "{script} formatted as {canonical}");
        assert_eq!(format_split_candidate(&reparsed).unwrap(), canonical);
    }
}

#[test]
fn angle_observation_collection_normalizes_empty_singleton_order_and_duplicates() {
    let empty = CandidateParser::parse_split("<>").unwrap();
    let singleton_collection = CandidateParser::parse_split("<?[event-1]:[A]>").unwrap();
    let singleton = CandidateParser::parse_split("?[event-1]:[A]").unwrap();
    let forward = CandidateParser::parse_split("<?[event-1]:[A], ?[event-2]:[B]>").unwrap();
    let reverse = CandidateParser::parse_split("<?[event-2]:[B], ?[event-1]:[A]>").unwrap();
    let duplicate = CandidateParser::parse_split("<?[event-1]:[A], ?[event-1]:[A]>").unwrap();

    assert_eq!(empty, ModelConcept::Null);
    assert_eq!(format_split_candidate(&empty).unwrap(), "[]");
    assert_eq!(singleton_collection, singleton);
    assert_eq!(format_split_candidate(&singleton_collection).unwrap(), "?[event-1]:[A]");
    assert_eq!(forward, reverse);
    assert_eq!(format_split_candidate(&forward).unwrap(), "<?[event-1]:[A], ?[event-2]:[B]>");
    assert_eq!(duplicate, singleton);
}

#[test]
fn split_collection_remains_distinct_from_observation_composition_and_weighting() {
    let collection = CandidateParser::parse_split("<?[event-1]:[A], ?[event-2]:[B]>").unwrap();
    let composition = CandidateParser::parse_split("(?[event-1]:[A])(?[event-2]:[B])").unwrap();
    let weighted = CandidateParser::parse_split("(x2<?[event-1]:[A], ?[event-2]:[B]>)").unwrap();

    assert_ne!(collection, composition);
    assert_ne!(collection, weighted);
    assert_eq!(format_split_candidate(&composition).unwrap(), "((?[event-1]:[A])(?[event-2]:[B]))");
    assert_eq!(format_split_candidate(&weighted).unwrap(), "(x2<?[event-1]:[A], ?[event-2]:[B]>)");
}

#[test]
fn split_delimiters_reject_cross_class_and_mixed_collection_entries() {
    for script in ["{?[event-1]:[A], ?[]:[B]}", "<[A], [B]>", "<?[event-1]:[A], {[B]->[C]}>", "<x2[A]>"] {
        assert!(CandidateParser::parse_split(script).is_err(), "{script}");
    }

    assert!(matches!(CandidateParser::parse_split("(?[event-1]:[A], {[B]->[C]})").unwrap(), ModelConcept::Relevance(_)));
}

#[test]
fn production_refines_the_delimiter_split_with_operand_prefix_relevance() {
    let mut pangine = Pangine::new();

    let relevance = pangine.reference_concept("x2[A][B]").unwrap().unwrap();
    assert!(matches!(pangine.concept_kind(&relevance), Some(ConceptKind::Relevance)));
    assert_eq!(pangine.format_concept(&relevance, false), "x2[A][B]");
    assert_eq!(pangine.reference_concept("(x2[A])").unwrap(), pangine.reference_concept("x2[A]").unwrap());
    assert!(pangine.reference_concept("(x2[A], [B])").is_err());
    assert!(pangine.reference_concept("<x2[A], [B]>").is_err());
    assert!(CandidateParser::parse_split("<x2[A], [B]>").is_err());
    assert!(CandidateParser::parse_split("(x2[A], [B])").is_ok());

    let production_state = pangine.reference_concept("<?[event-1]:[A], ?[]:[B]>").unwrap().unwrap();
    let candidate_state = CandidateParser::parse_split("<?[event-1]:[A], ?[]:[B]>").unwrap();
    assert!(matches!(pangine.concept_kind(&production_state), Some(ConceptKind::ObservationSet)));
    assert!(matches!(candidate_state, ModelConcept::Observation(_)));
}

#[test]
fn split_syntax_preserves_replay_partition_and_question_entry_behavior() {
    let collection = CandidateParser::parse_split("<?[event-1]:[A], ?[event-2]:[B]>").unwrap();
    let left = CandidateParser::parse_split("?[event-1]:[A]").unwrap();
    let right = CandidateParser::parse_split("?[event-2]:[B]").unwrap();
    let composition = CandidateParser::parse_split("(?[event-1]:[A])(?[event-2]:[B])").unwrap();

    let direct = fold_experiences([collection.clone()]);
    let replay = fold_experiences([direct.clone(), direct.clone()]);
    let partitioned = fold_experiences([fold_experiences([left]), fold_experiences([right])]);
    let fresh_composition = fold_experiences([composition]);
    assert_eq!(direct, replay);
    assert_eq!(direct, partitioned);
    assert_ne!(direct, fresh_composition);

    let mut pangine = Pangine::new();
    let state = CandidateParser::parse_split("<?[C]:[A], ?[B]:[D]>").unwrap();
    let ModelConcept::Observation(entries) = state else {
        panic!("expected explicit Observation state");
    };
    for entry in entries {
        let script = format_split_candidate(&observation([entry])).unwrap();
        pangine.reference_concept(&format!("['memory'] ~= {script}")).unwrap();
    }
    pangine.reference_concept("['memory'] @ ?['X']:[A]").unwrap();
    let answers = pangine.reference_concept("$['X']").unwrap().unwrap();
    let weights = pangine
        .get_relevance_map(&answers)
        .into_iter()
        .filter_map(|(relevance, concept)| pangine.get_name(&concept).map(|name| (name.to_owned(), relevance.weight())))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(weights.get("C"), Some(&2.0));
    assert_eq!(weights.get("B"), Some(&1.0));
}

#[test]
fn canonical_arrows_free_braces_without_moving_relevance() {
    let grouped = CandidateParser::parse_arrows("([A])").unwrap();
    let relevance = CandidateParser::parse_arrows("<x2[A], [B]>").unwrap();
    let braced_correlation = CandidateParser::parse_arrows("{[A]->[B]}").unwrap();
    let direct_correlation = CandidateParser::parse_arrows("[A]->[B]").unwrap();
    let observations = CandidateParser::parse_arrows("{?[]:[A], ?[event-1]:[B]}").unwrap();

    assert_eq!(grouped, named("A"));
    assert_eq!(braced_correlation, direct_correlation);
    assert_eq!(format_arrow_candidate(&relevance).unwrap(), "<x2[A], [B]>");
    assert_eq!(format_arrow_candidate(&direct_correlation).unwrap(), "([A]->[B])");
    assert_eq!(format_arrow_candidate(&observations).unwrap(), "{?[]:[A], ?[event-1]:[B]}");

    let mut pangine = Pangine::new();
    assert_eq!(pangine.reference_concept("([A]->[B])").unwrap(), pangine.reference_concept("{[A]->[B]}").unwrap());
}

#[test]
fn canonical_arrow_candidate_round_trips_recursive_boundary_cases() {
    for script in [
        "?[event-1]:[rain]",
        "{?[]:[A], ?[event-1]:[B]}",
        "?([camera]->[event-1]):[rain]",
        "{?([camera]->[event-1]):[rain], ?[]:([rain]->[wet])}",
        "?[outer]:{?[]:[A], ?[inner]:[B]}",
        "?({?[]:[source], ?[inner]:[source-2]}):[payload]",
        "({?[]:[A], ?[event-1]:[B]}->[C])",
        "{?[outer]:{?[]:[A], ?[inner]:[B]}, ?[]:<x2[C], [D]>}",
        "<x2{?[]:[A], ?[event-1]:[B]}>",
        "<x-2{?[]:[A], ?[event-1]:[B]}>",
        "<x2([A]->[B]), ([C]->[D])>",
        "((?[event-1]:[A])(?[event-2]:[B]))",
    ] {
        let parsed = CandidateParser::parse_arrows(script).unwrap_or_else(|error| panic!("{script}: {error}"));
        let canonical = format_arrow_candidate(&parsed).unwrap();
        let reparsed = CandidateParser::parse_arrows(&canonical).unwrap_or_else(|error| panic!("{canonical}: {error}"));
        assert_eq!(reparsed, parsed, "{script} formatted as {canonical}");
        assert_eq!(format_arrow_candidate(&reparsed).unwrap(), canonical);
    }
}

#[test]
fn production_correlation_formatting_preserves_an_observation_source_boundary() {
    let mut pangine = Pangine::new();
    let concept = pangine.reference_concept("{(?[]:[A])->[target]}").unwrap().unwrap();
    let formatted = pangine.format_concept(&concept, false);

    assert_eq!(formatted, "{(?[]:[A])->[target]}");
    assert_eq!(pangine.reference_concept(&formatted).unwrap(), Some(concept));
    assert_eq!(
        format_split_candidate(&correlation([(
            DirectedPair { source: observation([ObservedPair { observer: None, observation: named("A") }]), target: named("target") },
            1,
        )]))
        .unwrap(),
        "{(?[]:[A])->[target]}"
    );
}

#[test]
fn candidate_schemes_round_trip_a_generated_recursive_concept_sample() {
    let mut concepts = BTreeSet::from([named("A"), named("B")]);

    for _ in 0..2 {
        let snapshot = concepts.iter().cloned().collect::<Vec<_>>();
        for concept in &snapshot {
            concepts.insert(relevance([(concept.clone(), 2)]));
            concepts.insert(correlation([(DirectedPair { source: concept.clone(), target: named("target") }, 1)]));
            concepts.insert(observation([ObservedPair { observer: None, observation: concept.clone() }]));
            concepts.insert(observation([ObservedPair { observer: Some(named("observer")), observation: concept.clone() }]));
        }

        for pair in snapshot.windows(2).take(12) {
            concepts.insert(relevance([(pair[0].clone(), 1), (pair[1].clone(), 1)]));
            concepts.insert(relevance([(pair[0].clone(), 2), (pair[1].clone(), -1)]));
            concepts.insert(observation([
                ObservedPair { observer: None, observation: pair[0].clone() },
                ObservedPair { observer: Some(named("observer")), observation: pair[1].clone() },
            ]));
        }
    }

    assert!(concepts.len() > 50);
    for concept in concepts {
        for delimiters in [Delimiters::SplitCollections, Delimiters::CanonicalArrows] {
            let formatted = format_candidate_with(&concept, delimiters).unwrap();
            let reparsed = CandidateParser::parse_with(&formatted, delimiters).unwrap_or_else(|error| panic!("{formatted}: {error}"));
            assert_eq!(reparsed, concept, "{formatted}");
        }
    }
}
