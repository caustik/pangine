//! Focused warning checks for experience changing later route choices.
//!
//! These programs compare two explicit representations. Observed transitions
//! can extend route knowledge, while complete episodes can keep a result tied
//! to the route that produced it. Neither representation defines universal
//! learning or outcome semantics.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::BTreeMap;

const TWO_STEP_ROUTE: &str = "($['current']->['first-action']->['middle'])(['middle']->['second-action']->$['goal'])";
const SUCCESSFUL_ROUTE_EXTENSION: &str = "['episodes']['wanted-outcome'] @
    (['episode']->[from]->$['current'])
    (['episode']->[first-action]->['first-action'])
    (['episode']->[middle]->['middle'])
    (['episode']->[second-action]->['second-action'])
    (['episode']->[to]->$['goal'])
    (['episode']->[outcome]->['outcome'])
    ([wanted]->['outcome'])";

#[test]
#[ignore = "warning: observed transition repetition and additive route support remain provisional"]
fn observed_transitions_can_change_a_later_complete_route() {
    let mut pangine = Pangine::new();
    remember_move(&mut pangine, "moves", "[A]->[north]->[B]", 2);
    remember_move(&mut pangine, "moves", "[B]->[east]->[D]", 1);
    remember_move(&mut pangine, "moves", "[A]->[east]->[C]", 1);
    remember_move(&mut pangine, "moves", "[C]->[north]->[D]", 1);
    set_route_inputs(&mut pangine);

    must_ref(&mut pangine, &format!("['moves'] @ {TWO_STEP_ROUTE}"));
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[B]}{[B]->[east]->[D]}"));

    for _ in 0..2 {
        observe_move(&mut pangine, "A", "north", "C");
        observe_move(&mut pangine, "C", "north", "D");
    }

    must_ref(&mut pangine, &format!("['moves']['observed-moves'] @ {TWO_STEP_ROUTE}"));
    assert_eq!(
        must_ref(&mut pangine, "$(&['first-action'])"),
        must_ref(&mut pangine, "x5({[A]->[north]->[C]}{[C]->[north]->[D]})x4({[A]->[east]->[C]}{[C]->[north]->[D]})x3({[A]->[north]->[B]}{[B]->[east]->[D]})")
    );
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[C]}{[C]->[north]->[D]}"));
}

#[test]
#[ignore = "warning: this complete adaptive loop still uses provisional additive support and represented success"]
fn a_route_answer_can_be_extended_by_an_observed_success() {
    let mut pangine = Pangine::new();
    remember_move(&mut pangine, "moves", "[A]->[north]->[B]", 2);
    remember_move(&mut pangine, "moves", "[B]->[east]->[D]", 1);
    remember_move(&mut pangine, "moves", "[A]->[east]->[C]", 1);
    remember_move(&mut pangine, "moves", "[C]->[north]->[D]", 1);
    set_route_inputs(&mut pangine);

    must_ref(&mut pangine, &format!("['moves'] @ {TWO_STEP_ROUTE}"));
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[B]}{[B]->[east]->[D]}"));

    observe_move(&mut pangine, "A", "north", "C");
    must_ref(&mut pangine, "['current'] = [C]");
    must_ref(&mut pangine, "['moves']['observed-moves'] @ $['current']->['next-action']->$['goal']");
    assert_eq!(must_ref(&mut pangine, "^['next-action']"), must_ref(&mut pangine, "[north]"));
    observe_move(&mut pangine, "C", "north", "D");
    remember_episode(&mut pangine, "episode-1", "north", "C", "north", "success");

    must_ref(&mut pangine, "['current'] = [A]");
    must_ref(&mut pangine, &format!("['moves']['observed-moves'] @ {TWO_STEP_ROUTE}"));
    assert_eq!(
        must_ref(&mut pangine, "$(['first-action']->['middle']->['second-action'])"),
        must_ref(&mut pangine, "x3{[east]->[C]->[north]}x3{[north]->[B]->[east]}x3{[north]->[C]->[north]}")
    );

    must_ref(&mut pangine, "['wanted-outcome'] = [wanted]->[success]");
    must_ref(&mut pangine, SUCCESSFUL_ROUTE_EXTENSION);
    assert_eq!(must_ref(&mut pangine, "$(['first-action']->['middle']->['second-action'])"), must_ref(&mut pangine, "x4{[north]->[C]->[north]}"));
    assert_eq!(must_ref(&mut pangine, "^(['first-action']->['middle']->['second-action'])"), must_ref(&mut pangine, "{[north]->[C]->[north]}"));
    assert_eq!(must_ref(&mut pangine, "$['episode']"), must_ref(&mut pangine, "[episode-1]"));
    assert_eq!(must_ref(&mut pangine, "$['outcome']"), must_ref(&mut pangine, "x2[success]"));
}

