use super::{ConceptId, ConceptKind, ConceptShape};
use std::collections::{BTreeMap, BTreeSet};

/// Recursive lookup postings for the complete sources retained by one Percept.
///
/// The postings are intentionally source preserving. Recursive Concepts and
/// possible ordered-window shapes locate complete source Concepts rather than
/// becoming independent experiences.
#[derive(Default)]
pub(super) struct PerceptQuestionIndex {
    sources: BTreeSet<ConceptId>,
    sources_by_shape: BTreeMap<ConceptShape, BTreeSet<ConceptId>>,
    sources_by_anchor: BTreeMap<ConceptId, BTreeSet<ConceptId>>,
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

    pub(super) fn contains_source(&self, source: &ConceptId) -> bool {
        self.sources.contains(source)
    }

    pub(super) fn candidate_sources(&self, patterns: &BTreeSet<ConceptId>) -> BTreeSet<ConceptId> {
        if patterns.iter().any(|pattern| matches!(pattern.0.kind, ConceptKind::Percept { .. })) {
            return self.sources.clone();
        }

        let mut candidates = BTreeSet::new();
        for pattern in patterns {
            let Some(shape_sources) = self.sources_by_shape.get(&pattern.0.shape()) else {
                continue;
            };
            let anchors = required_anchors(pattern);

            let mut postings = vec![shape_sources];
            let mut missing_anchor = false;
            for anchor in &anchors {
                let Some(anchor_sources) = self.sources_by_anchor.get(anchor) else {
                    missing_anchor = true;
                    break;
                };
                postings.push(anchor_sources);
            }
            if missing_anchor {
                continue;
            }

            postings.sort_by_key(|posting| posting.len());
            let mut pattern_candidates = postings[0].clone();
            pattern_candidates.retain(|source| postings[1..].iter().all(|posting| posting.contains(source)));
            candidates.extend(pattern_candidates);
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
                    self.sources_by_shape.entry(ConceptShape::Ordered(width)).or_default().insert(source.clone());
                }
            }
            ConceptKind::Percept { .. } | ConceptKind::Unordered => {}
        }

        for (child, _) in concept.0.children() {
            self.insert_source_concept(source, child, visited);
        }
    }
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
