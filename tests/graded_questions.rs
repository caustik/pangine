use pangine::{CompletionGrade, ConceptId, Pangine};

const THREE_X_MEMORY: [&str; 7] =
    ["([E]->[A])([B]->[F])", "([E]->[A])([B]->[D])", "([C]->[A])([B]->[F])", "([C]->[A])([B]->[D])", "[C]->[A]", "[B]->[G]", "[H]->[A]"];

#[test]
fn a_graded_question_composes_parts_seen_separately_and_interpolates_their_probabilities() {
    let mut pangine = Pangine::new();
    for experience in THREE_X_MEMORY {
        must_ref(&mut pangine, &format!("{{m}} ~= {experience}"));
    }

    // `@` keeps the pairs observed whole.
    must_ref(&mut pangine, "{m} @ ({p}->[A])([B]->{q})");
    assert_eq!(must_ref(&mut pangine, "$({p}->{q})"), must_ref(&mut pangine, "([C]->[D])([C]->[F])([E]->[D])([E]->[F])"));

    // `@~` adds the pairs composed from separate experiences. Interpolation
    // keeps the observed pairs ahead, as the 3.x Percept Hierarchy note asked.
    must_ref(&mut pangine, "{m} @~ ({x}->[A])([B]->{y})");
    assert_eq!(
        readings(&mut pangine, "{x}->{y}"),
        vec![
            ("[C]->[D]".to_owned(), 9.0 / 40.0),
            ("[C]->[F]".to_owned(), 9.0 / 40.0),
            ("[E]->[D]".to_owned(), 23.0 / 120.0),
            ("[E]->[F]".to_owned(), 23.0 / 120.0),
            ("[C]->[G]".to_owned(), 1.0 / 20.0),
            ("[E]->[G]".to_owned(), 1.0 / 30.0),
            ("[H]->[D]".to_owned(), 1.0 / 30.0),
            ("[H]->[F]".to_owned(), 1.0 / 30.0),
            ("[H]->[G]".to_owned(), 1.0 / 60.0),
        ]
    );
    assert_eq!(readings(&mut pangine, "{x}"), vec![("[C]".to_owned(), 1.0 / 2.0), ("[E]".to_owned(), 4.0 / 9.0), ("[H]".to_owned(), 1.0 / 18.0)]);

    // `$` shows the probabilities as shares of their common denominator.
    assert_eq!(
        must_ref(&mut pangine, "$({x}->{y})"),
        must_ref(&mut pangine, "x27([C]->[D])x27([C]->[F])x23([E]->[D])x23([E]->[F])x6([C]->[G])x4([E]->[G])x4([H]->[D])x4([H]->[F])x2([H]->[G])")
    );
    assert_eq!(must_ref(&mut pangine, "${x}"), must_ref(&mut pangine, "x9[C]x8[E][H]"));
    assert_eq!(must_ref(&mut pangine, "^({x}->{y})"), must_ref(&mut pangine, "[C]->[D]"));
}

#[test]
fn the_removed_induction_now_prefers_the_case_observed_whole_and_labels_the_composed_one() {
    let mut pangine = Pangine::new();
    for experience in ["([E]->[A])([K]->[L])", "([E]->[A])([M]->[N])", "([E]->[A])([P]->[Q])", "([C]->[A])([B]->[D])"] {
        must_ref(&mut pangine, &format!("{{memory}} ~= {experience}"));
    }

    must_ref(&mut pangine, "{memory} @ ({exact}->[A])([B]->[D])");
    assert_eq!(must_ref(&mut pangine, "${exact}"), must_ref(&mut pangine, "[C]"));

    must_ref(&mut pangine, "{memory} @~ ({x}->[A])([B]->[D])");
    assert_eq!(readings(&mut pangine, "{x}"), vec![("[C]".to_owned(), 5.0 / 8.0), ("[E]".to_owned(), 3.0 / 8.0)]);

    let x = pangine.reference_percept("x");
    let view = pangine.answer_view(&x).expect("graded answer");
    let possibilities = view.possibilities(&mut pangine).expect("inspectable answer");
    let grades = |possibility: &pangine::AnswerPossibility| possibility.support().iter().map(|support| support.grade()).collect::<Vec<_>>();
    assert!(grades(&possibilities[0]).contains(&CompletionGrade::Exact), "C was observed whole");
    assert!(grades(&possibilities[1]).iter().all(|grade| *grade == CompletionGrade::Composed), "E is only composed");
}

