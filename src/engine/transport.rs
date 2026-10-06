//! Moves Concepts between engines as a graph in which each distinct Concept
//! appears once, so structure that a Concept shares stays shared instead of
//! repeating as it does in formatted text.

use super::{ConceptId, ConceptKind, ConceptMap, Pangine};
use crate::Relevance;
use std::collections::BTreeMap;

/// Concepts and everything they contain, as plain data that another engine
/// can rebuild. Each node refers only to nodes before it, and the roots are
/// the Concepts that were exported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConceptGraph {
    nodes: Vec<ConceptNode>,
    roots: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConceptNode {
    Named(String),
    Percept(String),
    Ordered(Vec<usize>),
    Unordered(Vec<(usize, i64)>),
}

impl ConceptGraph {
    /// Returns how many distinct Concepts the graph holds.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }
}

impl Pangine {
    /// Exports one owned Concept as a graph.
    pub(super) fn export_graph(&self, concept: &ConceptId) -> Option<ConceptGraph> {
        self.export_graphs(std::slice::from_ref(concept))
    }

    /// Exports owned Concepts as one graph, visiting each distinct Concept
    /// once, so Concepts that share structure share its nodes. A Percept
    /// travels by name; its value stays in this engine.
    pub(super) fn export_graphs(&self, concepts: &[ConceptId]) -> Option<ConceptGraph> {
        if concepts.iter().any(|concept| !self.owns(concept)) {
            return None;
        }

        let mut indexes = BTreeMap::<ConceptId, usize>::new();
        let mut nodes = Vec::new();
        for concept in concepts {
            // A Concept is pushed again above its children, so it is exported
            // after every node it refers to. The explicit stack keeps deep
            // Concepts from exhausting the call stack.
            let mut pending = vec![(concept.clone(), false)];
            while let Some((pending_concept, children_exported)) = pending.pop() {
                if indexes.contains_key(&pending_concept) {
                    continue;
                }
                if !children_exported {
                    let children = pending_concept
                        .0
                        .children()
                        .filter(|(child, _)| !indexes.contains_key(*child))
                        .map(|(child, _)| (child.clone(), false))
                        .collect::<Vec<_>>();
                    pending.push((pending_concept, true));
                    pending.extend(children);
                    continue;
                }

                let node = match &pending_concept.0.kind {
                    ConceptKind::Named(name) => ConceptNode::Named(name.clone()),
                    ConceptKind::Percept { name } => ConceptNode::Percept(name.clone()),
                    ConceptKind::Ordered { components } => ConceptNode::Ordered(components.iter().map(|component| indexes[component]).collect()),
                    ConceptKind::Unordered => {
                        ConceptNode::Unordered(pending_concept.0.subconcepts.iter().map(|(member, relevance)| (indexes[member], relevance.count())).collect())
                    }
                };
                indexes.insert(pending_concept, nodes.len());
                nodes.push(node);
            }
        }
        Some(ConceptGraph { roots: concepts.iter().map(|concept| indexes[concept]).collect(), nodes })
    }

    /// Rebuilds the Concept of a graph with one root.
    pub(super) fn import_graph(&mut self, graph: &ConceptGraph) -> Option<ConceptId> {
        let [concept] = <[ConceptId; 1]>::try_from(self.import_graphs(graph)?).ok()?;
        Some(concept)
    }

