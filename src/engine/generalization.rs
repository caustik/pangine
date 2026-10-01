//! The generalized level of a graded question: one remembered experience that
//! matches the question's shape while some of its names differ.

use super::{Completion, CompletionGrade, ConceptId, ConceptKind, ConceptMap, Pangine, QuestionSnapshot, QuestionSource};
use std::collections::{BTreeMap, BTreeSet};

/// One renamable name occurrence: the hidden blank that replaced it and the
/// question name it replaced.
struct NameOccurrence {
    blank: ConceptId,
    name: ConceptId,
}

impl Pangine {
    /// Returns the generalized completions of `question`.
    ///
    /// Every name that fills an ordered position in the question becomes its
    /// own hidden blank, and each remembered experience is matched alone
    /// against that opened shape, with every clause on a different part of the
    /// experience. A match's distance counts the name occurrences that differ
    /// from the question, plus one for each extra value that a name the
    /// question repeats takes. An experience enters only at its smallest
    /// distance, only when that distance is above zero, and only when it
    /// agrees with the question on at least one name occurrence. An answer
    /// that sits where a renamed question name sits maps back to that name.
    pub(super) fn generalized_completions(
        &mut self,
        question: &ConceptId,
        snapshot: impl FnOnce(&mut Self, &ConceptId) -> QuestionSnapshot,
    ) -> Option<Vec<Completion>> {
        let mut outputs = BTreeSet::new();
        self.collect_output_percepts(question, &mut outputs);
        let mut occurrences = Vec::new();
        let opened = self.open_ordered_names(question, false, &outputs, &mut occurrences, &mut 0)?;
        if occurrences.is_empty() {
            return Some(Vec::new());
        }

        let mut experiences = BTreeMap::<QuestionSource, QuestionSnapshot>::new();
        for (key, routes) in snapshot(self, &opened) {
            experiences.entry(key.0.clone()).or_default().insert(key, routes);
        }

        let mut completions = Vec::new();
        for experience in experiences.into_values() {
            let mut closest: Option<(usize, Vec<Completion>)> = None;
            for row in self.complete_question_snapshot(&opened, &experience, false).completions {
                if !uses_distinct_parts(&row) {
                    continue;
                }
                let Some(distance) = generalized_distance(&row, &occurrences) else {
                    continue;
                };
                match &mut closest {
                    Some((closest_distance, rows)) if distance == *closest_distance => rows.push(row),
                    Some((closest_distance, _)) if distance > *closest_distance => {}
                    _ => closest = Some((distance, vec![row])),
                }
            }
            if let Some((distance, rows)) = closest.filter(|(distance, _)| *distance > 0) {
                completions.extend(rows.iter().map(|row| respecialized(row, distance, &occurrences, &outputs)));
            }
        }
        Some(completions)
    }

    // Names that fill ordered positions become hidden blanks. Members of an
    // unordered group keep matching by identity, so their remainders behave
    // as they do under `@`.
    fn open_ordered_names(
        &mut self,
        concept: &ConceptId,
        ordered_position: bool,
        reserved: &BTreeSet<ConceptId>,
        occurrences: &mut Vec<NameOccurrence>,
        next_blank: &mut usize,
    ) -> Option<ConceptId> {
        match &concept.0.kind {
            ConceptKind::Named(_) if ordered_position => {
                let blank = self.hidden_blank(reserved, next_blank);
                occurrences.push(NameOccurrence { blank: blank.clone(), name: concept.clone() });
                Some(blank)
            }
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                let opened = components
                    .iter()
                    .map(|component| self.open_ordered_names(component, true, reserved, occurrences, next_blank))
                    .collect::<Option<Vec<_>>>()?;
                Some(self.reference_ordered(opened))
            }
            ConceptKind::Unordered => {
                let members = concept.0.subconcepts.clone();
                let mut opened = ConceptMap::new();
                for (member, relevance) in members {
                    opened.insert(self.open_ordered_names(&member, false, reserved, occurrences, next_blank)?, relevance);
                }
                self.reference_map(&opened)
            }
            ConceptKind::Named(_) | ConceptKind::Percept { .. } => Some(concept.clone()),
        }
    }

    fn hidden_blank(&mut self, reserved: &BTreeSet<ConceptId>, next_blank: &mut usize) -> ConceptId {
        loop {
            let blank = self.reference_percept(&format!("generalized-{next_blank}"));
            *next_blank += 1;
            if !reserved.contains(&blank) {
                return blank;
            }
        }
    }
}

// One experience supplies each clause from a different part of itself, so a
// single remembered relationship cannot stand in for two parts of a question.
fn uses_distinct_parts(row: &Completion) -> bool {
    let mut parts = BTreeSet::new();
    row.evidence().iter().all(|fragment| parts.insert(fragment.matched()))
}

fn generalized_distance(row: &Completion, occurrences: &[NameOccurrence]) -> Option<usize> {
    let mut differing = 0;
    let mut agreeing = false;
    for occurrence in occurrences {
        if row.binding(&occurrence.blank)? == &occurrence.name {
            agreeing = true;
        } else {
            differing += 1;
        }
    }
    let split_values = values_by_name(row, occurrences).values().map(|values| values.len() - 1).sum::<usize>();
    agreeing.then_some(differing + split_values)
}

fn values_by_name<'a>(row: &'a Completion, occurrences: &'a [NameOccurrence]) -> BTreeMap<&'a ConceptId, BTreeSet<&'a ConceptId>> {
    let mut values = BTreeMap::<&ConceptId, BTreeSet<&ConceptId>>::new();
    for occurrence in occurrences {
        if let Some(value) = row.binding(&occurrence.blank) {
            values.entry(&occurrence.name).or_default().insert(value);
        }
    }
    values
}

fn respecialized(row: &Completion, distance: usize, occurrences: &[NameOccurrence], outputs: &BTreeSet<ConceptId>) -> Completion {
    let values = values_by_name(row, occurrences);
    let assignment = outputs
        .iter()
        .filter_map(|output| {
            let value = row.binding(output)?;
            let mut names = values.iter().filter(|(name, values)| **name != value && values.len() == 1 && values.contains(value)).map(|(name, _)| *name);
            let answer = match (names.next(), names.next()) {
                (Some(name), None) => name.clone(),
                _ => value.clone(),
            };
            Some((output.clone(), answer))
        })
        .collect();
    Completion::from_parts(assignment, row.evidence().to_vec(), Vec::new(), CompletionGrade::Generalized { distance })
}
