use pangine::{ConceptId, Pangine, Relevance};
use std::collections::BTreeSet;

#[test]
fn ordered_question_finds_a_source_backed_indirect_answer() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", &["[C]->[bridge]->[E]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]"]);

    ask(&mut pangine, "['Alice'] @ [C]->[sound]->['answer']");

    assert_candidates(&mut pangine, "answer", &["loud", "quiet"]);
}

#[test]
fn additional_shared_surroundings_do_not_become_implicit_support() {
    let mut pangine = Pangine::new();
    experience(
        &mut pangine,
        "world",
        &[
            "[C]->[time]->[morning]",
            "[C]->[place]->[garden]",
            "[E]->[time]->[morning]",
            "[E]->[place]->[garden]",
            "[F]->[time]->[morning]",
            "[F]->[place]->[street]",
            "[E]->[sound]->[birds]",
            "[F]->[sound]->[traffic]",
        ],
    );

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");

    assert_candidates(&mut pangine, "answer", &["birds", "traffic"]);
}

#[test]
fn broad_and_bidirectional_ordered_connections_remain_eligible() {
    let mut pangine = Pangine::new();
    experience(
        &mut pangine,
        "world",
        &[
            "[C]->[c-link]->[shared]",
            "[E]->[e-link]->[shared]",
            "[C]->[context]->[morning]",
            "[B]->[context]->[evening]",
            "[C]->[sound]->[quiet]",
            "[E]->[sound]->[loud]",
            "[B]->[sound]->[buzz]",
        ],
    );

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");

    assert_candidates(&mut pangine, "answer", &["buzz", "loud", "quiet"]);
}

#[test]
fn an_answer_needs_context_outside_its_own_relationship() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", &["[C]->[sound]->[quiet]", "[D]->[sound]->[hiss]"]);

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["quiet"]);

    experience(&mut pangine, "world", &["[C]->[time]->[morning]", "[D]->[time]->[morning]"]);
    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["hiss", "quiet"]);
}

#[test]
fn only_the_ordered_origin_can_follow_context() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", &["[C]->[bridge]->[E]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]"]);

    ask(&mut pangine, "['world'] @ [C]->['relation']->[loud]");
    assert_candidates(&mut pangine, "relation", &["sound"]);

    ask(&mut pangine, "['world'] @ ['who']->[sound]->[quiet]");
    assert_candidates(&mut pangine, "who", &["C"]);
}

#[test]
fn a_route_uses_only_the_selected_percepts() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", &["[C]->[context]->[morning]", "[C]->[sound]->[quiet]"]);
    experience(&mut pangine, "Bob", &["[E]->[context]->[morning]", "[E]->[sound]->[loud]"]);

    ask(&mut pangine, "['Alice'] @ [C]->[sound]->['alice-answer']");
    assert_candidates(&mut pangine, "alice-answer", &["quiet"]);

    ask(&mut pangine, "['Bob'] @ [C]->[sound]->['bob-answer']");
    assert_candidates(&mut pangine, "bob-answer", &[]);

    ask(&mut pangine, "['Alice']['Bob'] @ [C]->[sound]->['joint-answer']");
    assert_candidates(&mut pangine, "joint-answer", &["loud", "quiet"]);
}

#[test]
fn retained_weighted_and_negative_memberships_can_supply_a_route() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", &["[C]50%x2[bridge]", "[bridge]![E]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]"]);

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["loud", "quiet"]);

    let mut zero = Pangine::new();
    experience(&mut zero, "world", &["[C]0%[bridge]", "[bridge][E]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]"]);
    ask(&mut zero, "['world'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut zero, "answer", &["quiet"]);
}

#[test]
fn a_recursive_origin_routes_as_one_concrete_concept() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", &["([C]*[day])->[sound]->[quiet]", "([E]*[night])->[sound]->[noisy]", "[C]->[bridge]->[E]", "[day]->[bridge]->[night]"]);

    ask(&mut pangine, "['world'] @ ([C]*[day])->[sound]->['answer']");

    assert_candidates(&mut pangine, "answer", &["noisy", "quiet"]);
}

#[test]
fn outputs_inside_an_origin_bind_before_the_origin_routes() {
    let mut pangine = Pangine::new();
    experience(
        &mut pangine,
        "world",
        &["([C]->[kind])->[sound]->[quiet]", "([E]->[kind])->[sound]->[loud]", "([E]->[other])->[sound]->[noisy]", "[C]->[bridge]->[E]"],
    );

    ask(&mut pangine, "['world'] @ ([C]->['type'])->[sound]->['answer']");
    assert_candidates(&mut pangine, "type", &["kind"]);
    assert_candidates(&mut pangine, "answer", &["loud", "noisy", "quiet"]);

    ask(&mut pangine, "['world'] @ (['who']->[kind])->[sound]->['generalized']");
    assert_candidates(&mut pangine, "who", &["C", "E"]);
    assert_candidates(&mut pangine, "generalized", &["loud", "quiet"]);
}

