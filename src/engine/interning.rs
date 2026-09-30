//! Canonical Concept interning, engine ownership, and retained Percept state.

use super::{
    ConceptId, ConceptKind, ConceptMap, LiveConceptAnswer, Pangine, ParseError, ParseResult, ParsedUnionOperand, PerceptQuestionIndex, GLOBAL_PERCEPT_NAME,
};
use crate::Relevance;
use std::collections::{hash_map::DefaultHasher, BTreeSet};
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};

// Concept interning and engine ownership.
impl Pangine {
    pub(super) fn reference_named(&mut self, name: &str) -> Option<ConceptId> {
        (!name.is_empty()).then(|| self.reference_name(name))
    }

    pub(super) fn reference_merge_with_inversion(
        &mut self,
        left: Option<ConceptId>,
        right: Option<ConceptId>,
        right_inversion: bool,
    ) -> ParseResult<Option<ConceptId>> {
        let mut map = ConceptMap::new();

        if let Some(left) = left {
            self.add_merge_concept(&mut map, left, false, Relevance::DEFAULT).ok_or(ParseError::RelevanceOverflow)?;
        }
        if let Some(right) = right {
            self.add_merge_concept(&mut map, right, right_inversion, Relevance::DEFAULT).ok_or(ParseError::RelevanceOverflow)?;
        }

        Ok(self.reference_map(&map))
    }

    pub(super) fn reference_union(&mut self, operands: &[ParsedUnionOperand]) -> ParseResult<Option<ConceptId>> {
        let mut map = ConceptMap::new();

        for operand in operands {
            self.add_union_concept(&mut map, operand.concept.clone(), false, operand.relevance).ok_or(ParseError::RelevanceOverflow)?;
        }

        Ok(self.reference_map(&map))
    }

    pub(super) fn materialized_percept_map(&mut self, subconcepts: &ConceptMap) -> Option<ConceptMap> {
        let mut map = ConceptMap::new();
        for (concept, &relevance) in subconcepts {
            self.add_union_concept(&mut map, concept.clone(), false, relevance)?;
        }
        Some(map)
    }

    fn sole_default_concept(map: &ConceptMap) -> Option<&ConceptId> {
        let (concept, relevance) = map.first_key_value()?;
        (map.len() == 1 && *relevance == Relevance::DEFAULT).then_some(concept)
    }

    pub(super) fn reference_map(&mut self, map: &ConceptMap) -> Option<ConceptId> {
        if map.is_empty() {
            return None;
        }

        // 3.x returns a sole default-coefficient Concept directly before interning:
        // 3.x/pangine/src/libpangine/common/pae_pangine.cpp:314-328
        if let Some(concept) = Self::sole_default_concept(map) {
            return Some(concept.clone());
        }

        Some(self.reference_composite(ConceptKind::Unordered, map.clone()))
    }

    pub(super) fn reference_ordered(&mut self, mut components: Vec<ConceptId>) -> ConceptId {
        if components.len() == 1 {
            return components.pop().unwrap();
        }

        debug_assert!(components.len() >= 2);
        self.reference_composite(ConceptKind::Ordered { components }, ConceptMap::new())
    }

    pub(super) fn reference_composite(&mut self, kind: ConceptKind, subconcepts: ConceptMap) -> ConceptId {
        let fingerprint = Self::composite_fingerprint(&kind, &subconcepts);
        if let Some(bucket) = self.composite_lookup.get_mut(&fingerprint) {
            let mut existing = None;
            bucket.retain(|candidate| {
                let Some(candidate) = candidate.upgrade() else {
                    return false;
                };
                if existing.is_none() && candidate.kind == kind && candidate.subconcepts == subconcepts {
                    existing = Some(ConceptId(candidate));
                }
                true
            });
            if let Some(existing) = existing {
                return existing;
            }
        }

        let concept = self.alloc(kind, subconcepts);
        let weak = Rc::downgrade(&concept.0);
        self.composites.push(weak.clone());
        self.composite_lookup.entry(fingerprint).or_default().push(weak);
        self.maybe_prune_indexes();
        concept
    }

