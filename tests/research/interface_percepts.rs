//! Warning checks for Rust-backed input and output Percepts.
//!
//! The bridge in this file is test-local. It measures whether current public
//! operations can capture one complete Rust input frame into experience and
//! return Pangine's selected result through an output Percept. It is not a
//! callback API, event loop, or settled input-request rule.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, VecDeque};

const DECISION_PROGRAM: &str = "
    ['lived-experience'] @
      [observation]->[context]->$['rust-context-input']->[reading]->['decision-candidate']->[result]->$['rust-result-input'];
    ['decision-output'] = ^['decision-candidate']
";

#[derive(Clone, Copy)]
struct RustFrame<'a> {
    context: &'a str,
    reading: &'a str,
    result: &'a str,
}

struct RustPerceptBridge {
    context_input: ConceptId,
    reading_input: ConceptId,
    result_input: ConceptId,
    experience_template: ConceptId,
    experience: ConceptId,
    output: ConceptId,
    delivered_outputs: Vec<Option<String>>,
}

impl RustPerceptBridge {
    fn new(pangine: &mut Pangine) -> Self {
        must_run(
            pangine,
            "['rust-experience-template'] =
               [observation]->[context]->['rust-context-input']->[reading]->['rust-reading-input']->[result]->['rust-result-input']",
        );
        Self {
            context_input: pangine.reference_percept("rust-context-input"),
            reading_input: pangine.reference_percept("rust-reading-input"),
            result_input: pangine.reference_percept("rust-result-input"),
            experience_template: pangine.reference_percept("rust-experience-template"),
            experience: pangine.reference_percept("lived-experience"),
            output: pangine.reference_percept("decision-output"),
            delivered_outputs: Vec::new(),
        }
    }

    fn set_frame(&self, pangine: &mut Pangine, frame: RustFrame<'_>) {
        set_named_input(pangine, &self.context_input, Some(frame.context));
        set_named_input(pangine, &self.reading_input, Some(frame.reading));
        set_named_input(pangine, &self.result_input, Some(frame.result));
    }

    fn capture_frame(&self, pangine: &mut Pangine, frame: RustFrame<'_>) -> Option<ConceptId> {
        self.set_frame(pangine, frame);
        self.capture_current_inputs(pangine)
    }

    fn capture_current_inputs(&self, pangine: &mut Pangine) -> Option<ConceptId> {
        pangine.perform_experience(&self.experience, Some(&self.experience_template))
    }

    fn decide(&mut self, pangine: &mut Pangine, context: &str, result: &str) -> Option<String> {
        set_named_input(pangine, &self.context_input, Some(context));
        set_named_input(pangine, &self.result_input, Some(result));
        must_run(pangine, DECISION_PROGRAM);
        let delivered = pangine.get_value(&self.output).map(|choice| pangine.format_concept(&choice, false));
        self.delivered_outputs.push(delivered.clone());
        delivered
    }
}

