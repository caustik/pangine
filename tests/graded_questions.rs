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
    for experience in ["[E]->[A]", "[E]->[A]", "[E]->[A]", "([C]->[A])([B]->[D])"] {
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
fn similar_cases_enter_below_the_exact_and_composed_answers() {
    let mut pangine = Pangine::new();
    for experience in ["([E]->[A])([K]->[L])", "([E]->[A])([M]->[N])", "([E]->[A])([P]->[Q])", "([C]->[A])([B]->[D])"] {
        must_ref(&mut pangine, &format!("{{memory}} ~= {experience}"));
    }

    // Each partial experience also matches the question's shape with `[B]->[D]`
    // renamed, so E gains support at distance 2 below its composed support.
    must_ref(&mut pangine, "{memory} @~ ({x}->[A])([B]->[D])");
    assert_eq!(readings(&mut pangine, "{x}"), vec![("[C]".to_owned(), 17.0 / 28.0), ("[E]".to_owned(), 11.0 / 28.0)]);
    let grades = support_grades(&mut pangine, "{x}");
    assert_eq!(grades[0], vec![CompletionGrade::Exact]);
    assert!(grades[1].contains(&CompletionGrade::Composed) && grades[1].contains(&CompletionGrade::Generalized { distance: 2 }));
}

#[test]
fn a_remembered_case_answers_a_new_one_in_its_own_terms() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{families} ~= ([Tom]->[parent-of]->[Bob])([Bob]->[parent-of]->[Ann])([Tom]->[grandparent-of]->[Ann])");
    must_ref(&mut pangine, "{families} ~= ([Liz]->[parent-of]->[Max])([Max]->[parent-of]->[Ivy])([Liz]->[grandparent-of]->[Ivy])");
    let question = "([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim])([Joe]->[grandparent-of]->{who})";

    assert_eq!(pangine.reference_concept(&format!("{{families}} @ {question}")).unwrap(), None);
    let completed = must_ref(&mut pangine, &format!("{{families}} @~ {question}"));
    assert_eq!(completed, must_ref(&mut pangine, "x2(([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim])([Joe]->[grandparent-of]->[Kim]))"));

    // Each family keeps its people in the question's positions at distance 5.
    // Splitting the question's repeated names would cost more, so no family
    // answers with its own grandchild.
    assert_eq!(readings(&mut pangine, "{who}"), vec![("[Kim]".to_owned(), 1.0)]);
    assert_eq!(support_grades(&mut pangine, "{who}"), vec![vec![CompletionGrade::Generalized { distance: 5 }, CompletionGrade::Generalized { distance: 5 }]]);
}

#[test]
fn a_nearby_board_shares_its_move_below_the_exact_game() {
    let mut pangine = Pangine::new();
    for game in ["[x]->[_]->[o]->[c2]", "[x]->[_]->[_]->[c3]", "[x]->[_]->[_]->[c3]"] {
        must_ref(&mut pangine, &format!("{{games}} ~= {game}"));
    }

    must_ref(&mut pangine, "{games} @~ [x]->[_]->[o]->{move}");
    assert_eq!(readings(&mut pangine, "{move}"), vec![("[c2]".to_owned(), 2.0 / 3.0), ("[c3]".to_owned(), 1.0 / 3.0)]);
    assert_eq!(support_grades(&mut pangine, "{move}"), vec![vec![CompletionGrade::Exact], vec![CompletionGrade::Generalized { distance: 1 }]]);
    assert_eq!(must_ref(&mut pangine, "${move}"), must_ref(&mut pangine, "x2[c2][c3]"));
}

#[test]
fn the_2x_xor_note_answers_by_analogy() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{notes} ~= ((([0]->[pair])->[1])->[xor])->[0]");
    must_ref(&mut pangine, "{notes} @~ ((([1]->[pair])->[0])->[xor])->{answer}");
    assert_eq!(must_ref(&mut pangine, "${answer}"), must_ref(&mut pangine, "[1]"));
    assert_eq!(support_grades(&mut pangine, "{answer}"), vec![vec![CompletionGrade::Generalized { distance: 2 }]]);
}

#[test]
fn a_case_must_share_a_name_and_supply_each_part_from_its_own_part() {
    let mut pangine = Pangine::new();
    // Nothing in this memory shares a name with the question.
    must_ref(&mut pangine, "{memory} ~= [c]->[d]->[e]");
    assert_eq!(pangine.reference_concept("{memory} @~ [a]->[b]->{x}").unwrap(), None);

    // One remembered relationship cannot answer both parts of a question.
    must_ref(&mut pangine, "{closet} ~= [top]->[green]");
    assert_eq!(pangine.reference_concept("{closet} @~ ([top]->{shirt})([bottom]->{pants})").unwrap(), None);
}