#[test]
#[ignore = "warning: represented success routing is one program, not universal outcome semantics"]
fn shared_episode_identity_lets_a_desired_outcome_change_route_choice() {
    let mut pangine = Pangine::new();
    remember_episode(&mut pangine, "north-success", "north", "B", "east", "success");
    for number in 1..=5 {
        remember_episode(&mut pangine, &format!("north-diverged-{number}"), "north", "B", "east", "diverged");
    }
    for number in 1..=3 {
        remember_episode(&mut pangine, &format!("east-success-{number}"), "east", "C", "north", "success");
    }
    set_route_inputs(&mut pangine);

    must_ref(
        &mut pangine,
        "['episodes'] @
           (['all-episode']->[from]->$['current'])
           (['all-episode']->[first-action]->['all-first-action'])
           (['all-episode']->[middle]->['all-middle'])
           (['all-episode']->[second-action]->['all-second-action'])
           (['all-episode']->[to]->$['goal'])",
    );
    assert_eq!(
        must_ref(&mut pangine, "$(['all-first-action']->['all-middle']->['all-second-action'])"),
        must_ref(&mut pangine, "x6{[north]->[B]->[east]}x3{[east]->[C]->[north]}")
    );
    assert_eq!(must_ref(&mut pangine, "^(['all-first-action']->['all-middle']->['all-second-action'])"), must_ref(&mut pangine, "{[north]->[B]->[east]}"));

    must_ref(&mut pangine, "['wanted-outcome'] = [wanted]->[success]");
    must_ref(&mut pangine, SUCCESSFUL_ROUTE_EXTENSION);
    assert_eq!(
        must_ref(&mut pangine, "$(['first-action']->['middle']->['second-action'])"),
        must_ref(&mut pangine, "x3{[east]->[C]->[north]}{[north]->[B]->[east]}")
    );
    assert_eq!(must_ref(&mut pangine, "^(['first-action']->['middle']->['second-action'])"), must_ref(&mut pangine, "{[east]->[C]->[north]}"));
}

