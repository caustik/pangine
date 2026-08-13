use pangine::{ConceptId, Pangine, PerceptUpdateError, Relevance};
use std::collections::BTreeMap;

const DECISION_PROGRAM: &str = "
    ['lived-experience'] @ [observation]->[context]->$['context-input']->[reading]->['decision-candidate']->[result]->$['result-input'];
    ['decision-output'] = ^['decision-candidate']
";

#[test]
fn percept_values_are_validated_as_one_group_before_any_input_changes() {
    let mut pangine = Pangine::new();
    let context = pangine.reference_percept("context-input");
    let reading = pangine.reference_percept("reading-input");
    let opal = must_ref(&mut pangine, "[opal]");
    let cedar = must_ref(&mut pangine, "[cedar]");
    let basalt = must_ref(&mut pangine, "[basalt]");
    let violet = must_ref(&mut pangine, "[violet]");

    pangine.set_percept_values(&[(context.clone(), Some(opal.clone())), (reading.clone(), Some(cedar.clone()))]).expect("valid input group");
    assert_eq!(pangine.get_value(&context), Some(opal.clone()));
    assert_eq!(pangine.get_value(&reading), Some(cedar.clone()));

    let ordinary = must_ref(&mut pangine, "[ordinary]");
    assert_eq!(
        pangine.set_percept_values(&[(context.clone(), Some(basalt.clone())), (ordinary, Some(violet.clone()))]),
        Err(PerceptUpdateError::InvalidPercept)
    );
    assert_eq!(pangine.get_value(&context), Some(opal.clone()));
    assert_eq!(pangine.get_value(&reading), Some(cedar.clone()));

    assert_eq!(
        pangine.set_percept_values(&[(context.clone(), Some(basalt.clone())), (context.clone(), Some(violet.clone()))]),
        Err(PerceptUpdateError::DuplicatePercept)
    );
    assert_eq!(pangine.get_value(&context), Some(opal.clone()));

    let mut other = Pangine::new();
    let foreign = must_ref(&mut other, "[foreign]");
    assert_eq!(pangine.set_percept_values(&[(context.clone(), Some(basalt)), (reading.clone(), Some(foreign))]), Err(PerceptUpdateError::ForeignConcept));
    assert_eq!(pangine.get_value(&context), Some(opal));
    assert_eq!(pangine.get_value(&reading), Some(cedar));

    pangine.set_percept_values(&[(context.clone(), None), (reading.clone(), None)]).expect("valid cleared input group");
    assert_eq!(pangine.get_value(&context), None);
    assert_eq!(pangine.get_value(&reading), None);
    assert_eq!(pangine.set_percept_values(&[]), Ok(()));
}

#[test]
fn rust_can_ground_one_complete_input_group_without_rewriting_history() {
    let mut pangine = Pangine::new();
    let context = pangine.reference_percept("context-input");
    let reading = pangine.reference_percept("reading-input");
    let result = pangine.reference_percept("result-input");
    let experience = pangine.reference_percept("lived-experience");
    let template = must_ref(&mut pangine, "[observation]->[context]->['context-input']->[reading]->['reading-input']->[result]->['result-input']");

    set_frame(&mut pangine, [&context, &reading, &result], ["opal", "cedar", "pearl"]);
    let grounded = pangine.evaluate_concept(&template).expect("complete input group");
    assert_eq!(pangine.format_concept(&grounded, false), "{[observation]->[context]->[opal]->[reading]->[cedar]->[result]->[pearl]}");
    pangine.perform_experience(&experience, Some(&grounded)).expect("valid grounded experience");

    set_frame(&mut pangine, [&context, &reading, &result], ["opal", "violet", "pearl"]);
    assert_eq!(
        pangine.format_concept(&pangine.get_value(&experience).expect("retained experience"), false),
        "{[observation]->[context]->[opal]->[reading]->[cedar]->[result]->[pearl]}",
        "later input does not alter the grounded experience"
    );

    pangine.set_percept_values(&[(result.clone(), None)]).expect("valid missing input");
    assert_eq!(pangine.evaluate_concept(&template), None, "a missing required input cannot create a partial observation");

    let mut other = Pangine::new();
    let foreign = must_ref(&mut other, "[foreign]");
    assert_eq!(pangine.evaluate_concept(&foreign), None);
}

#[test]
fn input_and_output_percepts_complete_a_living_decision_cycle() {
    let mut pangine = Pangine::new();
    let context = pangine.reference_percept("context-input");
    let reading = pangine.reference_percept("reading-input");
    let result = pangine.reference_percept("result-input");
    let experience = pangine.reference_percept("lived-experience");
    let output = pangine.reference_percept("decision-output");
    let template = must_ref(&mut pangine, "[observation]->[context]->['context-input']->[reading]->['reading-input']->[result]->['result-input']");
    let inputs = [&context, &reading, &result];

    for _ in 0..3 {
        capture_frame(&mut pangine, &template, &experience, inputs, ["opal", "cedar", "pearl"]);
    }
    for _ in 0..2 {
        capture_frame(&mut pangine, &template, &experience, inputs, ["opal", "violet", "pearl"]);
    }
    for _ in 0..20 {
        capture_frame(&mut pangine, &template, &experience, inputs, ["basalt", "violet", "pearl"]);
    }

    set_frame(&mut pangine, inputs, ["opal", "query", "pearl"]);
    must_run(&mut pangine, DECISION_PROGRAM);
    assert_eq!(formatted_value(&pangine, &output), Some("[cedar]".to_owned()));
    assert_eq!(read_named_weights(&mut pangine, "decision-candidate"), weight_map(&[("[cedar]", 3), ("[violet]", 2)]));

    for _ in 0..2 {
        capture_frame(&mut pangine, &template, &experience, inputs, ["opal", "violet", "pearl"]);
    }
    must_run(&mut pangine, DECISION_PROGRAM);
    assert_eq!(formatted_value(&pangine, &output), Some("[violet]".to_owned()));
    assert_eq!(read_named_weights(&mut pangine, "decision-candidate"), weight_map(&[("[cedar]", 3), ("[violet]", 4)]));

    set_frame(&mut pangine, inputs, ["unknown", "query", "pearl"]);
    must_run(&mut pangine, DECISION_PROGRAM);
    assert_eq!(formatted_value(&pangine, &output), None, "Rust does not replace Pangine's missing answer with a fallback");
}

fn capture_frame(pangine: &mut Pangine, template: &ConceptId, experience: &ConceptId, inputs: [&ConceptId; 3], values: [&str; 3]) {
    set_frame(pangine, inputs, values);
    pangine.perform_experience(experience, Some(template)).expect("valid grounded experience");
}

fn set_frame(pangine: &mut Pangine, inputs: [&ConceptId; 3], values: [&str; 3]) {
    let values = values.map(|value| must_ref(pangine, &format!("[{value}]")));
    pangine
        .set_percept_values(&[
            (inputs[0].clone(), Some(values[0].clone())),
            (inputs[1].clone(), Some(values[1].clone())),
            (inputs[2].clone(), Some(values[2].clone())),
        ])
        .expect("valid input group");
}

fn formatted_value(pangine: &Pangine, percept: &ConceptId) -> Option<String> {
    pangine.get_value(percept).map(|value| pangine.format_concept(&value, false))
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
    pangine.parse_script_text(input).unwrap_or_else(|error| panic!("failed to run {input:?}: {error}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}")).unwrap_or_else(|| panic!("{input:?} was null"))
}
