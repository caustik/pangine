#![forbid(unsafe_code)]

use pangine::{AnswerPossibility, CompletionGrade, ConceptId, ConceptKind, Pangine, Relevance};
use serde::Serialize;
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionView {
    command: String,
    canonical: String,
    console_lines: Vec<String>,
    current_concept: Option<usize>,
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
    x_coefficient: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionView {
    projection: String,
    possibilities: Vec<PossibilityView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PossibilityView {
    value: String,
    count: String,
    probability: f64,
    probability_text: String,
    complete_rows: usize,
    top_tie: bool,
    support: Vec<SupportView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportView {
    grade: &'static str,
    distance: Option<usize>,
    weight: String,
    sources: Vec<SourceView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceView {
    subject: String,
    concept: String,
    count: String,
}

struct SessionCore {
    engine: Pangine,
    current: Option<ConceptId>,
    // The statement that produced `current`. Console commands leave both alone.
    command: String,
}

struct GraphBuilder<'a> {
    engine: &'a Pangine,
    visited: BTreeSet<usize>,
    nodes: Vec<ConceptNode>,
    edges: Vec<ConceptEdge>,
}

impl Default for SessionCore {
    fn default() -> Self {
        Self { engine: Pangine::new(), current: None, command: String::new() }
    }
}

impl SessionCore {
    fn execute(&mut self, command: &str) -> Result<String, String> {
        if let Some(output) = self.engine.debug_console_command(command) {
            return self.serialize(output?);
        }
        self.current = self.engine.reference_concept(command).map_err(|error| error.to_string())?;
        self.command = command.to_owned();
        self.serialize(self.engine.debug_console_lines(self.current.as_ref()))
    }

    fn run(&mut self, command: &str) -> Result<String, String> {
        let concept = self.engine.reference_concept(command).map_err(|error| error.to_string())?;
        Ok(concept.map_or_else(|| "[]".to_owned(), |concept| self.engine.format_concept(&concept, false)))
    }

    fn inspect(&mut self, operand: &str) -> Result<String, String> {
        let projection =
            self.engine.reference_concept(operand).map_err(|error| error.to_string())?.ok_or_else(|| "inspect expects one linked Answer operand".to_owned())?;
        let answer = self.engine.answer_view(&projection).ok_or_else(|| "operand is not part of one linked Answer".to_owned())?;
        let possibilities = answer.possibilities(&mut self.engine).ok_or_else(|| "linked Answer could not be inspected".to_owned())?;
        let view = InspectionView {
            projection: self.engine.format_concept(&projection, false),
            possibilities: possibilities.iter().map(|possibility| self.possibility_view(possibility)).collect(),
        };
        serde_json::to_string(&view).map_err(|error| error.to_string())
    }

    fn possibility_view(&self, possibility: &AnswerPossibility) -> PossibilityView {
        let support = possibility
            .support()
            .iter()
            .map(|support| {
                let (grade, distance) = match support.grade() {
                    CompletionGrade::Exact => ("exact", None),
                    CompletionGrade::Composed => ("composed", None),
                    CompletionGrade::Generalized { distance } => ("generalized", Some(distance)),
                };
                let sources = support
                    .sources()
                    .iter()
                    .map(|source| SourceView {
                        subject: self.engine.format_concept(source.subject(), false),
                        concept: self.engine.format_concept(source.concept(), false),
                        count: source.relevance().count().to_string(),
                    })
                    .collect();
                SupportView { grade, distance, weight: support.weight().count().to_string(), sources }
            })
            .collect();
        PossibilityView {
            value: self.engine.format_concept(possibility.value(), false),
            count: possibility.strength().count().to_string(),
            probability: possibility.probability().as_f64(),
            probability_text: possibility.probability().to_string(),
            complete_rows: possibility.complete_rows(),
            top_tie: possibility.is_top_tie(),
            support,
        }
    }

    fn snapshot(&self) -> Result<String, String> {
        self.serialize(self.engine.debug_console_lines(self.current.as_ref()))
    }

    fn serialize(&self, console_lines: Vec<String>) -> Result<String, String> {
        let current = self.current.as_ref();
        let mut graph = GraphBuilder::new(&self.engine);
        if let Some(concept) = current {
            graph.visit(concept);
        }
        let (nodes, edges) = graph.finish();

        let view = ExecutionView {
            command: self.command.clone(),
            canonical: current.map_or_else(|| "[]".to_owned(), |concept| self.engine.format_concept(concept, false)),
            console_lines,
            current_concept: current.map(ConceptId::index),
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
                for (index, (relevance, child)) in self.engine.get_relevance_map(concept).into_iter().enumerate() {
                    self.add_edge(concept, &child, "member", Some(concept.index()), index, relevance);
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
            x_coefficient: relevance.count().to_string(),
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

    /// Executes Pangine syntax, or a console command such as `help`,
    /// `inspect operand`, or `seed n`, and returns its console output and
    /// graph view as JSON. A console command keeps the current result and
    /// replaces only the console output.
    pub fn execute(&mut self, command: &str) -> Result<String, JsValue> {
        self.core.execute(command).map_err(|error| JsValue::from_str(&error))
    }

    /// Runs Pangine syntax and returns the result's canonical spelling,
    /// without building the graph view or console output.
    pub fn run(&mut self, command: &str) -> Result<String, JsValue> {
        self.core.run(command).map_err(|error| JsValue::from_str(&error))
    }

    /// Inspects one linked answer and returns its possibilities as JSON, most
    /// probable first. Each has its value, evidence count, probability as a
    /// number and as text, complete rows, and whether it is a top tie, with
    /// the support behind its count: each support's grade, distance, weight,
    /// and sources. Counts are strings, so 64-bit values survive JavaScript.
    pub fn inspect(&mut self, operand: &str) -> Result<String, JsValue> {
        self.core.inspect(operand).map_err(|error| JsValue::from_str(&error))
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

    /// Seeds the generator that `^~` draws from. JavaScript passes the seed as
    /// a BigInt. A new or reset session starts from seed 0.
    pub fn set_sample_seed(&mut self, seed: u64) {
        self.core.engine.set_sample_seed(seed);
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

        assert_eq!(view["canonical"], "[cat]->[eats]->[food]");
        let ordered = view["nodes"].as_array().unwrap().iter().find(|node| node["kind"] == "ordered").unwrap();
        let ordered_id = ordered["id"].as_u64().unwrap();
        let edges = view["edges"].as_array().unwrap();
        assert_eq!(edges.iter().filter(|edge| edge["role"] == "component" && edge["owner"] == ordered_id).count(), 3);
        assert_eq!(edges.iter().filter(|edge| edge["role"] == "sequence" && edge["owner"] == ordered_id).count(), 2);
    }

    #[test]
    fn escaped_text_keeps_its_exact_label_and_canonical_spelling() {
        let mut session = SessionCore::default();
        let json = session.execute(r#"["C:\\Library\\Track 01.wav"]"#).unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(view["canonical"], r#"["C:\\Library\\Track 01.wav"]"#);
        assert_eq!(view["nodes"][0]["kind"], "named");
        assert_eq!(view["nodes"][0]["label"], r"C:\Library\Track 01.wav");
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
            assert_eq!(edge["xCoefficient"], "1");
        }
    }

    #[test]
    fn full_width_coefficients_serialize_without_javascript_number_loss() {
        let mut session = SessionCore::default();
        let json = session.execute("x9223372036854775807[cat]").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        let member = view["edges"].as_array().unwrap().iter().find(|edge| edge["role"] == "member").unwrap();
        assert_eq!(member["xCoefficient"], "9223372036854775807");
    }

    #[test]
    fn percept_state_uses_relevance_bearing_member_edges() {
        let mut session = SessionCore::default();
        session.execute("{memory} ~= [cat]").unwrap();
        session.execute("{memory} ~= [cat]").unwrap();
        let json = session.execute("{memory}").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        let percept = view["nodes"].as_array().unwrap().iter().find(|node| node["kind"] == "percept").unwrap();
        let percept_id = percept["id"].as_u64().unwrap();
        let state_member = view["edges"].as_array().unwrap().iter().find(|edge| edge["role"] == "member" && edge["owner"] == percept_id).unwrap();
        assert_eq!(state_member["xCoefficient"], "2");
        assert!(!view["edges"].as_array().unwrap().iter().any(|edge| edge["role"] == "root"));
    }

    #[test]
    fn reset_returns_to_null() {
        let mut session = SessionCore::default();
        session.execute("{memory} = [cat]").unwrap();
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
        let json = session.execute("${*}").unwrap();
        let view: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(view["canonical"], "[cat]");
        assert_eq!(view["consoleLines"], serde_json::json!(["  [cat]"]));
    }

    #[test]
    fn shared_answers_can_be_revealed_and_extended_in_the_browser_runtime() {
        let mut session = SessionCore::default();
        session.execute("([cat]->[eats]->[fish])([dog]->[eats]->[bone]) @ {animal}->[eats]->{food}").unwrap();
        session.execute("([cat]->[lives-in]->[house])([dog]->[lives-in]->[yard]) @ {animal}->[lives-in]->{home}").unwrap();

        let linked: serde_json::Value = serde_json::from_str(&session.execute("&{animal}").unwrap()).unwrap();
        assert_eq!(linked["consoleLines"], serde_json::json!(["  {animal}->[eats]->{food}", "  {animal}->[lives-in]->{home}"]));

        let possibilities: serde_json::Value = serde_json::from_str(&session.execute("$(&{animal})").unwrap()).unwrap();
        assert_eq!(
            possibilities["consoleLines"],
            serde_json::json!(["  ([cat]->[eats]->[fish])([cat]->[lives-in]->[house])", "  ([dog]->[eats]->[bone])([dog]->[lives-in]->[yard])"])
        );
    }

    #[test]
    fn a_seeded_session_repeats_its_draws() {
        let draws = |seed: Option<u64>| {
            let mut session = PangineSession::new();
            if let Some(seed) = seed {
                session.set_sample_seed(seed);
            }
            session.core.execute("{choice} = x2[tea]x3[coffee]").unwrap();
            (0..24)
                .map(|_| {
                    let view: serde_json::Value = serde_json::from_str(&session.core.execute("^~{choice}").unwrap()).unwrap();
                    view["canonical"].as_str().unwrap().to_owned()
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(draws(None), draws(Some(0)), "a new session starts from seed 0");
        assert_eq!(draws(Some(7)), draws(Some(7)));
        assert_ne!(draws(Some(7)), draws(None));
    }

    #[test]
    fn console_commands_run_in_the_workbench_without_replacing_the_result() {
        let mut session = SessionCore::default();
        for statement in ["{world} ~= [morning]->[birds]", "{world} ~= [morning]->[birds]", "{world} ~= [morning]->[traffic]", "{world} @ [morning]->{answer}"]
        {
            session.execute(statement).unwrap();
        }

        let inspected: serde_json::Value = serde_json::from_str(&session.execute("inspect {answer}").unwrap()).unwrap();
        assert_eq!(
            inspected["consoleLines"],
            serde_json::json!([
                "  * +2, p=2/3, 1 row: [birds]",
                "      +2 from {world}: [morning]->[birds]",
                "    +1, p=1/3, 1 row: [traffic]",
                "      +1 from {world}: [morning]->[traffic]"
            ])
        );
        assert_eq!(inspected["command"], "{world} @ [morning]->{answer}");
        assert_eq!(inspected["canonical"], "([morning]->[birds])([morning]->[traffic])");

        let seeded: serde_json::Value = serde_json::from_str(&session.execute("seed 3").unwrap()).unwrap();
        assert_eq!(seeded["consoleLines"], serde_json::json!([]));
        let help: serde_json::Value = serde_json::from_str(&session.execute("help").unwrap()).unwrap();
        assert_eq!(help["consoleLines"][0], "Commands:");
        assert_eq!(session.execute("inspect {missing}"), Err("operand is not part of one linked Answer".to_owned()));
    }

    #[test]
    fn inspect_reports_each_possibility_with_its_graded_support() {
        let mut session = SessionCore::default();
        for statement in [
            "{options} ~= [hall]->[north]->[left]",
            "{options} ~= [hall]->[north]->[right]",
            "{trips} ~= [lobby]->[north]->[left]",
            "{options} @ [hall]->[north]->{way}",
            "{trips} @~ [hall]->[north]->{trip-way}",
            "{way} @+= {trip-way}",
        ] {
            session.run(statement).unwrap();
        }

        let view: serde_json::Value = serde_json::from_str(&session.inspect("{way}").unwrap()).unwrap();
        assert_eq!(view["projection"], "{way}");
        let left = &view["possibilities"][0];
        assert_eq!(left["value"], "[left]");
        assert_eq!(left["count"], "2");
        assert_eq!(left["probabilityText"], "7/12");
        assert!((left["probability"].as_f64().unwrap() - 7.0 / 12.0).abs() < 1e-12);
        assert_eq!(left["completeRows"], 1);
        assert_eq!(left["topTie"], true);
        assert_eq!(left["support"][0]["grade"], "exact");
        assert_eq!(left["support"][0]["distance"], serde_json::Value::Null);
        assert_eq!(
            left["support"][1],
            serde_json::json!({
                "grade": "generalized",
                "distance": 1,
                "weight": "1",
                "sources": [{ "subject": "{trips}", "concept": "[lobby]->[north]->[left]", "count": "1" }]
            })
        );
        assert_eq!(view["possibilities"][1]["probabilityText"], "5/12");
        assert_eq!(session.inspect("{nowhere}"), Err("operand is not part of one linked Answer".to_owned()));
    }

    #[test]
    fn run_returns_the_canonical_result_without_a_graph() {
        let mut session = SessionCore::default();
        assert_eq!(session.run("[cat]->[purrs]"), Ok("[cat]->[purrs]".to_owned()));
        assert_eq!(session.run("{memory} = []"), Ok("[]".to_owned()));
        assert!(session.run("[cat]->").is_err());
        assert!(session.current.is_none(), "run leaves the workbench's current result alone");
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
        assert!(nodes.iter().any(|node| node["canonical"] == "[dog]->[runs]"));
        assert!(!nodes.iter().any(|node| node["canonical"] == "[cat]"));
        assert!(!nodes.iter().any(|node| node["canonical"] == "[eats]"));
        assert!(!nodes.iter().any(|node| node["canonical"] == "[cat][eats]"));
    }
}
