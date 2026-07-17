use pangine::{ConceptId, Pangine, Relevance};

#[test]
fn question_ranks_exact_correlation_above_generic_role_evidence() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->[D]}");
    ask_question(&mut pangine, "['memory'] @ {['X']->[A]}");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    let c = candidates.iter().find(|(_, name)| name == "C").unwrap().0;
    let b = candidates.iter().find(|(_, name)| name == "B").unwrap().0;

    assert!(c.weight() > b.weight());
    assert!(b.weight() > 0.0);
}

#[test]
fn question_keeps_correlation_output_bindings_distinct() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}*{[B]->[D]}");
    let question = must_ref(&mut pangine, "['memory'] @ {['X']->[A]}*{[B]->['Y']}");
    assert_eq!(
        question,
        must_ref(&mut pangine, "{['X']->[A]}*{[B]->['Y']}")
    );

    let answer = must_ref(&mut pangine, "$['memory'] @ {['X']->[A]}*{[B]->['Y']}");
    assert_eq!(
        answer,
        must_ref(
            &mut pangine,
            "{<x12[C], x4[B]>->[A]}*{[B]-><x12[D], x4[A]>}",
        )
    );

    let x = must_ref(&mut pangine, "$['X']");
    let x_candidates = named_relevance(&pangine, &x);
    assert_eq!(x_candidates[0].1, "C");
    assert_eq!(candidate_weight(&x_candidates, "C"), 12.0);
    assert_eq!(candidate_weight(&x_candidates, "B"), 4.0);

    let y = must_ref(&mut pangine, "$['Y']");
    let y_candidates = named_relevance(&pangine, &y);
    assert_eq!(y_candidates[0].1, "D");
    assert_eq!(candidate_weight(&y_candidates, "D"), 12.0);
    assert_eq!(candidate_weight(&y_candidates, "A"), 4.0);
}

#[test]
fn question_preserves_outer_correlation_context_across_unordered_children() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {([C]*[A])->([B]*[D])}");
    let answer = must_ref(&mut pangine, "$['memory'] @ {(['X']*[A])->([B]*['Y'])}");
    assert_eq!(
        answer,
        must_ref(
            &mut pangine,
            "{[A]<x16[C], x8[A], [B], [D]>->[B]<x16[D], x8[B], [A], [C]>}",
        )
    );

    let x = must_ref(&mut pangine, "$['X']");
    let x_candidates = named_relevance(&pangine, &x);
    assert_eq!(candidate_weight(&x_candidates, "C"), 16.0);
    assert_eq!(candidate_weight(&x_candidates, "A"), 8.0);
    assert_eq!(candidate_weight(&x_candidates, "B"), 1.0);
    assert_eq!(candidate_weight(&x_candidates, "D"), 1.0);

    let y = must_ref(&mut pangine, "$['Y']");
    let y_candidates = named_relevance(&pangine, &y);
    assert_eq!(candidate_weight(&y_candidates, "D"), 16.0);
    assert_eq!(candidate_weight(&y_candidates, "B"), 8.0);
    assert_eq!(candidate_weight(&y_candidates, "A"), 1.0);
    assert_eq!(candidate_weight(&y_candidates, "C"), 1.0);
}

#[test]
fn evaluation_recursively_resolves_percepts_inside_a_shape() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['A'] = [resolved_a]");
    must_ref(&mut pangine, "['B'] = ['A']->[resolved_b]");

    assert_eq!(
        must_ref(&mut pangine, "$(?['B']:{[fixed]->['A']})"),
        must_ref(
            &mut pangine,
            "?({[resolved_a]->[resolved_b]}):({[fixed]->[resolved_a]})",
        )
    );
}

#[test]
fn question_folds_recursive_wildcard_projections_into_context_score() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->{[A]->[Z]}}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->{[D]->[Y]}}");
    ask_question(&mut pangine, "['memory'] @ {['X']->{[A]->[Z]}}");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    let c = candidates.iter().find(|(_, name)| name == "C").unwrap().0;
    let b = candidates.iter().find(|(_, name)| name == "B").unwrap().0;

    assert_eq!(c.weight(), 5.0);
    assert_eq!(b.weight(), 2.0);
}

