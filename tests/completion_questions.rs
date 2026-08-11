use pangine::{Completion, CompletionRemainderSide, CompletionResult, ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn shared_holes_compose_relationships_while_one_atom_remains_one_step() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "knowledge", "[Socrates]->[is-a]->[human]", 1);
    experience(&mut pangine, "knowledge", "[human]->[is-a]->[mortal]", 1);

    let composed = complete(&mut pangine, &["knowledge"], "([Socrates]->[is-a]->['middle'])(['middle']->[is-a]->['conclusion'])");
    assert_eq!(composed.completions().len(), 1);
    assert_eq!(bound_name(&mut pangine, &composed.completions()[0], "middle"), "human");
    assert_eq!(bound_name(&mut pangine, &composed.completions()[0], "conclusion"), "mortal");

    let conclusion = must_ref(&mut pangine, "[Socrates]->[is-a]->['conclusion']");
    let conclusion = pangine.instantiate_completion(&conclusion, &composed.completions()[0]).expect("complete conclusion template");
    assert_eq!(pangine.format_concept(&conclusion, false), "{[Socrates]->[is-a]->[mortal]}");

    let one_step = complete(&mut pangine, &["knowledge"], "[Socrates]->[is-a]->['answer']");
    assert_eq!(one_step.completions().len(), 1);
    assert_eq!(bound_name(&mut pangine, &one_step.completions()[0], "answer"), "human");
}

#[test]
fn nested_unordered_concepts_are_questioned_as_ordinary_structure() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "([A][B])([C][D])", 1);

    let result = complete(&mut pangine, &["memory"], "([A]['left'])([C]['right'])");
    assert_eq!(result.completions().len(), 1);
    assert_eq!(bound_name(&mut pangine, &result.completions()[0], "left"), "B");
    assert_eq!(bound_name(&mut pangine, &result.completions()[0], "right"), "D");

    let row = must_ref(&mut pangine, "['memory'] @ ([A]['surface-left'])([C]['surface-right'])");
    let formatted = pangine.format_concept(&row, false);
    assert_eq!(formatted, "([A][B])([C][D])");
    assert_eq!(must_ref(&mut pangine, &formatted), row);
}

#[test]
fn at_uses_explicit_graph_composition_instead_of_implicit_contextual_widening() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "knowledge", "[Socrates]->[is-a]->[human]", 1);
    experience(&mut pangine, "knowledge", "[human]->[is-a]->[mortal]", 1);

    must_ref(&mut pangine, "['knowledge'] @ ([Socrates]->[is-a]->['middle'])(['middle']->[is-a]->['conclusion'])");
    let conclusion = must_ref(&mut pangine, "$['conclusion']");
    assert_eq!(pangine.format_concept(&conclusion, false), "[mortal]");

    must_ref(&mut pangine, "['knowledge'] @ [Socrates]->[is-a]->['one-step']");
    let one_step = must_ref(&mut pangine, "$['one-step']");
    assert_eq!(pangine.format_concept(&one_step, false), "[human]");

    experience(&mut pangine, "rule110", "[1]->[1]->[1]->[0]", 1);
    experience(&mut pangine, "rule110", "[0]->[1]->[1]->[1]", 1);
    must_ref(&mut pangine, "['rule110'] @ [1]->[1]->[1]->['next']");
    let next = must_ref(&mut pangine, "$['next']");
    assert_eq!(pangine.format_concept(&next, false), "[0]");
}