#[test]
#[ignore = "warning: explicit positive-minus-negative route totals preserve candidates but discard linked source evidence"]
fn represented_outcome_adjustments_can_keep_untried_routes_visible() {
    let mut pangine = Pangine::new();
    remember_weighted_routes(&mut pangine);
    remember_episode(&mut pangine, "north-success", "north", "B", "east", "success");
    for number in 1..=5 {
        remember_episode(&mut pangine, &format!("north-diverged-{number}"), "north", "B", "east", "diverged");
    }
    for number in 1..=3 {
        remember_episode(&mut pangine, &format!("east-success-{number}"), "east", "C", "north", "success");
    }
    set_route_inputs(&mut pangine);

    must_ref(
        &mut pangine,
        "['moves'] @
           ($['current']->['base-first-action']->['base-middle'])
           (['base-middle']->['base-second-action']->$['goal'])",
    );
    must_ref(&mut pangine, "['route-choice'] = $(['base-first-action']->['base-middle']->['base-second-action'])");
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x4{[west]->[E]->[south]}x3{[north]->[B]->[east]}x2{[east]->[C]->[north]}"));

    must_ref(&mut pangine, "['positive-result'] = [positive]->[success]");
    ask_episode_routes(&mut pangine, "positive");
    must_ref(&mut pangine, "['route-choice'] *= $(['positive-first-action']->['positive-middle']->['positive-second-action'])");
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x5{[east]->[C]->[north]}x4{[north]->[B]->[east]}x4{[west]->[E]->[south]}"));

    must_ref(&mut pangine, "['negative-result'] = [negative]->[diverged]");
    ask_episode_routes(&mut pangine, "negative");
    must_ref(&mut pangine, "['route-choice'] /= $(['negative-first-action']->['negative-middle']->['negative-second-action'])");
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x5{[east]->[C]->[north]}x4{[west]->[E]->[south]}!{[north]->[B]->[east]}"));

    must_ref(&mut pangine, "['selected-route'] = ^['route-choice']");
    must_ref(&mut pangine, "['selected-route'] @ ['selected-first-action']->['selected-middle']->['selected-second-action']");
    assert_eq!(must_ref(&mut pangine, "$['selected-first-action']"), must_ref(&mut pangine, "[east]"));
    assert_eq!(must_ref(&mut pangine, "$['selected-middle']"), must_ref(&mut pangine, "[C]"));
    assert_eq!(must_ref(&mut pangine, "$['selected-second-action']"), must_ref(&mut pangine, "[north]"));

    remember_episode(&mut pangine, "east-success-4", "east", "C", "north", "success");
    rebuild_route_choice(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x6{[east]->[C]->[north]}x4{[west]->[E]->[south]}!{[north]->[B]->[east]}"));

    must_ref(&mut pangine, "['episodes'] @ [east-success-4]->[outcome]->['recorded-outcome']");
    assert_eq!(must_ref(&mut pangine, "$['recorded-outcome']"), must_ref(&mut pangine, "[success]"));
}

#[test]
#[ignore = "warning: updating route totals preserves inner coefficients but collapses their source evidence"]
fn adjusted_route_values_do_not_preserve_inner_weights_as_question_sources() {
    let mut pangine = Pangine::new();
    remember_weighted_routes(&mut pangine);
    must_ref(&mut pangine, "['outcome-routes'] ~= ([A]->[north]->[B])([B]->[east]->[D])");
    for _ in 0..3 {
        must_ref(&mut pangine, "['outcome-routes'] ~= ([A]->[east]->[C])([C]->[north]->[D])");
    }
    must_ref(&mut pangine, "['outcome-routes'] -= x5(([A]->[north]->[B])([B]->[east]->[D]))");
    set_route_inputs(&mut pangine);

    let outcome_routes = pangine.reference_percept("outcome-routes");
    let outcome_sources = pangine.get_relevance_map(&outcome_routes);
    assert_eq!(outcome_sources.len(), 1);
    assert_eq!(outcome_sources[0].0, Relevance::DEFAULT);
    assert_eq!(pangine.format_concept(&outcome_sources[0].1, false), "x-4({[A]->[north]->[B]}{[B]->[east]->[D]})x3({[A]->[east]->[C]}{[C]->[north]->[D]})");
    assert_eq!(
        route_source_totals(&mut pangine),
        BTreeMap::from([
            ("{[A]->[east]->[C]}{[C]->[north]->[D]}".to_owned(), 3),
            ("{[A]->[north]->[B]}{[B]->[east]->[D]}".to_owned(), 4),
            ("{[A]->[west]->[E]}{[E]->[south]->[D]}".to_owned(), 4),
        ])
    );

    must_ref(&mut pangine, &format!("['moves']['outcome-routes'] @ {TWO_STEP_ROUTE}"));
    assert_eq!(
        must_ref(&mut pangine, "$(&['first-action'])"),
        must_ref(&mut pangine, "x4({[A]->[north]->[B]}{[B]->[east]->[D]})x4({[A]->[west]->[E]}{[E]->[south]->[D]})x3({[A]->[east]->[C]}{[C]->[north]->[D]})")
    );
    assert_eq!(must_ref(&mut pangine, "^(&['first-action'])"), must_ref(&mut pangine, "{[A]->[north]->[B]}{[B]->[east]->[D]}"));
}

#[test]
#[ignore = "warning: repeated route adaptation still uses explicit outcome roles and the placeholder tie rule"]
fn repeated_outcomes_regenerate_choice_without_losing_raw_episodes() {
    let mut pangine = Pangine::new();
    remember_weighted_routes(&mut pangine);
    set_route_inputs(&mut pangine);
    must_ref(&mut pangine, "['positive-result'] = [positive]->[success]");
    must_ref(&mut pangine, "['negative-result'] = [negative]->[diverged]");

    rebuild_route_choice(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "^['route-choice']"), must_ref(&mut pangine, "{[west]->[E]->[south]}"));
    record_chosen_episode(&mut pangine, "cycle-1", "diverged");

    rebuild_route_choice(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x3{[north]->[B]->[east]}x3{[west]->[E]->[south]}x2{[east]->[C]->[north]}"));
    assert_eq!(must_ref(&mut pangine, "^['route-choice']"), must_ref(&mut pangine, "{[north]->[B]->[east]}"));
    record_chosen_episode(&mut pangine, "cycle-2", "diverged");

    rebuild_route_choice(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "^['route-choice']"), must_ref(&mut pangine, "{[west]->[E]->[south]}"));
    record_chosen_episode(&mut pangine, "cycle-3", "diverged");

    rebuild_route_choice(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x2{[east]->[C]->[north]}x2{[north]->[B]->[east]}x2{[west]->[E]->[south]}"));
    assert_eq!(must_ref(&mut pangine, "^['route-choice']"), must_ref(&mut pangine, "{[east]->[C]->[north]}"));
    record_chosen_episode(&mut pangine, "cycle-4", "success");

    rebuild_route_choice(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['route-choice']"), must_ref(&mut pangine, "x3{[east]->[C]->[north]}x2{[north]->[B]->[east]}x2{[west]->[E]->[south]}"));
    assert_eq!(must_ref(&mut pangine, "^['route-choice']"), must_ref(&mut pangine, "{[east]->[C]->[north]}"));

    must_ref(&mut pangine, "['episodes'] @ [cycle-1]->[outcome]->['first-cycle-outcome']");
    must_ref(&mut pangine, "['episodes'] @ [cycle-4]->[outcome]->['last-cycle-outcome']");
    assert_eq!(must_ref(&mut pangine, "$['first-cycle-outcome']"), must_ref(&mut pangine, "[diverged]"));
    assert_eq!(must_ref(&mut pangine, "$['last-cycle-outcome']"), must_ref(&mut pangine, "[success]"));
}

fn route_source_totals(pangine: &mut Pangine) -> BTreeMap<String, i64> {
    let question = must_ref(pangine, TWO_STEP_ROUTE);
    let sources = [pangine.reference_percept("moves"), pangine.reference_percept("outcome-routes")];
    let result = pangine.complete_question(&sources, &question).expect("valid linked route question");
    let mut inventory = BTreeMap::<String, BTreeMap<(String, String), Relevance>>::new();
    for completion in result.completions() {
        let route = pangine.instantiate_completion(&question, completion).expect("complete route");
        let route = pangine.format_concept(&route, false);
        for evidence in completion.evidence() {
            let source_percept = pangine.format_concept(evidence.source_percept().expect("retained source"), false);
            let source_concept = pangine.format_concept(evidence.source_concept(), false);
            inventory.entry(route.clone()).or_default().insert((source_percept, source_concept), evidence.source_relevance());
        }
    }

    inventory
        .into_iter()
        .map(|(route, sources)| {
            let support = sources
                .values()
                .try_fold(Relevance::EMPTY, |sum, relevance| sum.checked_add(*relevance))
                .expect("route source total within signed relevance range");
            (route, support.weight())
        })
        .collect()
}

fn rebuild_route_choice(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['moves'] @
           ($['current']->['base-first-action']->['base-middle'])
           (['base-middle']->['base-second-action']->$['goal'])",
    );
    must_ref(pangine, "['route-choice'] = $(['base-first-action']->['base-middle']->['base-second-action'])");
    ask_episode_routes(pangine, "positive");
    must_ref(pangine, "['route-choice'] *= $(['positive-first-action']->['positive-middle']->['positive-second-action'])");
    ask_episode_routes(pangine, "negative");
    must_ref(pangine, "['route-choice'] /= $(['negative-first-action']->['negative-middle']->['negative-second-action'])");
}

fn record_chosen_episode(pangine: &mut Pangine, episode: &str, outcome: &str) {
    must_ref(pangine, "['selected-route'] = ^['route-choice']");
    must_ref(pangine, "['selected-route'] @ ['chosen-first-action']->['chosen-middle']->['chosen-second-action']");
    must_ref(pangine, "['recorded-first-action'] = $['chosen-first-action']");
    must_ref(pangine, "['recorded-middle'] = $['chosen-middle']");
    must_ref(pangine, "['recorded-second-action'] = $['chosen-second-action']");
    must_ref(pangine, &format!("['recorded-episode'] = [{episode}]"));
    must_ref(pangine, &format!("['recorded-outcome'] = [{outcome}]"));
    must_ref(
        pangine,
        "['episodes'] ~= (['recorded-episode']->[from]->['current'])
                          (['recorded-episode']->[first-action]->['recorded-first-action'])
                          (['recorded-episode']->[middle]->['recorded-middle'])
                          (['recorded-episode']->[second-action]->['recorded-second-action'])
                          (['recorded-episode']->[to]->['goal'])
                          (['recorded-episode']->[outcome]->['recorded-outcome'])",
    );
}

fn set_route_inputs(pangine: &mut Pangine) {
    must_ref(pangine, "['current'] = [A]");
    must_ref(pangine, "['goal'] = [D]");
}

fn observe_move(pangine: &mut Pangine, from: &str, action: &str, reached: &str) {
    must_ref(pangine, &format!("['observed-from'] = [{from}]"));
    must_ref(pangine, &format!("['observed-action'] = [{action}]"));
    must_ref(pangine, &format!("['observed-reached'] = [{reached}]"));
    must_ref(pangine, "['observed-moves'] ~= ['observed-from']->['observed-action']->['observed-reached']");
}

fn remember_move(pangine: &mut Pangine, percept: &str, move_concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("['{percept}'] ~= {move_concept}"));
    }
}