#[test]
fn question_folds_multiple_output_wildcards_into_separate_marginals() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    ask_question(&mut pangine, "['memory'] @ {['X']->['Y']}");

    let x = must_ref(&mut pangine, "$['X']");
    assert_eq!(candidate_weight(&named_relevance(&pangine, &x), "C"), 2.0);

    let y = must_ref(&mut pangine, "$['Y']");
    assert_eq!(candidate_weight(&named_relevance(&pangine, &y), "A"), 2.0);
}

#[test]
fn generic_projection_rule_applies_to_dependencies_too() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= ?[C]:[A]");
    must_ref(&mut pangine, "['memory'] ~= ?[B]:[D]");
    ask_question(&mut pangine, "['memory'] @ ?['X']:[A]");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(candidate_weight(&candidates, "C"), 2.0);
    assert_eq!(candidate_weight(&candidates, "B"), 1.0);
}

#[test]
fn repeated_experience_scales_folded_projection_evidence() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->[D]}");
    ask_question(&mut pangine, "['memory'] @ {['X']->[A]}");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(candidate_weight(&candidates, "C"), 4.0);
    assert_eq!(candidate_weight(&candidates, "B"), 1.0);
}

#[test]
fn question_clears_an_output_when_no_projection_can_bind_it() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['X'] = [old]");
    must_ref(&mut pangine, "['memory'] ~= [A]");
    assert_eq!(
        must_ref(&mut pangine, "['memory'] @ {['X']->[B]}"),
        must_ref(&mut pangine, "{['X']->[B]}")
    );
    assert!(pangine.reference_concept("$['X']").unwrap().is_none());
}

#[test]
fn standalone_wildcard_question_can_bind_a_single_atomic_experience() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]");
    ask_question(&mut pangine, "['memory'] @ ['X']");

    assert_eq!(
        must_ref(&mut pangine, "$['X']"),
        must_ref(&mut pangine, "[A]")
    );
}

#[test]
fn experience_stores_linear_structure_instead_of_wildcard_closure() {
    let mut pangine = Pangine::new();
    let depth = 20;
    let mut experience = format!("[N{depth}]");
    for index in (0..depth).rev() {
        experience = format!("[N{index}]->({experience})");
    }

    let value = must_ref(&mut pangine, &format!("['memory'] ~= {experience}"));

    assert_eq!(pangine.get_relevance_map(&value).len(), depth * 2 + 1);
}

#[test]
fn wide_unordered_projection_uses_folded_matching() {
    let mut pangine = Pangine::new();
    let width = 20;
    let experience = (0..width)
        .map(|index| format!("{{[V{index}]->[K{index}]}}"))
        .collect::<Vec<_>>()
        .join("*");
    let question = (0..width)
        .map(|index| {
            if index == 0 {
                "{['X']->[K0]}".to_owned()
            } else {
                format!("{{[V{index}]->[K{index}]}}")
            }
        })
        .collect::<Vec<_>>()
        .join("*");

    must_ref(&mut pangine, &format!("['memory'] ~= {experience}"));
    ask_question(&mut pangine, &format!("['memory'] @ {question}"));

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(candidates[0].1, "V0");
    assert!(candidates
        .iter()
        .all(|(relevance, _)| relevance.weight().is_finite()));
}

fn candidate_weight(candidates: &[(Relevance, String)], name: &str) -> f32 {
    candidates
        .iter()
        .find(|(_, candidate)| candidate == name)
        .unwrap_or_else(|| panic!("missing candidate {name:?}"))
        .0
        .weight()
}

fn named_relevance(pangine: &Pangine, concept: &ConceptId) -> Vec<(Relevance, String)> {
    pangine
        .get_relevance_map(concept)
        .into_iter()
        .map(|(relevance, concept)| {
            let name = pangine
                .get_name(&concept)
                .unwrap_or_else(|| panic!("expected named candidate, got {concept:?}"));
            (relevance, name.to_owned())
        })
        .collect()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}

fn ask_question(pangine: &mut Pangine, input: &str) {
    must_ref(pangine, input);
}
