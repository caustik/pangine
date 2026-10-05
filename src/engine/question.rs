//! Question source selection, recursive source views, answer installation, and linked outputs.

use super::{
    CompletionOrderedStep, CompletionOrderedWindow, CompletionResult, CompletionRoute, ConceptAnswer, ConceptId, ConceptKind, ConceptMap, ConceptShape,
    LiveConceptAnswer, Pangine, ParsedUnionOperand, PerceptQuestionIndex, ProjectionAssignment, QuestionSelector, QuestionSnapshot, QuestionSource,
    QuestionSourceViews,
};
use crate::Relevance;
use std::collections::{BTreeMap, BTreeSet};

type QuestionSourceViewVisit = (ConceptId, CompletionRoute, Option<(ConceptId, ConceptId)>, Vec<CompletionOrderedStep>);

struct QuestionSourceViewTraversal<'a> {
    ordered_widths: &'a BTreeSet<usize>,
    source_shapes: Option<&'a BTreeSet<ConceptShape>>,
    track_ordered_occurrences: bool,
    visited: BTreeSet<QuestionSourceViewVisit>,
    source_views: &'a mut QuestionSourceViews,
}

// Question selection and answering.
impl Pangine {
    pub(super) fn question_selector(&self, selector: &ConceptId) -> QuestionSelector {
        if let Some(percepts) = self.question_percepts(selector) {
            return QuestionSelector::Percepts(percepts);
        }

        QuestionSelector::Subject(selector.clone())
    }

    fn question_percepts(&self, selector: &ConceptId) -> Option<Vec<ConceptId>> {
        if self.is_percept(selector) {
            return Some(vec![selector.clone()]);
        }
        if !matches!(selector.0.kind, ConceptKind::Unordered) {
            return None;
        }

        let percepts = self
            .canonical_entries(&selector.0.subconcepts)
            .into_iter()
            .map(|(concept, relevance)| (self.is_percept(&concept) && relevance == Relevance::DEFAULT).then_some(concept))
            .collect::<Option<Vec<_>>>()?;
        (!percepts.is_empty()).then_some(percepts)
    }

    pub(super) fn answer_question(&mut self, selector: QuestionSelector, question: Option<ConceptId>, graded: bool) -> Option<ConceptId> {
        let question = question?;
        let mut result = self.complete_selected_question(selector, &question, graded)?;
        let mut outputs = BTreeSet::new();
        self.collect_output_percepts(&question, &mut outputs);
        if outputs.is_empty() {
            return self.materialize_completion_rows(&result);
        }

        let mut prior = BTreeMap::<ConceptId, LiveConceptAnswer>::new();
        for output in &outputs {
            let Some((value, live)) = self.live_answer_value(output) else {
                continue;
            };
            prior.insert(value, live);
        }
        let replaces_answer = prior.is_empty() || (prior.len() == 1 && prior.values().next().is_some_and(|live| live.answer.outputs == outputs));
        if replaces_answer {
            return self.install_answer_result(result, outputs, BTreeSet::from([question.clone()]), prior.values(), &question);
        }
        if result.completions().is_empty() {
            return None;
        }

        let mut linked_outputs = outputs.clone();
        let mut questions = BTreeSet::from([question.clone()]);
        for live in prior.values() {
            linked_outputs.extend(live.answer.outputs.iter().cloned());
            questions.extend(live.answer.visible_components(self));
        }
        let answer_shape = self.answer_shape(&questions)?;
        let mut joined_outputs = outputs;
        for live in prior.values() {
            let prior_result = live.answer.to_result(self)?;
            result = self.join_completion_results(&prior_result, &live.answer.outputs, &result, &joined_outputs, &answer_shape)?;
            joined_outputs.extend(live.answer.outputs.iter().cloned());
            if result.completions().is_empty() {
                return None;
            }
        }

        self.install_answer_result(result, linked_outputs, questions, prior.values(), &answer_shape)
    }

