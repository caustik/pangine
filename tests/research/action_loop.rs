//! Focused warning checks for a small Pangine-native action loop.
//!
//! The program forms and chooses routes with Pangine operations. The simulated
//! application supplies only the observed destination and result after each
//! action. This demonstrates the loop mechanics without defining universal
//! planning, action, or outcome semantics.

use pangine::{ConceptId, Pangine};

const TWO_STEP_PLAN: &str = "['moves'] @ ($['current']->['first-action']->['middle'])(['middle']->['second-action']->$['goal'])";

#[test]
#[ignore = "research detail: one represented action loop does not settle planning or outcome semantics"]
fn complete_plan_choice_can_continue_from_an_observed_outcome() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "[A]->[north]->[B]", 2);
    remember(&mut pangine, "[B]->[east]->[D]", 1);
    remember(&mut pangine, "[A]->[east]->[C]", 1);
    remember(&mut pangine, "[C]->[north]->[D]", 1);

    must_ref(&mut pangine, "['current'] = [A]");
    must_ref(&mut pangine, "['goal'] = [D]");
    must_ref(&mut pangine, TWO_STEP_PLAN);

    assert_eq!(must_ref(&mut pangine, "&['first-action']"), must_ref(&mut pangine, "([A]->['first-action']->['middle'])(['middle']->['second-action']->[D])"));
    assert_eq!(
        must_ref(&mut pangine, "$(&['first-action'])"),
        must_ref(&mut pangine, "x2({[A]->[east]->[C]}{[C]->[north]->[D]})x3({[A]->[north]->[B]}{[B]->[east]->[D]})")
    );
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[B]}{[B]->[east]->[D]}"));

    must_ref(&mut pangine, "['planned-from'] = $['current']");
    must_ref(&mut pangine, "['planned-first-action'] = ^['first-action']");
    must_ref(&mut pangine, "['planned-middle'] = ^['middle']");
    must_ref(&mut pangine, "['attempt-id'] = [attempt-1]");
    must_ref(&mut pangine, "['actual-middle'] = [C]");
    must_ref(&mut pangine, "['actual-result'] = [diverged]");
    must_ref(
        &mut pangine,
        "['attempts'] ~= (['attempt-id']->[from]->['planned-from'])
                          (['attempt-id']->[action]->['planned-first-action'])
                          (['attempt-id']->[expected]->['planned-middle'])
                          (['attempt-id']->[reached]->['actual-middle'])
                          (['attempt-id']->[result]->['actual-result'])",
    );

    must_ref(&mut pangine, "['current'] = $['actual-middle']");
    assert_eq!(must_ref(&mut pangine, "['moves'] @ $['current']->['next-action']->$['goal']"), must_ref(&mut pangine, "{[C]->[north]->[D]}"));
    assert_eq!(must_ref(&mut pangine, "^['next-action']"), must_ref(&mut pangine, "[north]"));

    must_ref(&mut pangine, "['planned-from'] = $['current']");
    must_ref(&mut pangine, "['planned-first-action'] = ^['next-action']");
    must_ref(&mut pangine, "['planned-middle'] = $['goal']");
    must_ref(&mut pangine, "['attempt-id'] = [attempt-2]");
    must_ref(&mut pangine, "['actual-middle'] = [D]");
    must_ref(&mut pangine, "['actual-result'] = [success]");
    must_ref(
        &mut pangine,
        "['attempts'] ~= (['attempt-id']->[from]->['planned-from'])
                          (['attempt-id']->[action]->['planned-first-action'])
                          (['attempt-id']->[expected]->['planned-middle'])
                          (['attempt-id']->[reached]->['actual-middle'])
                          (['attempt-id']->[result]->['actual-result'])",
    );
    must_ref(&mut pangine, "['current'] = $['actual-middle']");

    must_ref(&mut pangine, "['attempts'] @ [attempt-1]->[reached]->['first-reached']");
    must_ref(&mut pangine, "['attempts'] @ [attempt-2]->[reached]->['second-reached']");
    must_ref(&mut pangine, "['attempts'] @ [attempt-1]->[result]->['first-result']");
    must_ref(&mut pangine, "['attempts'] @ [attempt-2]->[result]->['second-result']");
    assert_eq!(must_ref(&mut pangine, "$['first-reached']"), must_ref(&mut pangine, "[C]"));
    assert_eq!(must_ref(&mut pangine, "$['second-reached']"), must_ref(&mut pangine, "[D]"));
    assert_eq!(must_ref(&mut pangine, "$['first-result']"), must_ref(&mut pangine, "[diverged]"));
    assert_eq!(must_ref(&mut pangine, "$['second-result']"), must_ref(&mut pangine, "[success]"));
    assert_eq!(must_ref(&mut pangine, "$['current']"), must_ref(&mut pangine, "[D]"));
}

