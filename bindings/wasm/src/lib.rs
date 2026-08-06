#![forbid(unsafe_code)]

use pangine::{ConceptId, ConceptKind, Pangine, Relevance};
use serde::Serialize;
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionView {
    command: String,
    canonical: String,
    console_lines: Vec<String>,
    current_root: Option<usize>,
    concept_count: usize,
    nodes: Vec<ConceptNode>,
    edges: Vec<ConceptEdge>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConceptNode {
    id: usize,
    kind: &'static str,
    label: String,
    canonical: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConceptEdge {
    id: String,
    source: usize,
    target: usize,
    role: &'static str,
    owner: Option<usize>,
    x_coefficient: f32,
}

struct SessionCore {
    engine: Pangine,
    current: Option<ConceptId>,
}

struct GraphBuilder<'a> {
    engine: &'a Pangine,
    visited: BTreeSet<usize>,
    nodes: Vec<ConceptNode>,
    edges: Vec<ConceptEdge>,
}

impl Default for SessionCore {
    fn default() -> Self {
        Self { engine: Pangine::new(), current: None }
    }
}

impl SessionCore {
    fn execute(&mut self, command: &str) -> Result<String, String> {
        self.current = self.engine.reference_concept(command).map_err(|error| error.to_string())?;
        self.serialize(command)
    }

    fn snapshot(&self) -> Result<String, String> {
        self.serialize("")
    }

    fn serialize(&self, command: &str) -> Result<String, String> {
        let current = self.current.as_ref();
        let mut graph = GraphBuilder::new(&self.engine);
        if let Some(root) = current {
            graph.visit(root);
        }
        let (nodes, edges) = graph.finish();

        let view = ExecutionView {
            command: command.to_owned(),
            canonical: current.map_or_else(|| "[]".to_owned(), |concept| self.engine.format_concept(concept, false)),
            console_lines: self.engine.debug_console_lines(current),
            current_root: current.map(ConceptId::index),
            concept_count: self.engine.concept_count(),
            nodes,
            edges,
        };

        serde_json::to_string(&view).map_err(|error| error.to_string())
    }
}

impl<'a> GraphBuilder<'a> {
    fn new(engine: &'a Pangine) -> Self {
        Self { engine, visited: BTreeSet::new(), nodes: Vec::new(), edges: Vec::new() }
    }

    fn finish(self) -> (Vec<ConceptNode>, Vec<ConceptEdge>) {
        (self.nodes, self.edges)
    }

    fn visit(&mut self, concept: &ConceptId) {
        if !self.visited.insert(concept.index()) {
            return;
        }

        let Some(kind) = self.engine.concept_kind(concept).cloned() else {
            return;
        };
        let (kind_name, label) = match &kind {
            ConceptKind::Named(name) => ("named", name.clone()),
            ConceptKind::Percept { name } => ("percept", name.clone()),
            ConceptKind::Unordered => ("unordered", "unordered composition".to_owned()),
            ConceptKind::Ordered { .. } => ("ordered", "ordered composition".to_owned()),
        };

        self.nodes.push(ConceptNode { id: concept.index(), kind: kind_name, label, canonical: self.engine.format_concept(concept, false) });

        match kind {
            ConceptKind::Named(_) => {}
            ConceptKind::Percept { .. } => {
                for root in self.engine.get_percept_roots(concept).unwrap_or_default() {
                    self.add_edge(concept, &root, "root", None, 0, Relevance::DEFAULT);
                }
            }
            ConceptKind::Unordered => {
                for (index, (relevance, child)) in self.engine.get_relevance_map(concept).into_iter().enumerate() {
                    self.add_edge(concept, &child, "member", Some(concept.index()), index, relevance);
                }
            }
            ConceptKind::Ordered { components } => {
                for (index, component) in components.iter().enumerate() {
                    self.add_edge(concept, component, "component", Some(concept.index()), index, Relevance::DEFAULT);
                }

                for (index, adjacent) in components.windows(2).enumerate() {
                    self.add_edge(&adjacent[0], &adjacent[1], "sequence", Some(concept.index()), index, Relevance::DEFAULT);
                }
            }
        }
    }

    fn add_edge(&mut self, source: &ConceptId, target: &ConceptId, role: &'static str, owner: Option<usize>, ordinal: usize, relevance: Relevance) {
        self.visit(source);
        self.visit(target);
        self.edges.push(ConceptEdge {
            id: format!("{}-{role}-{ordinal}-{}", owner.unwrap_or(source.index()), target.index()),
            source: source.index(),
            target: target.index(),
            role,
            owner,
            x_coefficient: relevance.x_coefficient,
        });
    }
}

/// A browser-local Pangine engine and disposable visualization view.
#[wasm_bindgen]
pub struct PangineSession {
    core: SessionCore,
}

#[wasm_bindgen]
impl PangineSession {
    /// Creates an empty browser-local Pangine session.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { core: SessionCore::default() }
    }

    /// Executes Pangine syntax and returns its console output and graph view as JSON.
    pub fn execute(&mut self, command: &str) -> Result<String, JsValue> {
        self.core.execute(command).map_err(|error| JsValue::from_str(&error))
    }

    /// Returns the current disposable graph view as JSON.
    pub fn snapshot(&self) -> Result<String, JsValue> {
        self.core.snapshot().map_err(|error| JsValue::from_str(&error))
    }

    /// Replaces the browser-local engine with a new empty session.
    pub fn reset(&mut self) -> Result<String, JsValue> {
        self.core = SessionCore::default();
        self.snapshot()
    }
}

