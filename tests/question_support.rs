use pangine::{ConceptId, Pangine, Relevance};

#[test]
fn repeated_experiences_produce_exact_support_counts_without_event_ids() {
    let mut pangine = Pangine::new();
    let birds = "[morning][birds]";
    experience(&mut pangine, "world", birds);
    experience(&mut pangine, "world", birds);
    experience(&mut pangine, "world", "[morning][traffic]");

    ask(&mut pangine, "['world'] @ [morning]['answer']");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::new(2.0)));
    assert_eq!(candidate_relevance(&mut pangine, "answer", "traffic"), Some(Relevance::DEFAULT));
    assert_eq!(must_ref(&mut pangine, "^['answer']"), must_ref(&mut pangine, "[birds]"));
}

#[test]
fn one_experience_contributes_once_through_recursive_views() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", "{([morning][birds])->[archived]}");

    ask(&mut pangine, "['world'] @ [morning]['answer']");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::DEFAULT));
}

#[test]
fn one_experience_contributes_once_across_distinct_complete_bindings() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", "({[sample-a]->[sound]->[birds]})({[sample-b]->[sound]->[birds]})");

    ask(&mut pangine, "['world'] @ {['sample']->[sound]->['answer']}");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::DEFAULT));
}

#[test]
fn separate_exact_roots_are_separate_experiences_without_event_ids() {
    let mut pangine = Pangine::new();
    let event = "[morning][birds]";
    experience(&mut pangine, "world", event);
    experience(&mut pangine, "world", &format!("{{({event})->[archived]}}"));

    ask(&mut pangine, "['world'] @ [morning]['answer']");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::new(2.0)));
}

#[test]
fn selected_percepts_and_root_occurrences_both_supply_support() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", "{[C]->[sound]->[birds]}");
    experience(&mut pangine, "Alice", "{[C]->[sound]->[birds]}");
    experience(&mut pangine, "Bob", "{[C]->[sound]->[birds]}");

    ask(&mut pangine, "['Alice']['Bob'] @ {[C]->[sound]->['answer']}");

    assert_eq!(candidate_relevance(&mut pangine, "answer", "birds"), Some(Relevance::new(3.0)));
}

#[test]
fn one_composite_answer_remains_one_decision_candidate_without_fake_support() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "[A][B][C]");

    ask(&mut pangine, "['memory'] @ ['remainder'][B]");

    assert_eq!(must_ref(&mut pangine, "^['remainder']"), must_ref(&mut pangine, "[A][C]"));
}

fn experience(pangine: &mut Pangine, percept: &str, root: &str) {
    must_ref(pangine, &format!("['{percept}'] ~= {root}"));
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