    fn install_answer_result<'a>(
        &mut self,
        result: CompletionResult,
        outputs: BTreeSet<ConceptId>,
        questions: BTreeSet<ConceptId>,
        replaced: impl IntoIterator<Item = &'a LiveConceptAnswer>,
        row_template: &ConceptId,
    ) -> Option<ConceptId> {
        let rows = self.materialize_completion_rows_for(&result, row_template);
        let revision = match replaced.into_iter().map(|live| live.revision).max() {
            Some(revision) => revision.checked_add(1)?,
            None => 0,
        };
        if result.completions().is_empty() {
            for output in outputs {
                self.write_current_percept_value(&output, None);
            }
        } else {
            let mut answer = ConceptAnswer::from_result(self, &result);
            answer.outputs = outputs;
            answer.questions = questions;
            let live = LiveConceptAnswer::new(self, revision, answer)?;
            self.install_live_answer(live)?;
        }

        rows
    }

    pub(super) fn question_snapshot(&mut self, percepts: &[ConceptId], question: &ConceptId) -> QuestionSnapshot {
        let patterns = self.question_patterns(question);
        let sources = percepts
            .iter()
            .flat_map(|percept| {
                let subconcepts = self.question_source_map(percept);
                let candidates = if self.is_global_percept(percept) {
                    subconcepts
                        .as_ref()
                        .map(|subconcepts| PerceptQuestionIndex::from_sources(subconcepts.keys()).candidate_sources(&patterns))
                        .unwrap_or_default()
                } else {
                    // Indexes are disposable: the first question over a
                    // Percept builds its index, and any change but new
                    // experience drops it.
                    self.percept_question_indexes
                        .entry(percept.index())
                        .or_insert_with(|| PerceptQuestionIndex::from_sources(subconcepts.iter().flat_map(ConceptMap::keys)))
                        .candidate_sources(&patterns)
                };
                candidates
                    .into_iter()
                    .filter_map(|concept| {
                        let relevance = subconcepts.as_ref()?.get(&concept)?;
                        Some(QuestionSource::from_percept(percept.clone(), concept, *relevance))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        self.question_snapshot_from_sources(sources, question)
    }

    pub(super) fn subject_question_snapshot(&mut self, subject: &ConceptId, question: &ConceptId) -> QuestionSnapshot {
        self.question_snapshot_from_sources(vec![QuestionSource::from_subject(subject.clone())], question)
    }

    fn question_snapshot_from_sources(&mut self, sources: Vec<QuestionSource>, question: &ConceptId) -> QuestionSnapshot {
        #[cfg(test)]
        {
            self.question_source_visits += sources.len();
        }
        let patterns = self.question_patterns(question);
        // A top-level output Percept may bind any recursive view. Every other
        // pattern can only match its own structural shape.
        let source_shapes =
            (!patterns.iter().any(|pattern| self.is_percept(pattern))).then(|| patterns.iter().map(|pattern| pattern.0.shape()).collect::<BTreeSet<_>>());
        let mut ordered_widths = BTreeSet::new();
        self.collect_ordered_question_widths(question, &mut BTreeSet::new(), &mut BTreeMap::new(), &mut ordered_widths);
        let track_ordered_occurrences = self.question_has_shared_clause_percept(question);
        let mut snapshot = QuestionSnapshot::new();
        for source in sources {
            let mut traversal = QuestionSourceViewTraversal {
                ordered_widths: &ordered_widths,
                source_shapes: source_shapes.as_ref(),
                track_ordered_occurrences,
                visited: BTreeSet::new(),
                source_views: &mut snapshot,
            };
            self.add_question_source_views_rec(&source, &source.concept, &CompletionRoute::default(), None, &[], &mut traversal);
        }
        snapshot
    }

    fn question_patterns(&self, question: &ConceptId) -> BTreeSet<ConceptId> {
        let mut patterns = BTreeSet::new();
        let mut contains_percept_cache = BTreeMap::new();
        self.collect_question_patterns(question, true, &mut patterns, &mut contains_percept_cache);
        patterns
    }

    fn collect_ordered_question_widths(
        &self,
        concept: &ConceptId,
        visited: &mut BTreeSet<ConceptId>,
        contains_percept_cache: &mut BTreeMap<usize, bool>,
        widths: &mut BTreeSet<usize>,
    ) {
        if !visited.insert(concept.clone()) || !self.contains_percept(concept, contains_percept_cache) {
            return;
        }

        if let ConceptKind::Ordered { components } = &concept.0.kind {
            widths.insert(components.len());
        }
        for (child, _) in concept.0.children() {
            self.collect_ordered_question_widths(child, visited, contains_percept_cache, widths);
        }
    }

    fn add_question_source_views_rec(
        &mut self,
        source: &QuestionSource,
        concept: &ConceptId,
        route: &CompletionRoute,
        latent_ordered_entry: Option<&(ConceptId, ConceptId)>,
        ordered_occurrence: &[CompletionOrderedStep],
        traversal: &mut QuestionSourceViewTraversal<'_>,
    ) {
        if !traversal.visited.insert((concept.clone(), route.clone(), latent_ordered_entry.cloned(), ordered_occurrence.to_vec())) {
            return;
        }
        if traversal.source_shapes.is_none_or(|shapes| shapes.contains(&concept.0.shape())) {
            traversal.source_views.entry((source.clone(), concept.clone(), route.selected_entries.clone())).or_default().insert(route.clone());
        }

        // A sole non-default unordered edge is one coefficient-bearing source
        // boundary. Recursive source-view discovery may inspect its ordinary
        // operand and descendants while retaining the wrapper itself as a view.
        // Carry the complete wrapper beside disposable source views. A later
        // projection may choose how to group that provenance; matching does not
        // copy the coefficient into clause count, row relevance, or support.
        if let Some((_, operand)) = concept.0.coefficient_operand() {
            let mut operand_route = route.clone();
            operand_route.coefficient_ancestors.insert(concept.clone());
            self.add_question_source_views_rec(source, operand, &operand_route, latent_ordered_entry, ordered_occurrence, traversal);
            return;
        }

        match &concept.0.kind {
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                for &width in traversal.ordered_widths.range(2..components.len()) {
                    if traversal.source_shapes.is_some_and(|shapes| !shapes.contains(&ConceptShape::Ordered(width))) {
                        continue;
                    }
                    for (start, window) in components.windows(width).enumerate() {
                        let matched = self.reference_ordered(window.to_vec());
                        let mut window_route = route.clone();
                        if let Some((container, entry)) = latent_ordered_entry {
                            if window_route.selected_entries.get(container).is_some_and(|selected| selected != entry) {
                                continue;
                            }
                            window_route.selected_entries.insert(container.clone(), entry.clone());
                        }
                        window_route.ordered_windows.insert(CompletionOrderedWindow {
                            parent: concept.clone(),
                            parent_occurrence: ordered_occurrence.to_vec(),
                            start,
                            width,
                        });
                        traversal.source_views.entry((source.clone(), matched, window_route.selected_entries.clone())).or_default().insert(window_route);
                    }
                }
                for (position, child) in components.into_iter().enumerate() {
                    let child_occurrence = if traversal.track_ordered_occurrences {
                        let mut child_occurrence = ordered_occurrence.to_vec();
                        child_occurrence.push(CompletionOrderedStep { parent: concept.clone(), position });
                        child_occurrence
                    } else {
                        Vec::new()
                    };
                    self.add_question_source_views_rec(source, &child, route, None, &child_occurrence, traversal);
                }
            }
            ConceptKind::Unordered => {
                let children = concept.0.subconcepts.clone();
                for (child, relevance) in children {
                    // reference_map returns a sole default member directly.
                    // Avoid global interner cleanup for that common case.
                    let coefficient_concept =
                        if relevance == Relevance::DEFAULT { Some(child) } else { self.reference_map(&ConceptMap::from([(child, relevance)])) };
                    if let Some(coefficient_concept) = coefficient_concept {
                        let mut child_route = route.clone();
                        if Self::is_grouped_entry(&coefficient_concept) {
                            if child_route.selected_entries.get(concept).is_some_and(|selected| selected != &coefficient_concept) {
                                continue;
                            }
                            child_route.selected_entries.insert(concept.clone(), coefficient_concept.clone());
                        }
                        let child_latent_ordered_entry = (concept.clone(), coefficient_concept.clone());
                        self.add_question_source_views_rec(
                            source,
                            &coefficient_concept,
                            &child_route,
                            Some(&child_latent_ordered_entry),
                            ordered_occurrence,
                            traversal,
                        );
                    }
                }
            }
            ConceptKind::Named(_) | ConceptKind::Percept { .. } => {}
        }
    }

    fn is_grouped_entry(concept: &ConceptId) -> bool {
        if matches!(concept.0.kind, ConceptKind::Unordered) && concept.0.subconcepts.len() > 1 {
            return true;
        }
        concept.0.coefficient_operand().is_some_and(|(_, operand)| matches!(operand.0.kind, ConceptKind::Unordered) && operand.0.subconcepts.len() > 1)
    }

    fn question_has_shared_clause_percept(&self, question: &ConceptId) -> bool {
        if !matches!(question.0.kind, ConceptKind::Unordered)
            || question.0.subconcepts.len() < 2
            || question.0.subconcepts.values().any(|relevance| *relevance != Relevance::DEFAULT)
            || question.0.subconcepts.keys().any(|concept| !matches!(concept.0.kind, ConceptKind::Ordered { .. }))
        {
            return false;
        }

        let mut seen = BTreeSet::new();
        for clause in question.0.subconcepts.keys() {
            let mut percepts = BTreeSet::new();
            self.collect_output_percepts(clause, &mut percepts);
            if percepts.into_iter().any(|percept| !seen.insert(percept)) {
                return true;
            }
        }
        false
    }
}

// Experience/question projection and shared-answer conditioning.
impl Pangine {
    /// Returns the shared answer shape linked to every Percept in `concept`.
    ///
    /// `$` can read the returned shape and `^` can choose it. If explicit
    /// assignment detached part of the original shape, the result contains
    /// only question fragments and Percepts that remain linked. A Concept
    /// containing an unlinked Percept or Percepts from different answers has
    /// no linked answer.
    pub(super) fn linked_answer(&mut self, concept: &ConceptId) -> Option<ConceptId> {
        self.shared_live_answer(concept)?.1.answer.shape(self)
    }

    /// Returns the ordinary live answer Concept shared by every output in
    /// `concept`.
    ///
    /// The returned value contains the complete rows, proof details, active
    /// outputs, cached projections, and current revision. It can be formatted,
    /// parsed by another engine, and installed on its encoded output group. The
    /// exact internal encoding moves with this crate.
    pub fn linked_answer_value(&self, concept: &ConceptId) -> Option<ConceptId> {
        Some(self.shared_live_answer(concept)?.0)
    }

    /// Installs an owned live answer Concept on the output group encoded inside
    /// it.
    ///
    /// The complete group is validated before any Percept changes. Existing
    /// linked outputs outside the installed group are detached in the same way
    /// as an ordinary grouped update.
    pub fn install_answer_value(&mut self, value: &ConceptId) -> bool {
        if !self.owns(value) {
            return false;
        }
        let Some(live) = LiveConceptAnswer::decode_validated(self, value) else {
            return false;
        };
        let updates = live.answer.outputs.into_iter().map(|output| (output, Some(value.clone()))).collect::<Vec<_>>();
        self.set_percept_values(&updates).is_ok()
    }

    pub(super) fn answer_shape(&mut self, concepts: &BTreeSet<ConceptId>) -> Option<ConceptId> {
        let mut contained_percepts = BTreeSet::new();
        for concept in concepts.iter().filter(|concept| !self.is_percept(concept)) {
            self.collect_output_percepts(concept, &mut contained_percepts);
        }
        let operands = concepts
            .iter()
            .filter(|concept| !self.is_percept(concept) || !contained_percepts.contains(*concept))
            .cloned()
            .map(ParsedUnionOperand::ordinary)
            .collect::<Vec<_>>();
        self.reference_union(&operands).ok().flatten()
    }

    pub(super) fn collect_output_percepts(&self, concept: &ConceptId, percepts: &mut BTreeSet<ConceptId>) {
        if self.is_percept(concept) {
            percepts.insert(concept.clone());
            return;
        }

        for (child, _) in concept.0.children() {
            self.collect_output_percepts(child, percepts);
        }
    }

    fn collect_question_patterns(
        &self,
        question: &ConceptId,
        is_top_level: bool,
        patterns: &mut BTreeSet<ConceptId>,
        contains_percept_cache: &mut BTreeMap<usize, bool>,
    ) {
        if !self.contains_percept(question, contains_percept_cache) {
            return;
        }

        if is_top_level || !self.is_percept(question) {
            patterns.insert(question.clone());
        }

        if matches!(question.0.kind, ConceptKind::Ordered { .. }) {
            return;
        }

        for (child, _) in question.0.children() {
            self.collect_question_patterns(child, false, patterns, contains_percept_cache);
        }
    }

    fn contains_percept(&self, concept: &ConceptId, cache: &mut BTreeMap<usize, bool>) -> bool {
        if let Some(contains) = cache.get(&concept.index()) {
            return *contains;
        }

        let contains = self.is_percept(concept) || concept.0.children().any(|(child, _)| self.contains_percept(child, cache));
        cache.insert(concept.index(), contains);
        contains
    }

    pub(super) fn merge_projection_assignments(left: &ProjectionAssignment, right: &ProjectionAssignment) -> Option<ProjectionAssignment> {
        let mut merged = left.clone();
        for (percept, candidate) in right {
            if let Some(current) = merged.get(percept) {
                if current != candidate {
                    return None;
                }
            } else {
                merged.insert(percept.clone(), candidate.clone());
            }
        }
        Some(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::test_rng::Rng;

    fn full_question_snapshot(pangine: &mut Pangine, percepts: &[ConceptId], question: &ConceptId) -> QuestionSnapshot {
        let sources = percepts
            .iter()
            .flat_map(|percept| {
                pangine
                    .percept_subconcepts
                    .get(&percept.index())
                    .into_iter()
                    .flatten()
                    .map(|(concept, &relevance)| QuestionSource::from_percept(percept.clone(), concept.clone(), relevance))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut ordered_widths = BTreeSet::new();
        pangine.collect_ordered_question_widths(question, &mut BTreeSet::new(), &mut BTreeMap::new(), &mut ordered_widths);
        let track_ordered_occurrences = pangine.question_has_shared_clause_percept(question);
        let mut snapshot = QuestionSnapshot::new();
        for source in sources {
            let mut traversal = QuestionSourceViewTraversal {
                ordered_widths: &ordered_widths,
                source_shapes: None,
                track_ordered_occurrences,
                visited: BTreeSet::new(),
                source_views: &mut snapshot,
            };
            pangine.add_question_source_views_rec(&source, &source.concept, &CompletionRoute::default(), None, &[], &mut traversal);
        }
        snapshot
    }

    #[test]
    fn question_snapshot_drops_only_work_the_question_cannot_use() {
        let mut pangine = Pangine::new();
        for concept in [
            "[C]->[bridge]->[E]",
            "[C]->[sound]->[quiet]",
            "[E]->[sound]->[loud]",
            "[C]*[sound]*[calm]",
            "[C]x2[bridge]",
            "[bridge]![E]",
            "[C]->([A]->[Z])",
            "x2(([weighted-left]->[A])([weighted-right]->[A]))",
        ] {
            let command = format!("{{world}} ~= {concept}");
            assert!(pangine.reference_concept(&command).unwrap().is_some());
        }

        let source = pangine.reference_percept("world");
        for question_text in [
            "[C]*[sound]*{unordered-answer}",
            "[C]->[sound]->{ordered-answer}",
            "{who}->([A]->[Z])",
            "([weighted-left]->{weighted-answer})([weighted-right]->{weighted-answer})",
            "{anything}",
        ] {
            let question = pangine.reference_concept(question_text).unwrap().unwrap();
            let filtered = pangine.question_snapshot(std::slice::from_ref(&source), &question);
            let full = full_question_snapshot(&mut pangine, std::slice::from_ref(&source), &question);
            assert!(filtered.iter().all(|(key, ancestors)| full.get(key).is_some_and(|full_ancestors| ancestors.is_subset(full_ancestors))));

            let filtered_results = pangine.complete_question_snapshot(&question, &filtered, false);
            let full_results = pangine.complete_question_snapshot(&question, &full, false);
            assert!(filtered_results.completions() == full_results.completions(), "question {question_text}");
        }

        let unordered = pangine.reference_concept("[C]*[sound]*{unordered-answer}").unwrap().unwrap();
        let unordered_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &unordered);
        assert!(unordered_snapshot.keys().all(|(_, matched, _)| matched.0.shape() == ConceptShape::Unordered));

        let ordered = pangine.reference_concept("[C]->[sound]->{ordered-answer}").unwrap().unwrap();
        let ordered_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &ordered);
        assert!(ordered_snapshot.keys().all(|(_, matched, _)| matched.0.shape() == ConceptShape::Ordered(3)));

        let wildcard = pangine.reference_percept("anything");
        let wildcard_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &wildcard);
        let full_wildcard = full_question_snapshot(&mut pangine, std::slice::from_ref(&source), &wildcard);
        assert!(wildcard_snapshot == full_wildcard);
    }

    #[test]
    fn anchored_questions_visit_only_indexed_candidate_experiences() {
        let mut pangine = Pangine::new();
        let memory = pangine.reference_percept("memory");

        for index in 0..512 {
            let key = pangine.reference_named(&format!("noise-{index}")).unwrap();
            let value = pangine.reference_named(&format!("value-{index}")).unwrap();
            let experience = pangine.reference_ordered(vec![key, value]);
            pangine.record_experience(&memory, &experience).unwrap();
        }

        let needle = pangine.reference_named("needle").unwrap();
        let answer = pangine.reference_named("answer").unwrap();
        let nested = pangine.reference_ordered(vec![needle.clone(), answer.clone()]);
        let wrapper_name = pangine.reference_named("wrapper").unwrap();
        let wrapper = pangine.reference_ordered(vec![wrapper_name, nested]);
        pangine.record_experience(&memory, &wrapper).unwrap();

        let output = pangine.reference_percept("output");
        let question = pangine.reference_ordered(vec![needle, output.clone()]);
        pangine.question_source_visits = 0;
        let result = pangine.complete_question(std::slice::from_ref(&memory), &question).unwrap();

        assert_eq!(pangine.question_source_visits, 1);
        assert_eq!(result.completions().len(), 1);
        assert_eq!(result.completions()[0].binding(&output), Some(&answer));
    }

    #[test]
    fn question_index_tracks_replacement_and_shape_only_queries() {
        let mut pangine = Pangine::new();
        let memory = pangine.reference_percept("memory");
        for index in 0..256 {
            let atomic = pangine.reference_named(&format!("atomic-{index}")).unwrap();
            pangine.record_experience(&memory, &atomic).unwrap();
        }

        let left = pangine.reference_named("left").unwrap();
        let right = pangine.reference_named("right").unwrap();
        let pair = pangine.reference_ordered(vec![left, right]);
        pangine.record_experience(&memory, &pair).unwrap();

        let first = pangine.reference_percept("first");
        let second = pangine.reference_percept("second");
        let shape_question = pangine.reference_ordered(vec![first, second]);
        pangine.question_source_visits = 0;
        let result = pangine.complete_question(std::slice::from_ref(&memory), &shape_question).unwrap();
        assert_eq!(pangine.question_source_visits, 1);
        assert_eq!(result.completions().len(), 1);

        let replacement = pangine.reference_named("replacement").unwrap();
        assert!(pangine.set_percept_value(&memory, Some(replacement)));
        pangine.question_source_visits = 0;
        let result = pangine.complete_question(std::slice::from_ref(&memory), &shape_question).unwrap();
        assert_eq!(pangine.question_source_visits, 0);
        assert!(result.completions().is_empty());

        assert!(pangine.set_percept_value(&memory, None));
        assert!(!pangine.percept_question_indexes.contains_key(&memory.index()));
    }

    #[test]
    fn shared_percept_join_visits_only_the_indexed_sources_for_each_clause() {
        let mut pangine = Pangine::new();
        let memory = pangine.reference_percept("memory");
        for index in 0..512 {
            let left = pangine.reference_named(&format!("unrelated-left-{index}")).unwrap();
            let right = pangine.reference_named(&format!("unrelated-right-{index}")).unwrap();
            let experience = pangine.reference_ordered(vec![left, right]);
            pangine.record_experience(&memory, &experience).unwrap();
        }

        let subject = pangine.reference_named("Socrates").unwrap();
        let middle_value = pangine.reference_named("human").unwrap();
        let conclusion = pangine.reference_named("mortal").unwrap();
        let first_source = pangine.reference_ordered(vec![subject.clone(), middle_value.clone()]);
        let second_source = pangine.reference_ordered(vec![middle_value, conclusion.clone()]);
        pangine.record_experience(&memory, &first_source).unwrap();
        pangine.record_experience(&memory, &second_source).unwrap();

        let middle = pangine.reference_percept("middle");
        let first_clause = pangine.reference_ordered(vec![subject, middle.clone()]);
        let second_clause = pangine.reference_ordered(vec![middle.clone(), conclusion]);
        let question = pangine.reference_map(&ConceptMap::from([(first_clause, Relevance::DEFAULT), (second_clause, Relevance::DEFAULT)]));
        pangine.question_source_visits = 0;
        let result = pangine.complete_question(std::slice::from_ref(&memory), &question.unwrap()).unwrap();

        assert_eq!(pangine.question_source_visits, 2);
        assert_eq!(result.completions().len(), 1);
        assert_eq!(result.completions()[0].binding(&middle).map(|concept| pangine.format_concept(concept, false)), Some("[human]".to_owned()));
    }

    #[test]
    fn positional_names_narrow_a_question_whose_names_every_experience_holds() {
        let mut pangine = Pangine::new();
        let mut rng = Rng::new(7);
        for _ in 0..256 {
            let cells = (0..4).map(|_| ["[x]", "[o]"][rng.below(2)]).collect::<Vec<_>>().join("->");
            pangine.reference_concept(&format!("{{boards}} ~= [x]->[o]->{cells}")).unwrap();
        }
        let boards = pangine.reference_percept("boards");
        let question = pangine.reference_concept("[x]->[o]->[x]->[o]->{third}->{fourth}").unwrap().unwrap();
        let matching =
            pangine.get_relevance_map(&boards).iter().filter(|(_, board)| pangine.format_concept(board, false).starts_with("[x]->[o]->[x]->[o]->")).count();

        pangine.question_source_visits = 0;
        let result = pangine.complete_question(std::slice::from_ref(&boards), &question).unwrap();
        // Every board holds both names, so only their positions narrow the question.
        assert_eq!((pangine.get_relevance_map(&boards).len(), matching), (16, 4));
        assert_eq!(pangine.question_source_visits, matching);
        assert_eq!(result.completions().len(), matching);
    }

    #[test]
    fn indexed_questions_find_every_completion_a_full_scan_finds() {
        const NAMES: [&str; 4] = ["[a]", "[b]", "[c]", "[d]"];
        fn random_name(rng: &mut Rng) -> String {
            NAMES[rng.below(NAMES.len())].to_owned()
        }
        fn random_sequence(rng: &mut Rng) -> String {
            let width = 2 + rng.below(4);
            (0..width).map(|_| random_name(rng)).collect::<Vec<_>>().join("->")
        }
        fn random_experience(rng: &mut Rng) -> String {
            match rng.below(8) {
                0 => format!("{}->({}->{})->{}", random_name(rng), random_name(rng), random_name(rng), random_name(rng)),
                1 => format!("({})({})", random_sequence(rng), random_sequence(rng)),
                2 => format!("x2({})", random_sequence(rng)),
                3 => format!("!({})", random_sequence(rng)),
                4 => format!("{}->x2{}->{}", random_name(rng), random_name(rng), random_name(rng)),
                5 => format!("{}->({}{})->{}", random_name(rng), random_name(rng), random_name(rng), random_name(rng)),
                _ => random_sequence(rng),
            }
        }
        // Every clause holds a blank, since a question without one selects no sources.
        fn random_clause(rng: &mut Rng, blank: usize) -> String {
            let width = 2 + rng.below(3);
            let blank_position = rng.below(width);
            (0..width)
                .map(|position| match (position == blank_position, rng.below(10)) {
                    (true, _) => format!("{{p{blank}}}"),
                    (false, 0..=4) => random_name(rng),
                    (false, 5..=8) => format!("{{p{}}}", rng.below(3)),
                    (false, _) => format!("({}->{{p{}}})", random_name(rng), rng.below(3)),
                })
                .collect::<Vec<_>>()
                .join("->")
        }

        let (mut narrowed, mut rows) = (0, 0);
        for seed in 0..64 {
            let mut rng = Rng::new(seed);
            let mut pangine = Pangine::new();
            for _ in 0..24 {
                let experience = random_experience(&mut rng);
                pangine.reference_concept(&format!("{{memory}} ~= {experience}")).unwrap();
            }
            let memory = pangine.reference_percept("memory");
            let sources = pangine.get_relevance_map(&memory).len();
            for _ in 0..16 {
                let question_text = if rng.below(3) == 0 {
                    let (first, second_blank) = (random_clause(&mut rng, 0), rng.below(2));
                    format!("({first})({})", random_clause(&mut rng, second_blank))
                } else {
                    random_clause(&mut rng, 0)
                };
                let question = pangine.reference_concept(&question_text).unwrap().unwrap();
                pangine.question_source_visits = 0;
                let indexed = pangine.question_snapshot(std::slice::from_ref(&memory), &question);
                narrowed += usize::from(pangine.question_source_visits < sources);
                let full = full_question_snapshot(&mut pangine, std::slice::from_ref(&memory), &question);
                for graded in [false, true] {
                    let indexed_results = pangine.complete_question_snapshot(&question, &indexed, graded);
                    let full_results = pangine.complete_question_snapshot(&question, &full, graded);
                    assert!(indexed_results.completions() == full_results.completions(), "seed {seed}: {question_text}");
                    rows += full_results.completions().len();
                }
            }
        }
        assert!(narrowed > 0 && rows > 0, "the generated cases must narrow some questions and complete some rows: {narrowed} narrowed, {rows} rows");
    }
}