#[test]
#[ignore = "warning: complete-plan choice is Pangine-native, but additive step support remains provisional"]
fn additive_plan_support_can_prefer_one_weak_link() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "[A]->[north]->[B]", 9);
    remember(&mut pangine, "[B]->[east]->[D]", 1);
    remember(&mut pangine, "[A]->[east]->[C]", 5);
    remember(&mut pangine, "[C]->[north]->[D]", 4);

    must_ref(&mut pangine, "['current'] = [A]");
    must_ref(&mut pangine, "['goal'] = [D]");
    must_ref(&mut pangine, TWO_STEP_PLAN);

    assert_eq!(
        must_ref(&mut pangine, "$(&['first-action'])"),
        must_ref(&mut pangine, "x9({[A]->[east]->[C]}{[C]->[north]->[D]})x10({[A]->[north]->[B]}{[B]->[east]->[D]})")
    );
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[B]}{[B]->[east]->[D]}"));

    let uneven_support = [9_i64, 1];
    let balanced_support = [5_i64, 4];
    assert!(uneven_support.iter().sum::<i64>() > balanced_support.iter().sum::<i64>());
    assert!(uneven_support.iter().product::<i64>() < balanced_support.iter().product::<i64>());
    assert!(uneven_support.iter().min() < balanced_support.iter().min());
}

#[test]
#[ignore = "research detail: complete-route experience changes the represented evidence without defining a universal step rule"]
fn complete_route_experience_can_change_choice_without_a_new_relevance_rule() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "[A]->[north]->[B]", 9);
    remember(&mut pangine, "[B]->[east]->[D]", 1);
    remember(&mut pangine, "[A]->[east]->[C]", 5);
    remember(&mut pangine, "[C]->[north]->[D]", 4);
    remember_in(&mut pangine, "routes", "([A]->[north]->[B])([B]->[east]->[D])", 1);
    remember_in(&mut pangine, "routes", "([A]->[east]->[C])([C]->[north]->[D])", 4);

    must_ref(&mut pangine, "['current'] = [A]");
    must_ref(&mut pangine, "['goal'] = [D]");
    must_ref(&mut pangine, TWO_STEP_PLAN);
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[B]}{[B]->[east]->[D]}"));

    must_ref(&mut pangine, "['routes'] @ ($['current']->['first-action']->['middle'])(['middle']->['second-action']->$['goal'])");
    assert_eq!(
        must_ref(&mut pangine, "$(&['first-action'])"),
        must_ref(&mut pangine, "({[A]->[north]->[B]}{[B]->[east]->[D]})x4({[A]->[east]->[C]}{[C]->[north]->[D]})")
    );
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[east]->[C]}{[C]->[north]->[D]}"));

    must_ref(&mut pangine, "['moves']['routes'] @ ($['current']->['first-action']->['middle'])(['middle']->['second-action']->$['goal'])");
    assert_eq!(
        must_ref(&mut pangine, "$(&['first-action'])"),
        must_ref(&mut pangine, "x11({[A]->[north]->[B]}{[B]->[east]->[D]})x13({[A]->[east]->[C]}{[C]->[north]->[D]})")
    );
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[east]->[C]}{[C]->[north]->[D]}"));
}

fn remember(pangine: &mut Pangine, move_concept: &str, repetitions: usize) {
    remember_in(pangine, "moves", move_concept, repetitions);
}

fn remember_in(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("['{percept}'] ~= {concept}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