#[test]
fn one_generic_graph_join_computes_a_complete_rule110_generation() {
    let mut pangine = Pangine::new();
    for (position, value) in [("boundary-left", "0"), ("p0", "0"), ("p1", "1"), ("p2", "1"), ("p3", "1"), ("p4", "0"), ("boundary-right", "0")] {
        experience(&mut pangine, "state", &format!("[cell]->[{position}]->[{value}]"), 1);
    }
    for (left, right) in [("boundary-left", "p0"), ("p0", "p1"), ("p1", "p2"), ("p2", "p3"), ("p3", "p4"), ("p4", "boundary-right")] {
        experience(&mut pangine, "state", &format!("[next-position]->[{left}]->[{right}]"), 1);
    }
    for (left, center, right, next) in [
        ("1", "1", "1", "0"),
        ("1", "1", "0", "1"),
        ("1", "0", "1", "1"),
        ("1", "0", "0", "0"),
        ("0", "1", "1", "1"),
        ("0", "1", "0", "1"),
        ("0", "0", "1", "1"),
        ("0", "0", "0", "0"),
    ] {
        experience(&mut pangine, "rule110", &format!("[transition]->[{left}]->[{center}]->[{right}]->[{next}]"), 1);
    }

    let result = complete(
        &mut pangine,
        &["state", "rule110"],
        "([cell]->['left-position']->['left'])
         ([next-position]->['left-position']->['center-position'])
         ([cell]->['center-position']->['center'])
         ([next-position]->['center-position']->['right-position'])
         ([cell]->['right-position']->['right'])
         ([transition]->['left']->['center']->['right']->['next'])",
    );

    assert_eq!(result.completions().len(), 5);
    let update_template = must_ref(&mut pangine, "[cell]->['center-position']->['next']");
    let updates = result
        .completions()
        .iter()
        .map(|completion| {
            let update = pangine.instantiate_completion(&update_template, completion).expect("complete cell-update template");
            pangine.format_concept(&update, false)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        updates,
        BTreeSet::from([
            "{[cell]->[p0]->[1]}".to_owned(),
            "{[cell]->[p1]->[1]}".to_owned(),
            "{[cell]->[p2]->[0]}".to_owned(),
            "{[cell]->[p3]->[1]}".to_owned(),
            "{[cell]->[p4]->[0]}".to_owned(),
        ])
    );
}

#[test]
fn unmatched_context_survives_the_same_completion_that_supplies_a_residual() {
    let mut pangine = Pangine::new();

    let direct_source = must_ref(&mut pangine, "([room][kettle][empty])->[result]");
    let direct_question = must_ref(&mut pangine, "([kettle][empty])->['direct-answer']");
    let direct = pangine.complete_subject(&direct_source, &direct_question).expect("valid direct containment question");
    assert_eq!(direct.completions().len(), 1);
    let direct_remainders = direct.completions()[0].evidence()[0].remainders().collect::<Vec<_>>();
    assert_eq!(direct_remainders.len(), 1);
    assert!(direct_remainders[0].side() == CompletionRemainderSide::Source);
    assert_eq!(direct_remainders[0].ordered_path(), &[0]);
    assert_eq!(pangine.format_concept(direct_remainders[0].concept(), false), "[room]");

    experience(&mut pangine, "rules", "([kettle][empty])->([full]/[empty])", 1);

    let result = complete(&mut pangine, &["rules"], "([room][kettle][empty])->['delta']");
    assert_eq!(result.completions().len(), 1);
    let completion = &result.completions()[0];
    assert_eq!(completion.evidence().len(), 1);
    let remainders = completion.evidence()[0].remainders().collect::<Vec<_>>();
    assert_eq!(remainders.len(), 1);
    assert!(remainders[0].side() == CompletionRemainderSide::Question);
    assert_eq!(remainders[0].ordered_path(), &[0]);
    assert_eq!(pangine.format_concept(remainders[0].concept(), false), "[room]");

    let delta = must_ref(&mut pangine, "['delta']");
    let delta = pangine.instantiate_completion(&delta, completion).expect("complete residual template");
    assert_eq!(pangine.format_concept(&delta, false), "[full]![empty]");

    let state = pangine.reference_percept("current-state");
    let initial_state = must_ref(&mut pangine, "[room][kettle][empty]");
    pangine.set_percept_value(&state, Some(initial_state));
    pangine.perform_merge(&state, Some(&delta));
    assert_eq!(pangine.format_concept(&pangine.get_value(&state).expect("updated state"), false), "[full][kettle][room]");

    must_ref(&mut pangine, "['rules'] @ ([room][kettle][empty])->['at-delta']");
    let at_delta_percept = pangine.reference_percept("at-delta");
    let stored_at_delta = pangine.get_value(&at_delta_percept).expect("stored composite answer");
    let stored_at_delta_text = pangine.format_concept(&stored_at_delta, false);
    assert_eq!(stored_at_delta_text, "[full]![empty]");
    assert_eq!(must_ref(&mut pangine, &stored_at_delta_text), stored_at_delta);
    let at_delta = must_ref(&mut pangine, "$['at-delta']");
    assert_eq!(pangine.format_concept(&at_delta, false), "[full]![empty]");
}

#[test]
fn correlated_rows_survive_before_any_marginal_projection() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "pairs", "[A]->[D]", 1);
    experience(&mut pangine, "pairs", "[B]->[C]", 1);

    let result = complete(&mut pangine, &["pairs"], "['left']->['right']");
    let pairs = result
        .completions()
        .iter()
        .map(|completion| (bound_name(&mut pangine, completion, "left"), bound_name(&mut pangine, completion, "right")))
        .collect::<BTreeSet<_>>();
    assert_eq!(pairs, BTreeSet::from([("A".to_owned(), "D".to_owned()), ("B".to_owned(), "C".to_owned())]));

    let rows = must_ref(&mut pangine, "['rows'] = (['pairs'] @ ['row-left']->['row-right'])");
    let row_members = pangine.get_relevance_map(&rows).into_iter().map(|(_, row)| pangine.format_concept(&row, false)).collect::<BTreeSet<_>>();
    assert_eq!(row_members, BTreeSet::from(["{[A]->[D]}".to_owned(), "{[B]->[C]}".to_owned()]));
    let rows_percept = pangine.reference_percept("rows");
    assert_eq!(pangine.get_value(&rows_percept), Some(rows));
}

