//! Warning checks for the current projection of Percept-member relevance into `x`.
//!
//! Integer member relevance is useful evidence, but placing its sum in the
//! answer coefficient is not an accepted definition of relevance.

use pangine::{ConceptId, Pangine, Relevance};

#[test]
#[ignore = "warning: projecting Percept-member relevance into answer x is provisional"]
fn repeated_experiences_produce_exact_additive_support_without_event_ids() {
    let mut pangine = Pangine::new();
    let birds = "[morning][birds]";
    experience(&mut pangine, "world", birds);
    experience(&mut pangine, "world", birds);
    experience(&mut pangine, "world", "[morning][traffic]");

    ask(&mut pangine, "['world'] @ [morning]['answer']");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::new(2)));
    assert_eq!(candidate_relevance(&mut pangine, "answer", "traffic"), Some(Relevance::DEFAULT));
    assert_eq!(must_ref(&mut pangine, "^['answer']"), must_ref(&mut pangine, "[birds]"));
}

#[test]
#[ignore = "warning: recursive-view deduplication is provisional evidence policy"]
fn one_experience_contributes_once_through_recursive_views() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", "{([morning][birds])->[archived]}");

    ask(&mut pangine, "['world'] @ [morning]['answer']");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::DEFAULT));
}

#[test]
#[ignore = "warning: binding deduplication is provisional evidence policy"]
fn one_experience_contributes_once_across_distinct_complete_bindings() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", "({[sample-a]->[sound]->[birds]})({[sample-b]->[sound]->[birds]})");

    ask(&mut pangine, "['world'] @ {['sample']->[sound]->['answer']}");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::DEFAULT));
}

#[test]
#[ignore = "warning: direct Percept members as separate support are provisional"]
fn separate_direct_subconcepts_are_separate_experiences_without_event_ids() {
    let mut pangine = Pangine::new();
    let event = "[morning][birds]";
    experience(&mut pangine, "world", event);
    experience(&mut pangine, "world", &format!("{{({event})->[archived]}}"));

    ask(&mut pangine, "['world'] @ [morning]['answer']");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::new(2)));
}

#[test]
#[ignore = "warning: cross-Percept support addition is provisional"]
fn selected_percepts_and_member_relevance_both_supply_support() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", "{[C]->[sound]->[birds]}");
    experience(&mut pangine, "Alice", "{[C]->[sound]->[birds]}");
    experience(&mut pangine, "Bob", "{[C]->[sound]->[birds]}");

    ask(&mut pangine, "['Alice']['Bob'] @ {[C]->[sound]->['answer']}");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::new(3)));
}

#[test]
#[ignore = "warning: equal Concepts under different Percepts as additive support is provisional"]
fn repeated_concept_under_different_percepts_increases_current_support() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", "{[cat]->[purr]}");
    experience(&mut pangine, "Bob", "{[cat]->[purr]}");

    ask(&mut pangine, "['Alice'] @ {[cat]->['one']}");
    let one = candidate_relevance(&mut pangine, "one", "purr").unwrap();

    ask(&mut pangine, "['Alice']['Bob'] @ {[cat]->['two']}");
    let two = candidate_relevance(&mut pangine, "two", "purr").unwrap();
    assert!(two.x_coefficient > one.x_coefficient);
}

#[test]
#[ignore = "warning: shared collapse treats one composite output binding as one complete candidate"]
fn current_decision_keeps_one_composite_output_binding_whole() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "[A][B][C]");

    ask(&mut pangine, "['memory'] @ ['remainder'][B]");

    let remainder = must_ref(&mut pangine, "[A][C]");
    let remainder_percept = pangine.reference_percept("remainder");
    assert_eq!(must_ref(&mut pangine, "$['remainder']"), remainder);
    assert_eq!(pangine.get_relevance_map(&remainder_percept), vec![(Relevance::DEFAULT, remainder.clone())]);
    assert_eq!(must_ref(&mut pangine, "^['remainder']"), remainder);
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str) {
    must_ref(pangine, &format!("['{percept}'] ~= {concept}"));
}

fn ask(pangine: &mut Pangine, question: &str) {
    must_ref(pangine, question);
}

fn candidate_relevance(pangine: &mut Pangine, percept: &str, candidate: &str) -> Option<Relevance> {
    let value = must_ref(pangine, &format!("$['{percept}']"));
    let candidate = must_ref(pangine, &format!("[{candidate}]"));
    pangine.get_relevance_map(&value).into_iter().find_map(|(relevance, concept)| (concept == candidate).then_some(relevance))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