    pub(super) fn composite_fingerprint(kind: &ConceptKind, subconcepts: &ConceptMap) -> u64 {
        let mut hasher = DefaultHasher::new();
        match kind {
            ConceptKind::Named(name) => {
                0_u8.hash(&mut hasher);
                name.hash(&mut hasher);
            }
            ConceptKind::Percept { name } => {
                1_u8.hash(&mut hasher);
                name.hash(&mut hasher);
            }
            ConceptKind::Unordered => 2_u8.hash(&mut hasher),
            ConceptKind::Ordered { components } => {
                3_u8.hash(&mut hasher);
                components.hash(&mut hasher);
            }
        }
        subconcepts.hash_lookup_summary(&mut hasher);
        hasher.finish()
    }

    pub(super) fn alloc(&self, kind: ConceptKind, subconcepts: ConceptMap) -> ConceptId {
        let index = self.next_concept_id.get();
        self.next_concept_id.set(index + 1);
        ConceptId::new(self.id, index, kind, subconcepts)
    }

    pub(super) fn owns(&self, concept: &ConceptId) -> bool {
        concept.0.pangine_id == self.id
    }

    pub(super) fn is_percept(&self, concept: &ConceptId) -> bool {
        self.owns(concept) && matches!(concept.0.kind, ConceptKind::Percept { .. })
    }

    pub(super) fn is_global_percept(&self, concept: &ConceptId) -> bool {
        self.owns(concept) && matches!(&concept.0.kind, ConceptKind::Percept { name } if name == GLOBAL_PERCEPT_NAME)
    }

    pub(super) fn is_mutable_percept(&self, concept: &ConceptId) -> bool {
        self.is_percept(concept) && !self.is_global_percept(concept)
    }

    pub(super) fn accepts_percept_input(&self, percept: &ConceptId, input: Option<&ConceptId>) -> bool {
        self.is_mutable_percept(percept) && input.is_none_or(|concept| self.owns(concept))
    }