#[test]
fn ordinary_concepts_are_one_structural_source_for_the_same_completion_calculus() {
    let mut pangine = Pangine::new();
    let subject = must_ref(&mut pangine, "{[cat]->[eats]}{[dog]->[sleeps]}");
    let question = must_ref(&mut pangine, "['what']->['whats']");
    let result = pangine.complete_subject(&subject, &question).expect("valid ordinary subject");

    let pairs = result
        .completions()
        .iter()
        .map(|completion| {
            assert_eq!(completion.evidence().len(), 1);
            let evidence = &completion.evidence()[0];
            assert!(evidence.source_percept().is_none());
            assert_eq!(evidence.source_subject(), &subject);
            assert_eq!(evidence.source_concept(), &subject);
            assert_eq!(evidence.source_relevance(), Relevance::DEFAULT);
            (bound_name(&mut pangine, completion, "what"), bound_name(&mut pangine, completion, "whats"))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(pairs, BTreeSet::from([("cat".to_owned(), "eats".to_owned()), ("dog".to_owned(), "sleeps".to_owned())]));

    let rows = must_ref(&mut pangine, "{[cat]->[eats]}{[dog]->[sleeps]} @ ['surface-what']->['surface-whats']");
    assert_eq!(pangine.debug_console_lines(Some(&rows)), vec!["  {[cat]->[eats]}", "  {[dog]->[sleeps]}"]);
    let surface_what = must_ref(&mut pangine, "$['surface-what']");
    let surface_whats = must_ref(&mut pangine, "$['surface-whats']");
    assert_eq!(pangine.format_concept(&surface_what, false), "[cat][dog]");
    assert_eq!(pangine.format_concept(&surface_whats, false), "[eats][sleeps]");

    for concept in ["[cat]->[eats]", "[dog]->[sleeps]"] {
        experience(&mut pangine, "test", concept, 1);
    }
    let retained = complete(&mut pangine, &["test"], "['retained-what']->['retained-whats']");
    let test = pangine.reference_percept("test");
    for completion in retained.completions() {
        let evidence = &completion.evidence()[0];
        assert_eq!(evidence.source_percept(), Some(&test));
        assert_eq!(evidence.source_subject(), &test);
        assert_ne!(evidence.source_concept(), &subject);
    }
}

#[test]
fn graph_rows_can_be_stored_round_tripped_and_directly_questioned_again() {
    let mut pangine = Pangine::new();
    for concept in ["[A]->[r]->[B]", "[B]->[s]->[C]", "[X]->[r]->[Y]", "[Y]->[s]->[Z]"] {
        experience(&mut pangine, "memory", concept, 1);
    }

    let rows = must_ref(&mut pangine, "['rows'] = (['memory'] @ (['start']->[r]->['middle'])(['middle']->[s]->['end']))");
    let formatted = pangine.format_concept(&rows, false);
    assert_eq!(formatted, "({[A]->[r]->[B]}{[B]->[s]->[C]})({[X]->[r]->[Y]}{[Y]->[s]->[Z]})");
    let reparsed = must_ref(&mut pangine, &formatted);
    assert_eq!(reparsed, rows, "formatted result did not preserve its row boundaries: {formatted}");

    let direct_question = must_ref(&mut pangine, "(['direct-start']->[r]->['direct-middle'])(['direct-middle']->[s]->['direct-end'])");
    let direct = pangine.complete_subject(&reparsed, &direct_question).expect("question grounded rows directly");
    let direct_paths = direct
        .completions()
        .iter()
        .map(|completion| {
            (
                bound_name(&mut pangine, completion, "direct-start"),
                bound_name(&mut pangine, completion, "direct-middle"),
                bound_name(&mut pangine, completion, "direct-end"),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(direct_paths, BTreeSet::from([("A".to_owned(), "B".to_owned(), "C".to_owned()), ("X".to_owned(), "Y".to_owned(), "Z".to_owned()),]));

    let chained = must_ref(
        &mut pangine,
        "(['memory'] @ (['chain-start']->[r]->['chain-middle'])(['chain-middle']->[s]->['chain-end']))
         @ (['next-start']->[r]->['next-middle'])(['next-middle']->[s]->['next-end'])",
    );
    assert_eq!(chained, rows);
    let next_start = must_ref(&mut pangine, "$['next-start']");
    let next_middle = must_ref(&mut pangine, "$['next-middle']");
    let next_end = must_ref(&mut pangine, "$['next-end']");
    assert_eq!(pangine.format_concept(&next_start, false), "[A][X]");
    assert_eq!(pangine.format_concept(&next_middle, false), "[B][Y]");
    assert_eq!(pangine.format_concept(&next_end, false), "[C][Z]");

    let restored = pangine.reference_percept("restored-rows");
    assert!(pangine.set_percept_value(&restored, Some(reparsed)));

    let result = complete(&mut pangine, &["restored-rows"], "(['next-start']->[r]->['next-middle'])(['next-middle']->[s]->['next-end'])");
    let paths = result
        .completions()
        .iter()
        .map(|completion| {
            (
                bound_name(&mut pangine, completion, "next-start"),
                bound_name(&mut pangine, completion, "next-middle"),
                bound_name(&mut pangine, completion, "next-end"),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(paths, BTreeSet::from([("A".to_owned(), "B".to_owned(), "C".to_owned()), ("X".to_owned(), "Y".to_owned(), "Z".to_owned()),]));

    let surface_rows = must_ref(&mut pangine, "['restored-rows'] @ (['surface-start']->[r]->['surface-middle'])(['surface-middle']->[s]->['surface-end'])");
    let row_members = pangine.get_relevance_map(&surface_rows).into_iter().map(|(_, row)| pangine.format_concept(&row, false)).collect::<BTreeSet<_>>();
    assert_eq!(row_members.len(), 2);

    let end = must_ref(&mut pangine, "$['surface-end']");
    let marginal = pangine.get_relevance_map(&end).into_iter().map(|(_, value)| pangine.format_concept(&value, false)).collect::<BTreeSet<_>>();
    assert_eq!(marginal, BTreeSet::from(["[C]".to_owned(), "[Z]".to_owned()]));
}

#[test]
fn explicit_evidence_factors_remain_separate_without_declaring_bayesian_semantics() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "prior-factor", "[prior]->[disease]", 1);
    experience(&mut pangine, "prior-factor", "[prior]->[healthy]", 9);
    experience(&mut pangine, "positive-factor", "[positive]->[disease]", 4);
    experience(&mut pangine, "positive-factor", "[positive]->[healthy]", 1);

    let result = complete(&mut pangine, &["prior-factor", "positive-factor"], "([prior]->['hypothesis'])([positive]->['hypothesis'])");
    assert_eq!(result.completions().len(), 2);

    let inventory = result
        .completions()
        .iter()
        .map(|completion| {
            let hypothesis = bound_name(&mut pangine, completion, "hypothesis");
            let factors = completion
                .evidence()
                .iter()
                .map(|evidence| {
                    let source = pangine.format_concept(evidence.source_percept().expect("retained Percept source"), false);
                    (source, evidence.source_relevance())
                })
                .collect::<BTreeMap<_, _>>();
            (hypothesis, factors)
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        inventory["disease"],
        BTreeMap::from([("['positive-factor']".to_owned(), Relevance::new(4)), ("['prior-factor']".to_owned(), Relevance::DEFAULT),])
    );
    assert_eq!(
        inventory["healthy"],
        BTreeMap::from([("['positive-factor']".to_owned(), Relevance::DEFAULT), ("['prior-factor']".to_owned(), Relevance::new(9)),])
    );
}

#[test]
fn console_question_results_are_grounded_rows() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "Alice", "[cat]->[purrs]", 1);

    let direct = must_ref(&mut pangine, "['Alice'] @ [cat]->['sound']");
    assert_eq!(pangine.debug_console_lines(Some(&direct)), vec!["  {[cat]->[purrs]}"]);

    for concept in ["[kitchen]->[connected-to]->[living-room]", "[kitchen]->[sound]->[fridge-hum]", "[living-room]->[sound]->[music]"] {
        experience(&mut pangine, "Room", concept, 1);
    }
    let composed = must_ref(&mut pangine, "['Room'] @ ([kitchen]->[connected-to]->['where'])(['where']->[sound]->['indirect-answer'])");
    assert_eq!(pangine.debug_console_lines(Some(&composed)), vec!["  {[kitchen]->[connected-to]->[living-room]}", "  {[living-room]->[sound]->[music]}"]);

    experience(&mut pangine, "world", "[morning][birds]", 2);
    experience(&mut pangine, "world", "[morning][traffic]", 1);
    let rows = must_ref(&mut pangine, "['world'] @ [morning]['answer']");
    assert_eq!(pangine.debug_console_lines(Some(&rows)), vec!["  [birds][morning]", "  [morning][traffic]"]);
}

fn complete(pangine: &mut Pangine, source_names: &[&str], question: &str) -> CompletionResult {
    let sources = source_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let question = must_ref(pangine, question);
    pangine.complete_question(&sources, &question).expect("valid structural question")
}

fn bound_name(pangine: &mut Pangine, completion: &Completion, percept: &str) -> String {
    let percept = must_ref(pangine, &format!("['{percept}']"));
    pangine.get_name(completion.binding(&percept).expect("bound Percept")).expect("named binding").to_owned()
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("['{percept}'] ~= {concept}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
