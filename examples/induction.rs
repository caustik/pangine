use pangine::{ConceptId, Pangine};

const COMPLETE_EXPERIENCE: &str = "{[C]->[A]}*{[B]->[D]}";
const PARTIAL_EXPERIENCES: [&str; 3] = ["{[E]->[A]}*{[P1]->[Q1]}", "{[E]->[A]}*{[P2]->[Q2]}", "{[E]->[A]}*{[P3]->[Q3]}"];
const QUESTION: &str = "{['X']->[A]}*{[B]->[D]}";

fn main() {
    let mut pangine = Pangine::new();

    experience(&mut pangine, COMPLETE_EXPERIENCE);
    for partial in PARTIAL_EXPERIENCES {
        experience(&mut pangine, partial);
    }

    let unseen_complete = must_reference(&mut pangine, "{[E]->[A]}*{[B]->[D]}");
    let memory = must_reference(&mut pangine, "$['memory']");
    assert!(!pangine
        .get_observations(&memory)
        .expect("memory should contain Observation state")
        .iter()
        .any(|record| pangine.get_observation(record).as_ref() == Some(&unseen_complete)));

    must_reference(&mut pangine, &format!("['memory'] @ {QUESTION}"));
    let ranked = must_reference(&mut pangine, "$['X']");
    let selected = must_reference(&mut pangine, "^['X']");
    assert_eq!(selected, must_reference(&mut pangine, "[E]"));

    println!("hypothesis: several partial experiences can outweigh one complete observation");
    println!("complete experience: {COMPLETE_EXPERIENCE}");
    for partial in PARTIAL_EXPERIENCES {
        println!("partial experience:  {partial}");
    }
    println!("question:            {QUESTION}");
    println!("ranked candidates:   {}", pangine.format_concept(&ranked, false));
    println!("selected candidate:  {}", pangine.format_concept(&selected, false));
    println!("result: E wins without the complete E-shaped observation ever being experienced");
    println!("limitation: these strengths are deterministic projection scores, not calibrated probabilities");
}

fn experience(pangine: &mut Pangine, concept: &str) {
    must_reference(pangine, &format!("['memory'] ~= {concept}"));
}

fn must_reference(pangine: &mut Pangine, expression: &str) -> ConceptId {
    pangine
        .reference_concept(expression)
        .unwrap_or_else(|error| panic!("failed to parse {expression:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {expression:?}"))
}