#[test]
#[ignore = "research detail: ordinary tests now cover grouped Rust input, automatic assigned-input capture, collapsed output reads, and no callback or LLM adapter"]
fn rust_input_frames_become_experience_and_output_percepts_deliver_pangines_changing_choice() {
    let mut pangine = Pangine::new();
    let mut bridge = RustPerceptBridge::new(&mut pangine);

    for _ in 0..3 {
        bridge.capture_frame(&mut pangine, RustFrame { context: "opal", reading: "cedar", result: "pearl" }).unwrap();
    }
    for _ in 0..2 {
        bridge.capture_frame(&mut pangine, RustFrame { context: "opal", reading: "violet", result: "pearl" }).unwrap();
    }
    for _ in 0..20 {
        bridge.capture_frame(&mut pangine, RustFrame { context: "basalt", reading: "violet", result: "pearl" }).unwrap();
    }

    assert_eq!(bridge.decide(&mut pangine, "opal", "pearl"), Some("[cedar]".to_owned()));
    assert_eq!(read_named_weights(&mut pangine, "decision-candidate"), weight_map(&[("[cedar]", 3)]));

    for _ in 0..2 {
        bridge.capture_frame(&mut pangine, RustFrame { context: "opal", reading: "violet", result: "pearl" }).unwrap();
    }
    assert_eq!(bridge.decide(&mut pangine, "opal", "pearl"), Some("[violet]".to_owned()));
    assert_eq!(read_named_weights(&mut pangine, "decision-candidate"), weight_map(&[("[violet]", 4)]));
    assert_eq!(
        bridge.delivered_outputs,
        vec![Some("[cedar]".to_owned()), Some("[violet]".to_owned())],
        "the Rust side receives only Pangine's output value and does not calculate or replace either decision"
    );

    assert_eq!(bridge.decide(&mut pangine, "unknown", "pearl"), None, "a missing answer remains an empty output rather than a Rust fallback");

    let experience_before_incomplete_frame = pangine.get_relevance_map(&bridge.experience);
    set_named_input(&mut pangine, &bridge.context_input, Some("opal"));
    set_named_input(&mut pangine, &bridge.reading_input, Some("amber"));
    set_named_input(&mut pangine, &bridge.result_input, None);
    assert!(bridge.capture_current_inputs(&mut pangine).is_none(), "the one ordered template cannot evaluate when one input is absent");
    assert_eq!(
        pangine.get_relevance_map(&bridge.experience),
        experience_before_incomplete_frame,
        "an incomplete Rust frame does not become a partial experience"
    );
}

#[test]
#[ignore = "research detail: experience now grounds assigned input Percepts at capture, while ordinary assignment can deliberately retain a live template"]
fn experience_capture_freezes_input_values_while_assignment_can_keep_a_live_template() {
    let mut pangine = Pangine::new();
    let bridge = RustPerceptBridge::new(&mut pangine);
    bridge.set_frame(&mut pangine, RustFrame { context: "opal", reading: "cedar", result: "pearl" });

    let live_template = pangine.get_value(&bridge.experience_template).expect("the template should retain its input Percept references");
    let assigned_live_template = pangine.reference_percept("assigned-live-template");
    assert!(pangine.set_percept_value(&assigned_live_template, Some(live_template)));
    let grounded = bridge.capture_current_inputs(&mut pangine).expect("complete Rust frame");
    assert_eq!(pangine.format_concept(&grounded, false), "{[observation]->[context]->[opal]->[reading]->[cedar]->[result]->[pearl]}");

    set_named_input(&mut pangine, &bridge.reading_input, Some("violet"));
    let grounded_history = must_ref(&mut pangine, "$['lived-experience']");
    let reinterpreted_live_template = must_ref(&mut pangine, "$['assigned-live-template']");
    assert_eq!(
        pangine.format_concept(&grounded_history, false),
        "{[observation]->[context]->[opal]->[reading]->[cedar]->[result]->[pearl]}",
        "capture-time evaluation keeps the reading that Rust supplied for that experience"
    );
    assert_eq!(
        pangine.format_concept(&reinterpreted_live_template, false),
        "{[observation]->[context]->[opal]->[reading]->[violet]->[result]->[pearl]}",
        "ordinary assignment can deliberately retain a template that follows current input"
    );
}

#[test]
#[ignore = "warning: separately sampled providers can mix application moments; production accepts one caller-supplied group but does not define provider snapshot behavior"]
fn separate_input_callbacks_can_mix_moments_while_one_complete_frame_stays_coherent() {
    let mut mixed_pangine = Pangine::new();
    let mixed_bridge = RustPerceptBridge::new(&mut mixed_pangine);
    let first = RustFrame { context: "opal", reading: "cedar", result: "pearl" };
    let second = RustFrame { context: "basalt", reading: "violet", result: "onyx" };

    set_named_input(&mut mixed_pangine, &mixed_bridge.context_input, Some(first.context));
    set_named_input(&mut mixed_pangine, &mixed_bridge.reading_input, Some(second.reading));
    set_named_input(&mut mixed_pangine, &mixed_bridge.result_input, Some(second.result));
    let mixed = mixed_bridge.capture_current_inputs(&mut mixed_pangine).expect("all three separately read inputs are present");
    assert_eq!(
        mixed_pangine.format_concept(&mixed, false),
        "{[observation]->[context]->[opal]->[reading]->[violet]->[result]->[onyx]}",
        "separate callbacks can form an observation that was never one application frame"
    );

    let mut coherent_pangine = Pangine::new();
    let coherent_bridge = RustPerceptBridge::new(&mut coherent_pangine);
    let coherent = coherent_bridge.capture_frame(&mut coherent_pangine, first).expect("complete application frame");
    assert_eq!(
        coherent_pangine.format_concept(&coherent, false),
        "{[observation]->[context]->[opal]->[reading]->[cedar]->[result]->[pearl]}",
        "one supplied frame keeps all input values from the same application moment"
    );
}