#[test]
fn a_graded_answer_with_many_levels_still_answers() {
    // One exact case and one case at each distance from 1 to 28. Exact shares
    // would need a 68-bit denominator, more than `$` can store as evidence
    // counts, so it shows millionths.
    let mut pangine = Pangine::new();
    let positions = (1..=29).map(|position| format!("[p{position}]")).collect::<Vec<_>>();
    for distance in 0..=28 {
        let cells = positions.iter().enumerate().map(|(index, name)| if index < distance { format!("[q{}]", index + 1) } else { name.clone() });
        must_ref(&mut pangine, &format!("{{memory}} ~= {}->[a{distance}]", cells.collect::<Vec<_>>().join("->")));
    }
    let question = format!("{{memory}} @~ {}->{{x}}", positions.join("->"));
    must_ref(&mut pangine, &question);

    let millionths = [693_147, 193_147, 68_147, 26_481, 10_856, 4_606, 2_001, 885, 397, 180, 82, 38, 18, 8, 4, 2, 1];
    let expected = millionths.iter().enumerate().map(|(index, share)| format!("x{share}[a{index}]")).collect::<String>();
    assert_eq!(must_ref(&mut pangine, "${x}"), must_ref(&mut pangine, &expected));
    let readings = readings(&mut pangine, "{x}");
    assert_eq!(readings.len(), 29);
    assert_eq!(readings[0].0, "[a0]");
    assert!((readings[0].1 - 0.693_147_180_563_974_2).abs() < 1e-15);
    assert!(pangine.reference_concept("^~{x}").unwrap().is_some());

    must_ref(&mut pangine, &question);
    assert_eq!(must_ref(&mut pangine, "^{x}"), must_ref(&mut pangine, "[a0]"));
}

#[test]
fn an_adjustment_keeps_the_grade_of_the_evidence_it_imports() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{options} ~= [hall]->[north]->[left]");
    must_ref(&mut pangine, "{options} ~= [hall]->[north]->[right]");
    must_ref(&mut pangine, "{trips} ~= [lobby]->[north]->[left]");
    must_ref(&mut pangine, "{options} @ [hall]->[north]->{way}");
    must_ref(&mut pangine, "{trips} @~ [hall]->[north]->{trip-way}");

    // The trip through the lobby supports left at distance 1, so it enters
    // below the hall's own options instead of counting as one of them.
    assert_eq!(must_ref(&mut pangine, "{way} @+= {trip-way}"), must_ref(&mut pangine, "x7[left]x5[right]"));
    assert_eq!(readings(&mut pangine, "{way}"), vec![("[left]".to_owned(), 7.0 / 12.0), ("[right]".to_owned(), 5.0 / 12.0)]);
    assert_eq!(
        support_grades(&mut pangine, "{way}"),
        vec![vec![CompletionGrade::Exact, CompletionGrade::Generalized { distance: 1 }], vec![CompletionGrade::Exact]]
    );

    // The imported grade crosses an engine boundary with the answer.
    let way = pangine.reference_percept("way");
    let value = pangine.linked_answer_value(&way).expect("adjusted answer value");
    let spelling = pangine.format_concept(&value, false);
    let mut restored = Pangine::new();
    let transported = must_ref(&mut restored, &spelling);
    assert!(restored.install_answer_value(&transported));
    assert_eq!(readings(&mut restored, "{way}"), readings(&mut pangine, "{way}"));
    assert_eq!(support_grades(&mut restored, "{way}"), support_grades(&mut pangine, "{way}"));

    // Exact counterevidence still applies at the exact level.
    must_ref(&mut pangine, "{dead-ends} ~= [hall]->[north]->[right]");
    must_ref(&mut pangine, "{dead-ends} @ [hall]->[north]->{dead-end}");
    must_ref(&mut pangine, "{way} @-= {dead-end}");
    assert_eq!(readings(&mut pangine, "{way}"), vec![("[left]".to_owned(), 1.0), ("[right]".to_owned(), 0.0)]);

    // A graded answer itself cannot be adjusted: each of one value's rows at
    // different grades would take the same evidence.
    assert!(pangine.reference_concept("{trip-way} @+= {way}").is_err());
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
    must_ref(&mut pangine, "{families} ~= ([Tom]->[parent-of]->[Bob])([Bob]->[parent-of]->[Ann])([Tom]->[grandparent-of]->[Ann])");
    must_ref(&mut pangine, "{families} @~ ([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim])([Joe]->[grandparent-of]->{who})");
    let who = pangine.reference_percept("who");
    let generalized = pangine.linked_answer_value(&who).expect("generalized answer value");
    let generalized_spelling = pangine.format_concept(&generalized, false);
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

    let transported = must_ref(&mut restored, &generalized_spelling);
    assert!(restored.install_answer_value(&transported));
    assert_eq!(support_grades(&mut restored, "{who}"), support_grades(&mut pangine, "{who}"));
    assert_eq!(must_ref(&mut restored, "${who}"), must_ref(&mut restored, "[Kim]"));
}

fn support_grades(pangine: &mut Pangine, projection: &str) -> Vec<Vec<CompletionGrade>> {
    let projection = must_ref(pangine, projection);
    let view = pangine.answer_view(&projection).expect("linked answer");
    let possibilities = view.possibilities(pangine).expect("inspectable answer");
    possibilities.iter().map(|possibility| possibility.support().iter().map(|support| support.grade()).collect()).collect()
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
