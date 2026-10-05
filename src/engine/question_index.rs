use super::{ConceptId, ConceptKind, ConceptShape};
use std::collections::{BTreeMap, BTreeSet};

/// A name at one position of an ordered Concept of one width.
type OrderedPosition = (usize, usize, ConceptId);

/// Recursive lookup postings for the complete sources retained by one Percept.
///
/// The postings are intentionally source preserving. Recursive Concepts and
/// possible ordered-window shapes locate complete source Concepts rather than
/// becoming independent experiences.
#[derive(Default)]
pub(super) struct PerceptQuestionIndex {
    sources: BTreeSet<ConceptId>,
    sources_by_shape: BTreeMap<ConceptShape, BTreeSet<ConceptId>>,
    // Sources holding an ordered Concept longer than the width, so a window
    // of that width can match an ordered question.
    sources_by_window_width: BTreeMap<usize, BTreeSet<ConceptId>>,
    sources_by_anchor: BTreeMap<ConceptId, BTreeSet<ConceptId>>,
    // Sources holding an ordered Concept whose width and position place one
    // name. An ordered question that fixes a name at a position can only match
    // a view of its own width with that name there.
    sources_by_position: BTreeMap<OrderedPosition, BTreeSet<ConceptId>>,
}

impl PerceptQuestionIndex {
    pub(super) fn from_sources<'a>(sources: impl IntoIterator<Item = &'a ConceptId>) -> Self {
        let mut index = Self::default();
        for source in sources {
            index.insert_source(source);
        }
        index
    }

    pub(super) fn insert_source(&mut self, source: &ConceptId) {
        if !self.sources.insert(source.clone()) {
            return;
        }

        let mut visited = BTreeSet::new();
        self.insert_source_concept(source, source, &mut visited);
    }

    pub(super) fn candidate_sources(&self, patterns: &BTreeSet<ConceptId>) -> BTreeSet<ConceptId> {
        if patterns.iter().any(|pattern| matches!(pattern.0.kind, ConceptKind::Percept { .. })) {
            return self.sources.clone();
        }

        let mut candidates = BTreeSet::new();
        for pattern in patterns {
            let Some(anchor_postings) = required_anchors(pattern).iter().map(|anchor| self.sources_by_anchor.get(anchor)).collect::<Option<Vec<_>>>() else {
                continue;
            };

            if let Some(shape_sources) = self.sources_by_shape.get(&pattern.0.shape()) {
                let position_postings = ordered_positions(pattern).map(|position| self.sources_by_position.get(&position)).collect::<Option<Vec<_>>>();
                if let Some(position_postings) = position_postings {
                    candidates.extend(intersection(std::iter::once(shape_sources).chain(anchor_postings.iter().copied()).chain(position_postings)));
                }
            }
            // A window of a longer ordered Concept starts at any offset, so
            // only the pattern's names narrow it.
            if let ConceptShape::Ordered(width) = pattern.0.shape() {
                if let Some(window_sources) = self.sources_by_window_width.get(&width) {
                    candidates.extend(intersection(std::iter::once(window_sources).chain(anchor_postings.iter().copied())));
                }
            }
        }
        candidates
    }

    fn insert_source_concept(&mut self, source: &ConceptId, concept: &ConceptId, visited: &mut BTreeSet<ConceptId>) {
        if !visited.insert(concept.clone()) {
            return;
        }

        self.sources_by_shape.entry(concept.0.shape()).or_default().insert(source.clone());
        match &concept.0.kind {
            ConceptKind::Named(_) => {
                self.sources_by_anchor.entry(concept.clone()).or_default().insert(source.clone());
            }
            ConceptKind::Ordered { components } => {
                for width in 2..components.len() {
                    self.sources_by_window_width.entry(width).or_default().insert(source.clone());
                }
                for position in ordered_positions(concept) {
                    self.sources_by_position.entry(position).or_default().insert(source.clone());
                }
            }
            ConceptKind::Percept { .. } | ConceptKind::Unordered => {}
        }

        for (child, _) in concept.0.children() {
            self.insert_source_concept(source, child, visited);
        }
    }
}

// The names an ordered Concept holds at each of its positions. Only a name
// must match a name in place; a composite component can match with a
// remainder, and a Percept matches anything.
fn ordered_positions(concept: &ConceptId) -> impl Iterator<Item = OrderedPosition> + '_ {
    let components = concept.0.ordered_components().unwrap_or_default();
    components
        .iter()
        .enumerate()
        .filter(|(_, component)| matches!(component.0.kind, ConceptKind::Named(_)))
        .map(move |(position, component)| (components.len(), position, component.clone()))
}

fn intersection<'a>(postings: impl IntoIterator<Item = &'a BTreeSet<ConceptId>>) -> BTreeSet<ConceptId> {
    let mut postings = postings.into_iter().collect::<Vec<_>>();
    postings.sort_by_key(|posting| posting.len());
    let Some((smallest, rest)) = postings.split_first() else {
        return BTreeSet::new();
    };
    smallest.iter().filter(|source| rest.iter().all(|posting| posting.contains(*source))).cloned().collect()
}

fn required_anchors(concept: &ConceptId) -> BTreeSet<ConceptId> {
    if let Some((_, operand)) = concept.0.coefficient_operand() {
        return required_anchors(operand);
    }

    match &concept.0.kind {
        ConceptKind::Named(_) => BTreeSet::from([concept.clone()]),
        ConceptKind::Percept { .. } => BTreeSet::new(),
        ConceptKind::Ordered { components } => components.iter().flat_map(required_anchors).collect(),
        ConceptKind::Unordered => {
            let members = concept.0.subconcepts.keys().collect::<Vec<_>>();
            if members.iter().any(|member| matches!(member.0.kind, ConceptKind::Percept { .. })) {
                return members.into_iter().filter(|member| !matches!(member.0.kind, ConceptKind::Percept { .. })).flat_map(required_anchors).collect();
            }

            let mut outputs = BTreeSet::new();
            collect_percepts(concept, &mut outputs);
            outputs
                .into_iter()
                .flat_map(|output| {
                    let mut alternatives = members.iter().filter(|member| contains_percept(member, &output)).map(|member| required_anchors(member));
                    let mut common = alternatives.next().unwrap_or_default();
                    for alternative in alternatives {
                        common.retain(|anchor| alternative.contains(anchor));
                    }
                    common
                })
                .collect()
        }
    }
}

fn collect_percepts(concept: &ConceptId, percepts: &mut BTreeSet<ConceptId>) {
    if matches!(concept.0.kind, ConceptKind::Percept { .. }) {
        percepts.insert(concept.clone());
        return;
    }

    for (child, _) in concept.0.children() {
        collect_percepts(child, percepts);
    }
}

fn contains_percept(concept: &ConceptId, percept: &ConceptId) -> bool {
    concept == percept || concept.0.children().any(|(child, _)| contains_percept(child, percept))
}