    pub(super) fn live_ordinary_concepts(&self) -> impl Iterator<Item = ConceptId> + '_ {
        self.names.values().chain(&self.composites).filter_map(Weak::upgrade).map(ConceptId)
    }

    pub(super) fn global_concept_map(&self) -> ConceptMap {
        self.live_ordinary_concepts().map(|concept| (concept, Relevance::DEFAULT)).collect()
    }

    pub(super) fn global_value(&self) -> Option<ConceptId> {
        self.reference_transient_map(self.global_concept_map())
    }

    pub(super) fn set_percept_subconcepts(&mut self, percept: &ConceptId, subconcepts: ConceptMap) -> Option<ConceptId> {
        if !self.is_mutable_percept(percept) || subconcepts.iter().any(|(concept, relevance)| !self.owns(concept) || relevance.is_empty()) {
            return None;
        }

        let index = percept.index();
        let value_map = self.materialized_percept_map(&subconcepts)?;
        let value = Self::sole_default_concept(&subconcepts).cloned().or_else(|| self.reference_map(&value_map));
        if subconcepts.is_empty() {
            self.percept_subconcepts.remove(&index);
            self.percept_question_indexes.remove(&index);
            self.percept_value_maps.remove(&index);
            self.percept_values.remove(&index);
        } else {
            let question_index = PerceptQuestionIndex::from_sources(subconcepts.keys());
            self.percept_subconcepts.insert(index, subconcepts);
            self.percept_question_indexes.insert(index, question_index);
            self.percept_value_maps.insert(index, value_map);
            match value.clone() {
                Some(value) => {
                    self.percept_values.insert(index, value);
                }
                None => {
                    self.percept_values.remove(&index);
                }
            }
        }
        value
    }

    pub(super) fn write_current_percept_value(&mut self, percept: &ConceptId, value: Option<ConceptId>) {
        if let Some(value) = value.as_ref() {
            if let Some(live) = LiveConceptAnswer::decode_validated(self, value) {
                if let Some(projection) = live.projection(percept) {
                    let _ = self.write_live_answer_value(percept, value, projection);
                    return;
                }
            }
        }
        let subconcepts = value.into_iter().map(|concept| (concept, Relevance::DEFAULT)).collect();
        self.set_percept_subconcepts(percept, subconcepts);
        self.current_value_percepts.insert(percept.index());
    }

    pub(super) fn live_answer_value(&self, percept: &ConceptId) -> Option<(ConceptId, LiveConceptAnswer)> {
        let value = self.percept_values.get(&percept.index())?.clone();
        let live = LiveConceptAnswer::decode(self, &value)?;
        if !live.answer.outputs.contains(percept) || live.answer.outputs.iter().any(|output| self.percept_values.get(&output.index()) != Some(&value)) {
            return None;
        }
        Some((value, live))
    }

    pub(super) fn question_source_map(&self, percept: &ConceptId) -> Option<ConceptMap> {
        if self.is_global_percept(percept) {
            return Some(self.global_concept_map());
        }

        if let Some((_, live)) = self.live_answer_value(percept) {
            let projection = live.projection(percept)?;
            return Some(projection.into_iter().map(|concept| (concept, Relevance::DEFAULT)).collect());
        }
        self.percept_subconcepts.get(&percept.index()).cloned()
    }

    pub(super) fn shared_live_answer(&self, concept: &ConceptId) -> Option<(ConceptId, LiveConceptAnswer)> {
        let mut outputs = BTreeSet::new();
        self.collect_output_percepts(concept, &mut outputs);
        let first = outputs.first()?;
        let (value, live) = self.live_answer_value(first)?;
        if !outputs.is_subset(&live.answer.outputs) {
            return None;
        }
        Some((value, live))
    }

    fn write_live_answer_value(&mut self, output: &ConceptId, value: &ConceptId, projection: Option<ConceptId>) -> Option<()> {
        let mut subconcepts = ConceptMap::new();
        subconcepts.insert(value.clone(), Relevance::DEFAULT);
        self.set_percept_subconcepts(output, subconcepts)?;
        match projection {
            Some(projection) => {
                self.percept_question_indexes.insert(output.index(), PerceptQuestionIndex::from_sources([&projection]));
            }
            None => {
                self.percept_question_indexes.remove(&output.index());
            }
        }
        self.current_value_percepts.insert(output.index());
        Some(())
    }

    pub(super) fn install_live_answer(&mut self, live: LiveConceptAnswer) -> Option<ConceptId> {
        let projections =
            live.answer.outputs.iter().map(|output| live.projection(output).map(|projection| (output.clone(), projection))).collect::<Option<Vec<_>>>()?;
        let value = live.encode(self);
        for (output, projection) in projections {
            self.write_live_answer_value(&output, &value, projection)?;
        }
        Some(value)
    }

    pub(super) fn record_experience(&mut self, percept: &ConceptId, experience: &ConceptId) -> Option<()> {
        if !self.accepts_percept_input(percept, Some(experience)) {
            return None;
        }

        let index = percept.index();
        let current_relevance = self.percept_subconcepts.get(&index).and_then(|subconcepts| subconcepts.get(experience)).copied().unwrap_or(Relevance::EMPTY);
        let next_relevance = current_relevance.checked_add(Relevance::DEFAULT)?;
        let incremental_value_map = if current_relevance.is_empty() {
            let mut value_map = if let Some(value_map) = self.percept_value_maps.remove(&index) {
                value_map
            } else {
                let subconcepts = self.percept_subconcepts.get(&index).cloned().unwrap_or_default();
                self.materialized_percept_map(&subconcepts)?
            };
            self.add_union_concept(&mut value_map, experience.clone(), false, Relevance::DEFAULT)?;
            Some(value_map)
        } else {
            None
        };
        self.percept_subconcepts.entry(index).or_default().insert(experience.clone(), next_relevance);
        let question_index = self.percept_question_indexes.entry(index).or_default();
        if current_relevance.is_empty() || !question_index.contains_source(experience) {
            question_index.insert_source(experience);
        }
        let value_map = if let Some(value_map) = incremental_value_map {
            value_map
        } else {
            let subconcepts = self.percept_subconcepts[&index].clone();
            self.materialized_percept_map(&subconcepts)?
        };
        self.percept_value_maps.insert(index, value_map);
        self.percept_values.remove(&index);
        Some(())
    }

    pub(super) fn materialize_percept_value(&mut self, percept: &ConceptId) -> Option<ConceptId> {
        if !self.is_mutable_percept(percept) {
            return None;
        }

        let index = percept.index();
        if let Some(value) = self.percept_values.get(&index) {
            return Some(value.clone());
        }

        let single_subconcept = self.percept_subconcepts.get(&index).and_then(Self::sole_default_concept).cloned();
        let value = if let Some(concept) = single_subconcept {
            Some(concept)
        } else {
            let value_map = if let Some(value_map) = self.percept_value_maps.remove(&index) {
                value_map
            } else {
                let subconcepts = self.percept_subconcepts.get(&index).cloned().unwrap_or_default();
                self.materialized_percept_map(&subconcepts)?
            };
            let value = self.reference_map(&value_map);
            self.percept_value_maps.insert(index, value_map);
            value
        };

        if let Some(value) = value.clone() {
            self.percept_values.insert(index, value);
        }
        value
    }

    pub(super) fn reference_transient_map(&self, map: ConceptMap) -> Option<ConceptId> {
        if map.is_empty() {
            return None;
        }

        if let Some(concept) = Self::sole_default_concept(&map) {
            return Some(concept.clone());
        }

        Some(self.alloc(ConceptKind::Unordered, map))
    }

    fn prune_indexes(&mut self) {
        self.names.retain(|_, concept| concept.strong_count() > 0);
        self.composites.retain(|concept| concept.strong_count() > 0);
        self.composite_lookup.clear();
        for concept in self.composites.iter().filter_map(Weak::upgrade) {
            let fingerprint = Self::composite_fingerprint(&concept.kind, &concept.subconcepts);
            self.composite_lookup.entry(fingerprint).or_default().push(Rc::downgrade(&concept));
        }

        let live_size = self.names.len().saturating_add(self.composites.len());
        self.next_index_prune_size = live_size.checked_next_power_of_two().and_then(|size| size.checked_mul(2)).unwrap_or(usize::MAX).max(2);
    }

    pub(super) fn maybe_prune_indexes(&mut self) {
        if self.names.len().saturating_add(self.composites.len()) >= self.next_index_prune_size {
            self.prune_indexes();
        }
    }
}

