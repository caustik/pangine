use super::*;

#[test]
fn an_outer_source_coefficient_does_not_replicate_the_fixture_completion() {
    let mut pangine = Pangine::new();
    let fact = must_ref(&mut pangine, "[fact]->[cat]->[eats]");
    let weighted = must_ref(&mut pangine, "x2([fact]->[cat]->[eats])");
    let question = must_ref(&mut pangine, "[fact]->{weighted-who}->{weighted-does}");

    let result = pangine.complete_subject(&weighted, &question).expect("valid weighted direct subject");
    let [completion] = result.completions() else {
        panic!("the weighted fact fixture should produce one completion");
    };
    assert_eq!(bound_name(&mut pangine, completion, "weighted-who"), "cat");
    assert_eq!(bound_name(&mut pangine, completion, "weighted-does"), "eats");
    let [evidence] = completion.evidence() else {
        panic!("the weighted fact fixture should supply one evidence fragment");
    };
    assert_eq!(evidence.source_concept(), &weighted);
    assert_eq!(evidence.matched(), &fact);
    assert_eq!(evidence.coefficient_ancestors().collect::<BTreeSet<_>>(), BTreeSet::from([&weighted]));
    assert_eq!(evidence.source_relevance(), Relevance::DEFAULT);

    let rows = must_ref(&mut pangine, "x2([fact]->[cat]->[eats]) @ [fact]->{surface-weighted-who}->{surface-weighted-does}");
    assert_eq!(rows, fact);
    let cat = must_ref(&mut pangine, "[cat]");
    let surface_who = pangine.reference_percept("surface-weighted-who");
    let surface_value = pangine.get_value(&surface_who).expect("materialized weighted-source binding");
    assert_eq!(pangine.get_relevance_map(&surface_value), vec![(Relevance::DEFAULT, cat)]);

    let whole_output = pangine.reference_percept("whole-weighted-value");
    let recursive = pangine.complete_subject(&weighted, &whole_output).expect("recursive-view question");
    assert_eq!(
        recursive.completions().iter().map(|completion| completion.binding(&whole_output).unwrap()).collect::<BTreeSet<_>>(),
        BTreeSet::from([&weighted, &fact, &must_ref(&mut pangine, "[fact]"), &must_ref(&mut pangine, "[cat]"), &must_ref(&mut pangine, "[eats]")]),
        "a top-level Percept should traverse coefficient boundaries like every other recursive source boundary"
    );
    let wrapper_completion = recursive.completions().iter().find(|completion| completion.binding(&whole_output) == Some(&weighted)).unwrap();
    assert!(wrapper_completion.evidence()[0].coefficient_ancestors().next().is_none(), "binding the wrapper crosses no coefficient boundary");
    assert!(recursive
        .completions()
        .iter()
        .filter(|completion| completion.binding(&whole_output) != Some(&weighted))
        .all(|completion| { completion.evidence()[0].coefficient_ancestors().collect::<BTreeSet<_>>() == BTreeSet::from([&weighted]) }));
}

#[test]
fn an_exact_coefficient_pattern_can_bind_inside_its_operand() {
    let mut pangine = Pangine::new();
    let weighted = must_ref(&mut pangine, "x2([fact]->[cat]->[eats])");
    let question = must_ref(&mut pangine, "x2([fact]->{coefficient-who}->{coefficient-does})");

    let result = pangine.complete_subject(&weighted, &question).expect("valid exact coefficient question");
    let [completion] = result.completions() else {
        panic!("the exact coefficient question should produce one completion");
    };
    assert_eq!(bound_name(&mut pangine, completion, "coefficient-who"), "cat");
    assert_eq!(bound_name(&mut pangine, completion, "coefficient-does"), "eats");
    let [evidence] = completion.evidence() else {
        panic!("the exact coefficient question should supply one evidence fragment");
    };
    assert_eq!(evidence.source_concept(), &weighted);
    assert_eq!(evidence.matched(), &weighted);
    assert!(evidence.coefficient_ancestors().next().is_none(), "matching the requested wrapper crosses no coefficient boundary");

    let different_coefficient = must_ref(&mut pangine, "x3([fact]->{other-who}->{other-does})");
    assert!(pangine.complete_subject(&weighted, &different_coefficient).expect("valid different coefficient question").completions().is_empty());
}