    /// Rebuilds a graph's root Concepts in this engine. Every node is
    /// interned, so a Concept this engine already holds comes back as the
    /// same handle. A graph with a node that refers forward, a composition too
    /// small to be one, or a member without evidence imports nothing.
    pub(super) fn import_graphs(&mut self, graph: &ConceptGraph) -> Option<Vec<ConceptId>> {
        let mut imported: Vec<ConceptId> = Vec::with_capacity(graph.nodes.len());
        for node in &graph.nodes {
            let concept = match node {
                ConceptNode::Named(name) => self.reference_name(name),
                ConceptNode::Percept(name) => self.reference_percept(name),
                ConceptNode::Ordered(components) => {
                    let components = components.iter().map(|index| imported.get(*index).cloned()).collect::<Option<Vec<_>>>()?;
                    if components.len() < 2 {
                        return None;
                    }
                    self.reference_ordered(components)
                }
                ConceptNode::Unordered(members) => {
                    let mut map = ConceptMap::new();
                    for (index, count) in members {
                        let relevance = Relevance::new(*count);
                        if relevance.is_empty() || map.insert(imported.get(*index)?.clone(), relevance).is_some() {
                            return None;
                        }
                    }
                    self.reference_map(&map)?
                }
            };
            imported.push(concept);
        }
        graph.roots.iter().map(|root| imported.get(*root).cloned()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graph_rebuilds_the_same_concepts_in_another_engine() {
        let mut source = Pangine::new();
        for text in [
            "[cat]",
            "{memory}",
            "{*}",
            r#"["C:\\Library\\Track 01.wav"]"#,
            r#"[""]"#,
            "x3[a]![b]",
            "!([a]->[b])",
            "[a]->([b][c])->x2([d]->[e])",
            "([a]->[a])([b]->{x})",
        ] {
            let concept = must_ref(&mut source, text);
            let graph = source.export_graph(&concept).expect("owned Concept");
            let mut target = Pangine::new();
            let imported = target.import_graph(&graph).expect("well-formed graph");
            assert_eq!(target.format_concept(&imported, false), source.format_concept(&concept, false), "{text}");
            assert_eq!(source.import_graph(&graph), Some(concept), "importing into the same engine returns the same Concept: {text}");
        }
        assert!(Pangine::new().export_graph(&must_ref(&mut source, "[cat]")).is_none(), "a foreign Concept exports nothing");
    }

    #[test]
    fn a_graph_holds_each_distinct_concept_once() {
        // Each level repeats the level below twice, so formatted text doubles
        // with every level while the graph gains one node.
        let mut pangine = Pangine::new();
        let mut concept = must_ref(&mut pangine, "[leaf]");
        for _ in 0..12 {
            concept = pangine.compose_ordered(&[concept.clone(), concept.clone()]).unwrap().unwrap();
        }
        let graph = pangine.export_graph(&concept).unwrap();
        assert_eq!(graph.len(), 13);
        assert!(pangine.format_concept(&concept, false).len() > 4096 * "[leaf]".len());

        let mut other = Pangine::new();
        let imported = other.import_graph(&graph).unwrap();
        assert_eq!(other.format_concept(&imported, false), pangine.format_concept(&concept, false));
    }

    #[test]
    fn concepts_exported_together_share_their_common_structure() {
        let mut source = Pangine::new();
        let concepts =
            ["[cat]->[eats]->[fish]", "[dog]->[eats]->[fish]", "([cat]->[eats]->[fish])([dog]->[eats]->[fish])"].map(|text| must_ref(&mut source, text));
        let together = source.export_graphs(&concepts).unwrap();
        let apart = concepts.iter().map(|concept| source.export_graph(concept).unwrap().len()).sum::<usize>();
        // [cat], [dog], [eats], [fish], the two relations, and their union.
        assert_eq!(together.len(), 7);
        assert!(together.len() < apart);

        let mut target = Pangine::new();
        let imported = target.import_graphs(&together).unwrap();
        let spellings = |pangine: &Pangine, concepts: &[ConceptId]| concepts.iter().map(|concept| pangine.format_concept(concept, false)).collect::<Vec<_>>();
        assert_eq!(spellings(&target, &imported), spellings(&source, &concepts));
        assert_eq!(target.import_graph(&together), None, "a graph with several roots is not one Concept");
    }

    #[test]
    fn a_malformed_graph_imports_nothing() {
        let named = |name: &str| ConceptNode::Named(name.to_owned());
        for graph in [
            ConceptGraph { nodes: vec![ConceptNode::Ordered(vec![1, 2]), named("a"), named("b")], roots: vec![0] },
            ConceptGraph { nodes: vec![named("a"), ConceptNode::Ordered(vec![0])], roots: vec![1] },
            ConceptGraph { nodes: vec![named("a"), ConceptNode::Unordered(vec![(0, 0)])], roots: vec![1] },
            ConceptGraph { nodes: vec![named("a"), ConceptNode::Unordered(vec![(0, 1), (0, 2)])], roots: vec![1] },
            ConceptGraph { nodes: vec![named("a")], roots: vec![3] },
        ] {
            assert_eq!(Pangine::new().import_graphs(&graph), None, "{graph:?}");
        }
    }

    fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
        pangine
            .reference_concept(input)
            .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
            .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
    }
}
