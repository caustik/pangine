//! Choice (`^`): the most probable value, the one with the greatest positive evidence count, wins, and canonical order breaks ties.

use super::{completion::projection_strength, CompletionProjectionSupport, ConceptId, ConceptKind, LiveConceptAnswer, Pangine};

impl Pangine {
    pub(super) fn make_decision(&mut self, concept: &ConceptId) -> Option<ConceptId> {
        if !self.owns(concept) {
            return None;
        }
        if self.shared_live_answer(concept).is_some() {
            return self.choose_from_live_answer(concept);
        }

        let concept = self.get_value(concept)?;
        if !matches!(concept.0.kind, ConceptKind::Unordered) {
            return Some(concept);
        }

        self.select_greatest_positive(concept.0.subconcepts.iter().map(|(candidate, relevance)| (candidate, relevance.count())))
    }

    fn select_greatest_positive<'a>(&self, candidates: impl IntoIterator<Item = (&'a ConceptId, i64)>) -> Option<ConceptId> {
        let mut selected = None;
        for (candidate, count) in candidates {
            if count <= 0 {
                continue;
            }

            let canonical = self.format_concept(candidate, false);
            let replace = match &selected {
                None => true,
                Some((greatest, earliest, _)) => count > *greatest || (count == *greatest && canonical < *earliest),
            };
            if replace {
                selected = Some((count, canonical, candidate));
            }
        }
        selected.map(|(_, _, candidate)| candidate.clone())
    }

    fn choose_from_live_answer(&mut self, template: &ConceptId) -> Option<ConceptId> {
        let (_, live) = self.shared_live_answer(template)?;
        let (selected, answer) = live.answer.choose(self, template)?;
        let next = LiveConceptAnswer::successor(self, live.revision, answer)?;
        self.install_live_answer(next)?;
        Some(selected)
    }

    pub(super) fn select_projection_candidate(&self, support: &CompletionProjectionSupport) -> Option<ConceptId> {
        let candidates = support
            .iter()
            .map(|(candidate, derivations)| projection_strength(derivations).map(|strength| (candidate, strength.count())))
            .collect::<Option<Vec<_>>>()?;
        self.select_greatest_positive(candidates)
    }
}