#[test]
fn a_graded_question_with_only_exact_rows_answers_like_the_exact_question() {
    let mut pangine = Pangine::new();
    for (experience, repetitions) in [("[morning]->[birds]", 2), ("[morning]->[traffic]", 1)] {
        for _ in 0..repetitions {
            must_ref(&mut pangine, &format!("{{world}} ~= {experience}"));
        }
    }

    must_ref(&mut pangine, "{world} @ [morning]->{exact}");
    must_ref(&mut pangine, "{world} @~ [morning]->{graded}");
    assert_eq!(must_ref(&mut pangine, "${graded}"), must_ref(&mut pangine, "${exact}"));
    assert_eq!(readings(&mut pangine, "{graded}"), readings(&mut pangine, "{exact}"));
    assert_eq!(must_ref(&mut pangine, "^{graded}"), must_ref(&mut pangine, "[birds]"));
}

#[test]
fn a_graded_question_without_evidence_returns_nothing() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{world} ~= [morning]->[birds]");
    assert_eq!(pangine.reference_concept("{world} @~ [evening]->{answer}").unwrap(), None);
    assert_eq!(pangine.reference_concept("${answer}").unwrap(), None);
}

#[test]
fn completions_report_their_grades() {
    let mut pangine = Pangine::new();
    for experience in THREE_X_MEMORY {
        must_ref(&mut pangine, &format!("{{m}} ~= {experience}"));
    }
    let memory = pangine.reference_percept("m");
    let question = must_ref(&mut pangine, "({x}->[A])([B]->{y})");

    let exact = pangine.complete(&memory, &question).expect("exact completion");
    assert_eq!(exact.completions().len(), 4);
    assert!(exact.completions().iter().all(|completion| completion.grade() == CompletionGrade::Exact));

    let graded = pangine.complete_graded(&memory, &question).expect("graded completion");
    let composed = graded.completions().iter().filter(|completion| completion.grade() == CompletionGrade::Composed).count();
    assert_eq!(graded.completions().len() - composed, 4, "the graded result keeps every exact row");
    assert!(composed > 0);
    assert!(graded
        .completions()
        .iter()
        .filter(|completion| completion.grade() == CompletionGrade::Composed)
        .all(|completion| completion.evidence().len() == 2));
}

#[test]
fn a_graded_answer_crosses_an_engine_boundary_with_its_grades() {
    let mut pangine = Pangine::new();
    for experience in THREE_X_MEMORY {
        must_ref(&mut pangine, &format!("{{m}} ~= {experience}"));
    }
    must_ref(&mut pangine, "{m} @~ ({x}->[A])([B]->{y})");
    let x = pangine.reference_percept("x");
    let value = pangine.linked_answer_value(&x).expect("graded answer value");
    let spelling = pangine.format_concept(&value, false);

    let mut restored = Pangine::new();
    let transported = must_ref(&mut restored, &spelling);
    assert!(restored.install_answer_value(&transported));
    let restored_pairs = must_ref(&mut restored, "$({x}->{y})");
    let original_pairs = must_ref(&mut pangine, "$({x}->{y})");
    assert_eq!(restored.format_concept(&restored_pairs, false), pangine.format_concept(&original_pairs, false));
    assert_eq!(readings(&mut restored, "{x}"), readings(&mut pangine, "{x}"));
}

fn readings(pangine: &mut Pangine, projection: &str) -> Vec<(String, f64)> {
    let projection = must_ref(pangine, projection);
    let view = pangine.answer_view(&projection).expect("linked answer");
    let possibilities = view.possibilities(pangine).expect("inspectable answer");
    possibilities.iter().map(|possibility| (pangine.format_concept(possibility.value(), false), possibility.probability())).collect()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