#[test]
fn equal_views_preserve_alternative_coefficient_routes_without_duplicate_rows() {
    let mut pangine = Pangine::new();
    let fact = must_ref(&mut pangine, "[fact]->[cat]->[eats]");
    let left_route = must_ref(&mut pangine, "x2([left]->([fact]->[cat]->[eats]))");
    let right_route = must_ref(&mut pangine, "x3([right]->([fact]->[cat]->[eats]))");
    let subject = must_ref(&mut pangine, "x2([left]->([fact]->[cat]->[eats]))x3([right]->([fact]->[cat]->[eats]))");
    let question = must_ref(&mut pangine, "[fact]->{route-who}->{route-does}");

    let result = pangine.complete_subject(&subject, &question).unwrap();
    let [completion] = result.completions() else {
        panic!("two unrequested routes to one canonical fact should produce one logical row");
    };
    let [evidence] = completion.evidence() else {
        panic!("the canonical matched fact should remain one evidence fragment");
    };
    assert_eq!(evidence.matched(), &fact);
    assert_eq!(
        evidence.coefficient_ancestor_routes().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([BTreeSet::from([left_route]), BTreeSet::from([right_route])]),
        "either represented route can support the fact; they are not one simultaneous two-factor route"
    );
    assert_eq!(bound_name(&mut pangine, completion, "route-who"), "cat");
    assert_eq!(bound_name(&mut pangine, completion, "route-does"), "eats");

    let grouped_fact_subject = must_ref(&mut pangine, "x2([left]->(([kind]->[fact])([value]->[A])))x3([right]->(([kind]->[fact])([value]->[A])))");
    let left_group_route = must_ref(&mut pangine, "x2([left]->(([kind]->[fact])([value]->[A])))");
    let right_group_route = must_ref(&mut pangine, "x3([right]->(([kind]->[fact])([value]->[A])))");
    let grouped_fact_question = must_ref(&mut pangine, "([kind]->[fact])([value]->{route-group-value})");
    let grouped_fact = pangine.complete_subject(&grouped_fact_subject, &grouped_fact_question).unwrap();
    let [grouped_completion] = grouped_fact.completions() else {
        panic!("the same grouped value through two unrequested addresses should remain one correlated row");
    };
    assert_eq!(bound_name(&mut pangine, grouped_completion, "route-group-value"), "A");
    assert_eq!(grouped_completion.evidence().len(), 2);
    assert!(grouped_completion.evidence().iter().all(|evidence| {
        evidence.selected_entries().next().is_none()
            && evidence.coefficient_ancestor_routes().cloned().collect::<BTreeSet<_>>()
                == BTreeSet::from([BTreeSet::from([left_group_route.clone()]), BTreeSet::from([right_group_route.clone()])])
    }));
}