// Relevance accumulation.
impl Pangine {
    pub(super) fn add_merge_concept(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) -> Option<()> {
        let subconcepts = concept.0.subconcepts.clone();
        if matches!(concept.0.kind, ConceptKind::Unordered) {
            for (child, child_relevance) in subconcepts {
                self.add_union_concept(map, child, inversion, relevance.checked_mul(child_relevance)?)?;
            }
        } else {
            self.add_union_concept(map, concept, inversion, relevance)?;
        }
        Some(())
    }

    pub(super) fn add_union_concept(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) -> Option<()> {
        let subconcepts = concept.0.subconcepts.clone();
        if matches!(concept.0.kind, ConceptKind::Unordered) && subconcepts.len() == 1 {
            let (child, child_relevance) = subconcepts.into_iter().next().unwrap();
            self.add_union_concept(map, child, inversion, relevance.checked_mul(child_relevance)?)?;
        } else {
            self.add_relevance(map, concept, inversion, relevance)?;
        }
        Some(())
    }

    fn add_relevance(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, mut relevance: Relevance) -> Option<()> {
        if inversion {
            relevance = relevance.checked_neg()?;
        }

        if let Some(current) = map.get(&concept).copied() {
            let current = current.checked_add(relevance)?;
            if current.is_empty() {
                map.remove(&concept);
            } else {
                map.insert(concept, current);
            }
        } else if !relevance.is_empty() {
            map.insert(concept, relevance);
        }
        Some(())
    }
}
