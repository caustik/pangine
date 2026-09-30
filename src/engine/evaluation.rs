//! Explicit evaluation of Percept references.

use super::{ConceptId, ConceptKind, ConceptMap, Pangine};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
enum PerceptEvaluation {
    All,
    AssignedValues,
}

// Recursive evaluation.
impl Pangine {
    /// Evaluates the Percepts in an owned Concept.
    ///
    /// Returns no Concept when the input is foreign or a required Percept has
    /// no current value. A repeated Percept in a reference cycle remains as the
    /// point where recursion stops. Ordinary values are followed recursively.
    /// When every Percept in the Concept is linked to one question, the result
    /// is projected from that question's correlated answer without changing it.
    /// That projection substitutes its bindings once, so represented Percepts
    /// supplied by a source remain references and can be evaluated again.
    pub fn evaluate_concept(&mut self, concept: &ConceptId) -> Option<ConceptId> {
        if !self.owns(concept) {
            return None;
        }
        if let Some((_, live)) = self.shared_live_answer(concept) {
            return live.answer.materialize(self, concept);
        }
        self.evaluate_concept_inner(concept, &mut BTreeSet::new(), PerceptEvaluation::All)
    }

    pub(super) fn evaluate_experience_concept(&mut self, concept: &ConceptId) -> Option<ConceptId> {
        self.evaluate_concept_inner(concept, &mut BTreeSet::new(), PerceptEvaluation::AssignedValues)
    }

    fn evaluate_concept_inner(
        &mut self,
        concept: &ConceptId,
        visited_percepts: &mut BTreeSet<ConceptId>,
        percept_evaluation: PerceptEvaluation,
    ) -> Option<ConceptId> {
        match &concept.0.kind {
            ConceptKind::Named(_) => Some(concept.clone()),
            ConceptKind::Percept { .. } => {
                if matches!(percept_evaluation, PerceptEvaluation::AssignedValues) && !self.current_value_percepts.contains(&concept.index()) {
                    return Some(concept.clone());
                }
                if !visited_percepts.insert(concept.clone()) {
                    return Some(concept.clone());
                }

                let evaluated = self.get_value(concept).and_then(|value| {
                    if self.is_global_percept(concept) {
                        self.evaluate_transient_concept(&value, visited_percepts, percept_evaluation)
                    } else {
                        self.evaluate_concept_inner(&value, visited_percepts, percept_evaluation)
                    }
                });
                visited_percepts.remove(concept);
                evaluated
            }
            ConceptKind::Unordered => {
                let evaluated = self.evaluate_subconcepts(concept, visited_percepts, percept_evaluation)?;
                self.reference_map(&evaluated)
            }
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                let mut evaluated = Vec::with_capacity(components.len());
                for component in components {
                    evaluated.push(self.evaluate_concept_inner(&component, visited_percepts, percept_evaluation)?);
                }
                Some(self.reference_ordered(evaluated))
            }
        }
    }

    fn evaluate_transient_concept(
        &mut self,
        concept: &ConceptId,
        visited_percepts: &mut BTreeSet<ConceptId>,
        percept_evaluation: PerceptEvaluation,
    ) -> Option<ConceptId> {
        if matches!(concept.0.kind, ConceptKind::Unordered) {
            let evaluated = self.evaluate_subconcepts(concept, visited_percepts, percept_evaluation)?;
            self.reference_transient_map(evaluated)
        } else {
            self.evaluate_concept_inner(concept, visited_percepts, percept_evaluation)
        }
    }

    fn evaluate_subconcepts(
        &mut self,
        concept: &ConceptId,
        visited_percepts: &mut BTreeSet<ConceptId>,
        percept_evaluation: PerceptEvaluation,
    ) -> Option<ConceptMap> {
        let mut evaluated = ConceptMap::new();
        for (child, relevance) in concept.0.subconcepts.clone() {
            if let Some(child) = self.evaluate_concept_inner(&child, visited_percepts, percept_evaluation) {
                self.add_union_concept(&mut evaluated, child, false, relevance)?;
            }
        }
        Some(evaluated)
    }
}