#[test]
fn selected_entries_preserve_distinct_proofs_even_when_the_grounding_is_equal() {
    let mut pangine = Pangine::new();
    let left_entry = must_ref(&mut pangine, "([fact]->[same])([tag]->[L])");
    let right_entry = must_ref(&mut pangine, "([fact]->[same])([tag]->[R])");
    let subject = must_ref(&mut pangine, "(([fact]->[same])([tag]->[L]))(([fact]->[same])([tag]->[R]))");
    let question = must_ref(&mut pangine, "[fact]->{same-answer}");
    let result = pangine.complete_subject(&subject, &question).unwrap();

    assert_eq!(result.completions().len(), 2, "selected complete entries are distinct proofs even when they bind the same answer");
    assert!(result.completions().iter().all(|completion| bound_name(&mut pangine, completion, "same-answer") == "same"));
    assert_eq!(
        result
            .completions()
            .iter()
            .map(|completion| {
                let mut routes = completion.evidence()[0].routes();
                let route = routes.next().expect("selected-entry route");
                assert!(routes.next().is_none(), "each selected-entry proof should retain one route");
                route.selected_entries().find_map(|(container, entry)| (container == &subject).then_some(entry.clone())).expect("selected root entry")
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([left_entry, right_entry])
    );
}

#[test]
fn an_outer_coefficient_exposes_graph_clause_views_without_replication() {
    let mut pangine = Pangine::new();
    let graph = must_ref(&mut pangine, "([left]->[A])([right]->[A])");
    let weighted = must_ref(&mut pangine, "x2(([left]->[A])([right]->[A]))");
    let question = must_ref(&mut pangine, "([left]->{joined-value})([right]->{joined-value})");

    let result = pangine.complete_subject(&weighted, &question).expect("valid weighted graph subject");
    let [completion] = result.completions() else {
        panic!("the shared graph fixture should produce one completion");
    };
    assert_eq!(bound_name(&mut pangine, completion, "joined-value"), "A");
    assert_eq!(completion.evidence().len(), 2, "coefficient magnitude must not change the graph's two clause proofs");
    assert_eq!(
        completion.evidence().iter().map(|evidence| evidence.matched().clone()).collect::<BTreeSet<_>>(),
        BTreeSet::from([must_ref(&mut pangine, "[left]->[A]"), must_ref(&mut pangine, "[right]->[A]")])
    );
    for evidence in completion.evidence() {
        assert_eq!(evidence.source_concept(), &weighted);
        assert_eq!(evidence.source_relevance(), Relevance::DEFAULT);
        assert_eq!(evidence.coefficient_ancestors().collect::<BTreeSet<_>>(), BTreeSet::from([&weighted]));
    }

    let unweighted = pangine.complete_subject(&graph, &question).expect("valid unweighted graph subject");
    let [unweighted_completion] = unweighted.completions() else {
        panic!("the unweighted graph should produce the same correlated row");
    };
    assert_eq!(unweighted_completion.evidence().len(), 2);
    assert!(unweighted_completion.evidence().iter().all(|evidence| evidence.coefficient_ancestors().next().is_none()));

    let rows = must_ref(&mut pangine, "x2(([left]->[A])([right]->[A])) @ ([left]->{surface-joined-value})([right]->{surface-joined-value})");
    assert_eq!(rows, graph, "recognition must not reinterpret the coefficient as row multiplicity");
    let surface_value = pangine.reference_percept("surface-joined-value");
    let surface_value = pangine.get_value(&surface_value).expect("materialized shared binding");
    let a = must_ref(&mut pangine, "[A]");
    assert_eq!(pangine.get_relevance_map(&surface_value), vec![(Relevance::DEFAULT, a)]);

    let disagreeing = must_ref(&mut pangine, "x2(([left]->[A])([right]->[B]))");
    assert!(pangine.complete_subject(&disagreeing, &question).unwrap().completions().is_empty());
}

#[test]
fn preserved_unordered_entries_bound_correlated_clause_joins() {
    let mut pangine = Pangine::new();
    let question = must_ref(&mut pangine, "([left]->{same})([right]->{same})");

    let flat = must_ref(&mut pangine, "([left]->[A])([right]->[B])([left]->[B])([right]->[A])");
    let flat_result = pangine.complete_subject(&flat, &question).unwrap();
    assert!(flat_result.completions().iter().all(|completion| completion.evidence().iter().all(|evidence| evidence.selected_entries().next().is_none())));
    assert_eq!(
        flat_result.completions().iter().map(|completion| bound_name(&mut pangine, completion, "same")).collect::<BTreeSet<_>>(),
        BTreeSet::from(["A".to_owned(), "B".to_owned()]),
        "flat relation entries may be recombined by their shared binding"
    );

    let grouped = must_ref(&mut pangine, "(([left]->[A])([right]->[B]))(([left]->[B])([right]->[A]))");
    assert!(
        pangine.complete_subject(&grouped, &question).unwrap().completions().is_empty(),
        "one clause from each complete grouped entry must not invent a row that neither entry contains"
    );

    let weighted_groups = must_ref(&mut pangine, "x2(([left]->[A])([right]->[B]))x3(([left]->[B])([right]->[A]))");
    assert!(
        pangine.complete_subject(&weighted_groups, &question).unwrap().completions().is_empty(),
        "coefficients must not weaken the same ordinary entry boundary"
    );

    let weighted_atoms = must_ref(&mut pangine, "x2([left]->[A])x3([right]->[A])");
    let weighted_atoms = pangine.complete_subject(&weighted_atoms, &question).unwrap();
    let [completion] = weighted_atoms.completions() else {
        panic!("separately weighted flat atoms should remain independently joinable");
    };
    assert_eq!(bound_name(&mut pangine, completion, "same"), "A");
    assert_eq!(completion.evidence().iter().flat_map(|evidence| evidence.coefficient_ancestors()).collect::<BTreeSet<_>>().len(), 2);

    let grouped_payload_atoms = must_ref(&mut pangine, "([left]->(([kind]->[one])([value]->[A]))) ([right]->(([kind]->[two])([value]->[B])))");
    let grouped_payload_question = must_ref(&mut pangine, "([left]->{left-group})([right]->{right-group})");
    let grouped_payloads = pangine.complete_subject(&grouped_payload_atoms, &grouped_payload_question).unwrap();
    let [grouped_payload_completion] = grouped_payloads.completions() else {
        panic!("flat relation atoms remain joinable even when their values are grouped Concepts");
    };
    assert!(grouped_payload_completion.evidence().iter().all(|evidence| evidence.selected_entries().next().is_none()));

    let correlated_groups = must_ref(&mut pangine, "(([left]->[A])([right]->[A]))(([left]->[B])([right]->[B]))");
    let correlated = pangine.complete_subject(&correlated_groups, &question).unwrap();
    assert_eq!(
        correlated.completions().iter().map(|completion| bound_name(&mut pangine, completion, "same")).collect::<BTreeSet<_>>(),
        BTreeSet::from(["A".to_owned(), "B".to_owned()])
    );
    assert!(correlated.completions().iter().all(|completion| {
        let selected = completion
            .evidence()
            .iter()
            .flat_map(|evidence| evidence.selected_entries().map(|(container, entry)| (container.clone(), entry.clone())))
            .collect::<BTreeSet<_>>();
        selected.len() == 1 && selected.iter().all(|(container, _)| container == &correlated_groups)
    }));

    experience(&mut pangine, "left-source", "([left]->[A])([left-context]->[L])", 1);
    experience(&mut pangine, "right-source", "([right]->[A])([right-context]->[R])", 1);
    let separate_sources = complete(&mut pangine, &["left-source", "right-source"], "([left]->{cross-source})([right]->{cross-source})");
    let [completion] = separate_sources.completions() else {
        panic!("entry commitments are local to their represented source, not a ban on cross-source evidence joins");
    };
    assert_eq!(bound_name(&mut pangine, completion, "cross-source"), "A");
}

#[test]
fn ordered_windows_preserve_the_complete_path_that_supplied_them() {
    let mut pangine = Pangine::new();
    let first_path = must_ref(&mut pangine, "[A]->[r]->[M]->[s]->[D]");
    let second_path = must_ref(&mut pangine, "[X]->[r]->[M]->[s]->[E]");
    let subject = must_ref(&mut pangine, "([A]->[r]->[M]->[s]->[D])([X]->[r]->[M]->[s]->[E])");
    let question = must_ref(&mut pangine, "({path-start}->[r]->{path-mid})({path-mid}->[s]->{path-end})");
    let result = pangine.complete_subject(&subject, &question).unwrap();
    assert_eq!(result.completions().len(), 2, "proper windows from distinct complete paths must not cross through an equal middle value");
    assert_eq!(
        result
            .completions()
            .iter()
            .map(|completion| (bound_name(&mut pangine, completion, "path-start"), bound_name(&mut pangine, completion, "path-end")))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([("A".to_owned(), "D".to_owned()), ("X".to_owned(), "E".to_owned())])
    );

    for completion in result.completions() {
        assert_eq!(completion.evidence().len(), 2);
        let expected_path = if bound_name(&mut pangine, completion, "path-start") == "A" { &first_path } else { &second_path };
        for evidence in completion.evidence() {
            let local_routes = evidence.routes().collect::<Vec<_>>();
            assert_eq!(local_routes.len(), 1);
            let selected = local_routes[0].selected_entries().collect::<Vec<_>>();
            assert_eq!(selected, vec![(&subject, expected_path)]);
            let windows = local_routes[0].ordered_windows().collect::<Vec<_>>();
            assert_eq!(windows.len(), 1);
            assert_eq!(windows[0].parent(), expected_path);
            assert_eq!(windows[0].width(), 3);
        }
        let source_products = completion.evidence()[0].source_route_products().collect::<Vec<_>>();
        assert_eq!(source_products.len(), 1);
        assert_eq!(source_products[0].selected_entries().collect::<Vec<_>>(), vec![(&subject, expected_path)]);
        assert_eq!(source_products[0].ordered_windows().count(), 0, "descriptive windows remain factorized on their clause evidence");
        assert!(completion.evidence().iter().all(|evidence| evidence.source_route_products().eq(source_products.iter().copied())));
    }
}

#[test]
fn ordered_window_joins_preserve_the_shared_binding_occurrence_inside_one_path() {
    let mut pangine = Pangine::new();
    let subject = must_ref(&mut pangine, "[A]->[r]->[M]->[s]->[D]->[gap]->[X]->[r]->[M]->[s]->[E]");
    let question = must_ref(&mut pangine, "({repeat-start}->[r]->{repeat-mid})({repeat-mid}->[s]->{repeat-end})");
    let result = pangine.complete_subject(&subject, &question).unwrap();

    assert_eq!(
        result
            .completions()
            .iter()
            .map(|completion| (bound_name(&mut pangine, completion, "repeat-start"), bound_name(&mut pangine, completion, "repeat-end")))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([("A".to_owned(), "D".to_owned()), ("X".to_owned(), "E".to_owned())]),
        "equal middle values at different positions must not splice two unrelated windows into a path"
    );

    let middle = pangine.reference_percept("repeat-mid");
    for completion in result.completions() {
        let routes = completion.evidence()[0].source_route_products().collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            panic!("each surviving path should have one complete source route");
        };
        let origins = route.binding_origins().find_map(|(percept, origins)| (percept == &middle).then_some(origins)).expect("shared middle occurrence");
        let origins = origins.iter().collect::<Vec<_>>();
        let [origin] = origins.as_slice() else {
            panic!("the shared middle should resolve to one represented position");
        };
        assert_eq!(origin.parent(), &subject);
        assert_eq!(origin.span_width(), 1);
        assert!(matches!(origin.span_start(), 2 | 8));
    }
}

#[test]
fn equal_nested_ordered_values_retain_distinct_occurrence_routes_without_duplicate_rows() {
    let mut pangine = Pangine::new();
    let left_entry = must_ref(&mut pangine, "[left]->([A]->[r]->[M]->[s]->[D])");
    let right_entry = must_ref(&mut pangine, "[right]->([A]->[r]->[M]->[s]->[D])");
    let subject = must_ref(&mut pangine, "([left]->([A]->[r]->[M]->[s]->[D]))([right]->([A]->[r]->[M]->[s]->[D]))");
    let question = must_ref(&mut pangine, "({nested-start}->[r]->{nested-mid})({nested-mid}->[s]->{nested-end})");
    let result = pangine.complete_subject(&subject, &question).unwrap();
    let [completion] = result.completions() else {
        panic!("equal values reached at two represented positions should remain one grounding");
    };
    assert_eq!(bound_name(&mut pangine, completion, "nested-start"), "A");
    assert_eq!(bound_name(&mut pangine, completion, "nested-end"), "D");

    let middle = pangine.reference_percept("nested-mid");
    let products = completion.evidence()[0].source_route_products().collect::<Vec<_>>();
    assert_eq!(products.len(), 2, "the grounding should retain both complete occurrence routes");
    let containing_entries = products
        .iter()
        .map(|route| {
            let origins = route.binding_origins().find_map(|(percept, origins)| (percept == &middle).then_some(origins)).expect("shared nested middle origin");
            let parent_occurrences = origins
                .iter()
                .map(|origin| {
                    let steps = origin.parent_occurrence().collect::<Vec<_>>();
                    let [step] = steps.as_slice() else {
                        panic!("the nested path should have one enclosing ordered occurrence");
                    };
                    assert_eq!(step.position(), 1);
                    step.parent().clone()
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(parent_occurrences.len(), 1, "both windows in one product should come from the same enclosing occurrence");
            parent_occurrences.into_iter().next().unwrap()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(containing_entries, BTreeSet::from([left_entry, right_entry]));
}

fn complete(pangine: &mut Pangine, source_names: &[&str], question: &str) -> CompletionResult {
    let sources = source_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let question = must_ref(pangine, question);
    pangine.complete_question(&sources, &question).expect("valid structural question")
}

fn bound_name(pangine: &mut Pangine, completion: &Completion, percept: &str) -> String {
    let percept = must_ref(pangine, &format!("{{{percept}}}"));
    pangine.get_name(completion.binding(&percept).expect("bound Percept")).expect("named binding").to_owned()
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("{{{percept}}} ~= {concept}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