#[test]
#[ignore = "warning: an application-owned queue can deliver changed outputs after synchronous Pangine updates and feed later input without reentry; the core intentionally has no scheduling or notification policy"]
fn queued_output_delivery_can_feed_later_input_after_each_complete_cycle() {
    let mut pangine = Pangine::new();
    let bridge = RustPerceptBridge::new(&mut pangine);
    let mut living = QueuedLivingProgram::new(bridge);
    for _ in 0..3 {
        living.enqueue(RustFrame { context: "opal", reading: "cedar", result: "pearl" });
    }
    for _ in 0..2 {
        living.enqueue(RustFrame { context: "opal", reading: "violet", result: "pearl" });
    }

    let mut delivered = Vec::new();
    let mut fed_back = false;
    while living.has_input() {
        if let Some(output) = living.tick(&mut pangine) {
            delivered.push(output.clone());
            if output.as_deref() == Some("[cedar]") && !fed_back {
                fed_back = true;
                living.enqueue(RustFrame { context: "opal", reading: "violet", result: "pearl" });
                living.enqueue(RustFrame { context: "opal", reading: "violet", result: "pearl" });
            }
        }
    }

    assert_eq!(delivered, vec![Some("[cedar]".to_owned()), Some("[violet]".to_owned())]);
    assert_eq!(read_named_weights(&mut pangine, "decision-candidate"), weight_map(&[("[violet]", 4)]));
    assert_eq!(pangine.get_value(&living.bridge.output).map(|output| pangine.format_concept(&output, false)), Some("[violet]".to_owned()));
}

struct QueuedLivingProgram {
    bridge: RustPerceptBridge,
    input: VecDeque<RustFrame<'static>>,
    last_output: Option<String>,
}

impl QueuedLivingProgram {
    fn new(bridge: RustPerceptBridge) -> Self {
        Self { bridge, input: VecDeque::new(), last_output: None }
    }

    fn enqueue(&mut self, frame: RustFrame<'static>) {
        self.input.push_back(frame);
    }

    fn has_input(&self) -> bool {
        !self.input.is_empty()
    }

    fn tick(&mut self, pangine: &mut Pangine) -> Option<Option<String>> {
        let frame = self.input.pop_front()?;
        self.bridge.capture_frame(pangine, frame).expect("queued complete input frame");
        let output = self.bridge.decide(pangine, frame.context, frame.result);
        if output == self.last_output {
            return None;
        }
        self.last_output = output.clone();
        Some(output)
    }
}

fn set_named_input(pangine: &mut Pangine, percept: &ConceptId, value: Option<&str>) {
    let value = value.map(|name| must_ref(pangine, &format!("[{name}]")));
    assert!(pangine.set_percept_value(percept, value));
}

fn read_named_weights(pangine: &mut Pangine, percept: &str) -> BTreeMap<String, Relevance> {
    let percept = pangine.reference_percept(percept);
    pangine
        .get_value(&percept)
        .into_iter()
        .flat_map(|value| pangine.get_relevance_map(&value))
        .map(|(relevance, value)| (pangine.format_concept(&value, false), relevance))
        .collect()
}

fn weight_map(entries: &[(&str, i64)]) -> BTreeMap<String, Relevance> {
    entries.iter().map(|(value, relevance)| ((*value).to_owned(), Relevance::new(*relevance))).collect()
}

fn must_run(pangine: &mut Pangine, input: &str) {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to run {input:?}: {error}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}")).unwrap_or_else(|| panic!("{input:?} was null"))
}
