//! Keeps the smallest distinctive result from the retired decision-ledger comparison.
//!
//! Flat decision-history queries belong in SQL. This warning protects only the
//! behavior still relevant to Pangine's purpose search: an open Answer can be
//! extended by evidence that arrives later while retaining the exact sources of
//! the original and newly joined facts.

use pangine::{Completion, ConceptId, Pangine};
use std::collections::BTreeSet;

const HIGH_CONFIDENCE_ERROR: &str = "
    ({request}->[kind]->[request])
    ({request}->[language]->{language})
    ({request}->[schema]->{schema})
    ({judgment}->[kind]->[judgment])
    ({judgment}->[request]->{request})
    ({judgment}->[model]->{model})
    ({judgment}->[choice]->{choice})
    ({judgment}->[confidence]->[high])
    ({outcome}->[kind]->[outcome])
    ({outcome}->[judgment]->{judgment})
    ({outcome}->[result]->[wrong])
    ({outcome}->[expected]->{expected})";

const MODEL_EXTENSION: &str = "
    ({model}->[kind]->[model])
    ({model}->[calibration]->{calibration})";

const SCHEMA_EXTENSION: &str = "
    ({schema}->[kind]->[schema])
    ({schema}->[option-band]->{option-band})";

#[test]
#[ignore = "warning: live Answer extension is promising, but flat event-history investigation is a SQL workload"]
fn later_evidence_extends_an_open_answer_and_keeps_exact_sources() {
    let mut pangine = Pangine::new();
    let mut sources = Vec::new();

    remember(
        &mut pangine,
        &mut sources,
        "source-request-ro",
        "([request-ro]->[kind]->[request])
         ([request-ro]->[language]->[ro])
         ([request-ro]->[schema]->[skills-13])",
    );
    remember(
        &mut pangine,
        &mut sources,
        "source-judgment-ro-v1",
        "([judgment-ro-v1]->[kind]->[judgment])
         ([judgment-ro-v1]->[request]->[request-ro])
         ([judgment-ro-v1]->[model]->[laya-router-v1])
         ([judgment-ro-v1]->[choice]->[godot])
         ([judgment-ro-v1]->[confidence]->[high])",
    );
    remember(
        &mut pangine,
        &mut sources,
        "source-outcome-ro-v1",
        "([outcome-ro-v1]->[kind]->[outcome])
         ([outcome-ro-v1]->[judgment]->[judgment-ro-v1])
         ([outcome-ro-v1]->[result]->[wrong])
         ([outcome-ro-v1]->[expected]->[video])",
    );

    ask(&mut pangine, &sources, HIGH_CONFIDENCE_ERROR);
    let original = sole_completion(&mut pangine, "request");
    assert_eq!(source_names(&pangine, &original), names(&["source-request-ro", "source-judgment-ro-v1", "source-outcome-ro-v1"]));

    remember(
        &mut pangine,
        &mut sources,
        "source-model-router-v1",
        "([laya-router-v1]->[kind]->[model])
         ([laya-router-v1]->[calibration]->[unfitted])",
    );
    remember(
        &mut pangine,
        &mut sources,
        "source-schema-skills",
        "([skills-13]->[kind]->[schema])
         ([skills-13]->[option-band]->[many])",
    );

    ask(&mut pangine, &sources, MODEL_EXTENSION);
    ask(&mut pangine, &sources, SCHEMA_EXTENSION);
    let enriched = sole_completion(&mut pangine, "option-band");

    for (field, expected) in [
        ("request", "request-ro"),
        ("language", "ro"),
        ("judgment", "judgment-ro-v1"),
        ("model", "laya-router-v1"),
        ("schema", "skills-13"),
        ("choice", "godot"),
        ("expected", "video"),
        ("calibration", "unfitted"),
        ("option-band", "many"),
    ] {
        assert_eq!(binding_name(&mut pangine, &enriched, field), expected);
    }
    assert_eq!(
        source_names(&pangine, &enriched),
        names(&["source-request-ro", "source-judgment-ro-v1", "source-outcome-ro-v1", "source-model-router-v1", "source-schema-skills",])
    );
}

fn remember(pangine: &mut Pangine, sources: &mut Vec<ConceptId>, source_name: &str, text: &str) {
    let source = pangine.reference_percept(source_name);
    let value = concept(pangine, text);
    pangine.perform_experience(&source, Some(&value)).expect("remember source");
    sources.push(source);
}

fn ask(pangine: &mut Pangine, sources: &[ConceptId], question: &str) {
    let selector = sources.iter().map(|source| pangine.format_concept(source, false)).collect::<String>();
    concept(pangine, &format!("{selector} @ {question}"));
}

fn sole_completion(pangine: &mut Pangine, anchor: &str) -> Completion {
    let anchor = pangine.reference_percept(anchor);
    let answer = pangine.answer_snapshot(&anchor).expect("open Answer");
    assert_eq!(answer.result().completions().len(), 1);
    answer.result().completions()[0].clone()
}

fn binding_name(pangine: &mut Pangine, completion: &Completion, field: &str) -> String {
    let field = pangine.reference_percept(field);
    let value = completion.binding(&field).expect("bound output");
    pangine.get_name(value).expect("named output").to_owned()
}

fn source_names(pangine: &Pangine, completion: &Completion) -> BTreeSet<String> {
    completion.evidence().iter().map(|evidence| percept_name(pangine, evidence.source_percept().expect("source Percept"))).collect()
}

fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn percept_name(pangine: &Pangine, concept: &ConceptId) -> String {
    let formatted = pangine.format_concept(concept, false);
    formatted.strip_prefix('{').and_then(|name| name.strip_suffix('}')).expect("Percept name").to_owned()
}

fn concept(pangine: &mut Pangine, text: &str) -> ConceptId {
    pangine.reference_concept(text).unwrap_or_else(|error| panic!("{text}: {error}")).expect("nonempty Concept")
}