fn remember_weighted_routes(pangine: &mut Pangine) {
    remember_move(pangine, "moves", "[A]->[north]->[B]", 2);
    remember_move(pangine, "moves", "[B]->[east]->[D]", 1);
    remember_move(pangine, "moves", "[A]->[east]->[C]", 1);
    remember_move(pangine, "moves", "[C]->[north]->[D]", 1);
    remember_move(pangine, "moves", "[A]->[west]->[E]", 2);
    remember_move(pangine, "moves", "[E]->[south]->[D]", 2);
}

fn remember_episode(pangine: &mut Pangine, episode: &str, first_action: &str, middle: &str, second_action: &str, outcome: &str) {
    must_ref(
        pangine,
        &format!(
            "['episodes'] ~= ([{episode}]->[from]->[A])
                              ([{episode}]->[first-action]->[{first_action}])
                              ([{episode}]->[middle]->[{middle}])
                              ([{episode}]->[second-action]->[{second_action}])
                              ([{episode}]->[to]->[D])
                              ([{episode}]->[outcome]->[{outcome}])"
        ),
    );
}

fn ask_episode_routes(pangine: &mut Pangine, role: &str) {
    run_statement(
        pangine,
        &format!(
            "['episodes']['{role}-result'] @
               (['{role}-episode']->[from]->$['current'])
               (['{role}-episode']->[first-action]->['{role}-first-action'])
               (['{role}-episode']->[middle]->['{role}-middle'])
               (['{role}-episode']->[second-action]->['{role}-second-action'])
               (['{role}-episode']->[to]->$['goal'])
               (['{role}-episode']->[outcome]->['{role}-outcome'])
               ([{role}]->['{role}-outcome'])"
        ),
    );
}

fn run_statement(pangine: &mut Pangine, input: &str) {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