#[test]
fn a_top_level_unordered_question_remains_exact() {
    let mut pangine = Pangine::new();
    experience(
        &mut pangine,
        "world",
        &["[C]->[bridge]->[E]", "[sound]->[bridge]->[music]", "[C]*[sound]*[quiet]", "[E]*[sound]*[loud]", "[C]*[music]*[rhythmic]", "[E]*[music]*[noisy]"],
    );

    ask(&mut pangine, "['world'] @ [C]*[sound]*['answer']");

    assert_candidates(&mut pangine, "answer", &["quiet"]);
}

#[test]
fn a_derived_ordered_window_is_not_its_own_context() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "world", &["[C]->[link]->[tail]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]->[tail]"]);

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["quiet"]);

    experience(&mut pangine, "world", &["[E]->[context]->[tail]"]);
    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["loud", "quiet"]);
}

#[test]
fn equal_answer_relationships_remain_separate_by_complete_source_root() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", &["[C]->[sound]->[quiet]", "({[D]->[sound]->[hiss]}*[left-context])"]);
    experience(&mut pangine, "Alice", &["({[D]->[sound]->[hiss]}*[left-context])"]);

    ask(&mut pangine, "['Alice'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["quiet"]);

    experience(&mut pangine, "Alice", &["({[D]->[sound]->[hiss]}*[right-context])"]);
    ask(&mut pangine, "['Alice'] @ [C]->[sound]->['answer']");
    assert_candidates(&mut pangine, "answer", &["hiss", "quiet"]);
}

#[test]
fn equal_answer_relationships_under_different_percepts_are_distinct_sources() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", &["[C]->[sound]->[quiet]"]);
    experience(&mut pangine, "Bob", &["[D]->[sound]->[hiss]"]);
    experience(&mut pangine, "Eve", &["[D]->[sound]->[hiss]"]);

    ask(&mut pangine, "['Alice']['Bob'] @ [C]->[sound]->['two-sources']");
    assert_candidates(&mut pangine, "two-sources", &["quiet"]);

    ask(&mut pangine, "['Alice']['Bob']['Eve'] @ [C]->[sound]->['three-sources']");
    assert_candidates(&mut pangine, "three-sources", &["hiss", "quiet"]);
}

#[test]
fn a_question_result_becomes_context_only_when_selected_later() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", &["[A]*[B]*[C]"]);
    ask(&mut pangine, "['memory'] @ ['remainder']*[B]");
    assert_eq!(must_ref(&mut pangine, "^['remainder']"), must_ref(&mut pangine, "[A]*[C]"));

    experience(&mut pangine, "world", &["[A]->[bridge]->[E]", "[E]->[sound]->[loud]"]);
    ask(&mut pangine, "['world'] @ ([A]*[C])->[sound]->['without-output']");
    assert_candidates(&mut pangine, "without-output", &[]);

    ask(&mut pangine, "['world']['remainder'] @ ([A]*[C])->[sound]->['with-output']");
    assert_candidates(&mut pangine, "with-output", &["loud"]);
}

#[test]
fn contextual_reachability_terminates_on_cycles() {
    let mut pangine = Pangine::new();
    experience(
        &mut pangine,
        "world",
        &[
            "[C]->[c-a]->[A]",
            "[A]->[a-b]->[B]",
            "[B]->[b-c]->[C]",
            "[B]->[b-e]->[E]",
            "[C]->[sound]->[quiet]",
            "[E]->[sound]->[distant]",
            "[Z]->[sound]->[decoy]",
        ],
    );

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");

    assert_candidates(&mut pangine, "answer", &["distant", "quiet"]);
}

#[test]
fn alternate_routes_do_not_multiply_support() {
    let mut pangine = Pangine::new();
    experience(
        &mut pangine,
        "world",
        &["[C]->[first]->[A]", "[A]->[second]->[E]", "[C]->[third]->[B]", "[B]->[fourth]->[E]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]"],
    );

    ask(&mut pangine, "['world'] @ [C]->[sound]->['answer']");

    let answer = must_ref(&mut pangine, "$['answer']");
    let loud = must_ref(&mut pangine, "[loud]");
    let relevance = pangine
        .get_relevance_map(&answer)
        .into_iter()
        .find_map(|(relevance, candidate)| (candidate == loud).then_some(relevance))
        .expect("expected the indirect loud candidate");
    assert_eq!(relevance, Relevance::DEFAULT);
}

fn experience(pangine: &mut Pangine, percept: &str, roots: &[&str]) {
    for root in roots {
        must_ref(pangine, &format!("['{percept}'] ~= {root}"));
    }
}

fn ask(pangine: &mut Pangine, question: &str) {
    must_ref(pangine, question);
}

fn assert_candidates(pangine: &mut Pangine, percept: &str, expected: &[&str]) {
    let expected = expected.iter().map(|name| (*name).to_owned()).collect::<BTreeSet<_>>();
    assert_eq!(named_candidates(pangine, percept), expected);
}

fn named_candidates(pangine: &mut Pangine, percept: &str) -> BTreeSet<String> {
    let Some(value) = pangine.reference_concept(&format!("$['{percept}']")).unwrap_or_else(|error| panic!("failed to inspect {percept:?}: {error}")) else {
        return BTreeSet::new();
    };

    pangine
        .get_relevance_map(&value)
        .into_iter()
        .map(|(_, candidate)| pangine.get_name(&candidate).unwrap_or_else(|| panic!("expected a named candidate, got {candidate:?}")).to_owned())
        .collect()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