impl Default for PangineSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_real_pangine_and_exposes_canonical_structure() {
        let mut session = SessionCore::default();
        session.execute("[cat]").unwrap();
        let json = session.execute("[cat]->[eats]->[food]").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(view["canonical"], "{[cat]->[eats]->[food]}");
        let ordered = view["nodes"].as_array().unwrap().iter().find(|node| node["kind"] == "ordered").unwrap();
        let ordered_id = ordered["id"].as_u64().unwrap();
        let edges = view["edges"].as_array().unwrap();
        assert_eq!(edges.iter().filter(|edge| edge["role"] == "component" && edge["owner"] == ordered_id).count(), 3);
        assert_eq!(edges.iter().filter(|edge| edge["role"] == "sequence" && edge["owner"] == ordered_id).count(), 2);
    }

    #[test]
    fn unordered_compositions_expose_members_without_a_synthetic_union_node_contract() {
        let mut session = SessionCore::default();
        let json = session.execute("[cat][dog]").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        let unordered = view["nodes"].as_array().unwrap().iter().find(|node| node["kind"] == "unordered").unwrap();
        let unordered_id = unordered["id"].as_u64().unwrap();
        let member_edges =
            view["edges"].as_array().unwrap().iter().filter(|edge| edge["role"] == "member" && edge["owner"] == unordered_id).collect::<Vec<_>>();
        assert_eq!(member_edges.len(), 2);
        for edge in member_edges {
            assert_eq!(edge["xCoefficient"], 1.0);
        }
    }

    #[test]
    fn reset_returns_to_null() {
        let mut session = SessionCore::default();
        session.execute("['memory'] = [cat]").unwrap();
        session = SessionCore::default();
        let json = session.snapshot().unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(view["canonical"], "[]");
        assert_eq!(view["nodes"], serde_json::json!([]));
    }

    #[test]
    fn current_output_follows_the_global_live_concept_contract() {
        let mut session = SessionCore::default();
        session.execute("[cat]").unwrap();
        let json = session.execute("$['*']").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(view["canonical"], "[cat]");
        assert_eq!(view["consoleLines"], serde_json::json!(["  [cat]"]));
    }

    #[test]
    fn graph_contains_only_the_current_command_output() {
        let mut session = SessionCore::default();
        session.execute("[cat][eats]").unwrap();
        let json = session.execute("[dog]->[runs]").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();
        let nodes = view["nodes"].as_array().unwrap();

        assert!(nodes.iter().any(|node| node["canonical"] == "[dog]"));
        assert!(nodes.iter().any(|node| node["canonical"] == "[runs]"));
        assert!(nodes.iter().any(|node| node["canonical"] == "{[dog]->[runs]}"));
        assert!(!nodes.iter().any(|node| node["canonical"] == "[cat]"));
        assert!(!nodes.iter().any(|node| node["canonical"] == "[eats]"));
        assert!(!nodes.iter().any(|node| node["canonical"] == "[cat][eats]"));
    }
}
