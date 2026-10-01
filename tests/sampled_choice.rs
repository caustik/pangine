use pangine::{ConceptId, Pangine};

const WORLD: [&str; 3] = ["[morning]->[birds]", "[morning]->[birds]", "[morning]->[traffic]"];

#[test]
fn a_draw_follows_each_value_share() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "world", &WORLD);
    must_ref(&mut pangine, "{world} @ [morning]->{answer}");

    // `sample` leaves the linked answer open, so every draw sees birds 2/3
    // and traffic 1/3.
    let draws = sample(&mut pangine, "{answer}", 6_000);
    assert_share(&draws, "[birds]", 2.0 / 3.0);
    assert_share(&draws, "[traffic]", 1.0 / 3.0);

    // A plain value draws by its members' counts.
    must_ref(&mut pangine, "{choice} = x2[tea]x3[coffee]");
    let draws = (0..6_000).map(|_| spell(&mut pangine, "^~{choice}")).collect::<Vec<_>>();
    assert_share(&draws, "[tea]", 2.0 / 5.0);
    assert_share(&draws, "[coffee]", 3.0 / 5.0);
}

#[test]
fn a_graded_answer_draws_by_its_interpolated_probabilities() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "closet", &["([top]->[red])([bottom]->[jeans])", "([top]->[blue])([bottom]->[skirt])", "[top]->[green]"]);
    must_ref(&mut pangine, "{closet} @~ ([top]->{shirt})([bottom]->{pants})");

    // Each outfit worn whole keeps 1/3, and each composed outfit gets 1/12.
    let draws = sample(&mut pangine, "{shirt}->{pants}", 6_000);
    for (outfit, probability) in [
        ("[red]->[jeans]", 1.0 / 3.0),
        ("[blue]->[skirt]", 1.0 / 3.0),
        ("[blue]->[jeans]", 1.0 / 12.0),
        ("[green]->[jeans]", 1.0 / 12.0),
        ("[green]->[skirt]", 1.0 / 12.0),
        ("[red]->[skirt]", 1.0 / 12.0),
    ] {
        assert_share(&draws, outfit, probability);
    }
}

#[test]
fn the_same_seed_draws_the_same_values() {
    let draws = |seed: Option<u64>, names_first: &str| {
        let mut pangine = Pangine::new();
        // Allocating the answer's names in another order must not change a
        // draw, because candidates line up by canonical spelling.
        let _names = must_ref(&mut pangine, names_first);
        if let Some(seed) = seed {
            pangine.set_sample_seed(seed);
        }
        remember(&mut pangine, "world", &WORLD);
        must_ref(&mut pangine, "{world} @ [morning]->{answer}");
        sample(&mut pangine, "{answer}", 32)
    };

    let default = draws(None, "[birds][traffic]");
    assert_eq!(default, draws(Some(0), "[birds][traffic]"), "a new engine starts from seed 0");
    assert_eq!(default, draws(None, "[traffic]->[birds]"));
    assert_eq!(draws(Some(42), "[birds][traffic]"), draws(Some(42), "[traffic]->[birds]"));
    assert_ne!(draws(Some(42), "[birds][traffic]"), default);
}

#[test]
fn a_draw_collapses_the_answer_like_choice() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "memory", &["[cat]->[fish]", "[cat]->[milk]", "[cat]->[milk]", "[dog]->[fish]", "[dog]->[fish]", "[dog]->[fish]"]);

    let mut drawn = Vec::new();
    for seed in 0..8 {
        pangine.set_sample_seed(seed);
        must_ref(&mut pangine, "{memory} @ {animal}->{food}");
        let before = rows(&mut pangine, "{animal}->{food}");
        let animal = spell(&mut pangine, "^~{animal}");

        // Only the drawn animal's complete rows remain, with their counts,
        // and every linked output reads from them.
        let expected = before.into_iter().filter(|(row, _)| row.starts_with(&format!("{animal}->"))).collect::<Vec<_>>();
        assert_eq!(rows(&mut pangine, "{animal}->{food}"), expected);
        assert_eq!(rows(&mut pangine, "{animal}").into_iter().map(|(value, _)| value).collect::<Vec<_>>(), [animal.as_str()]);
        drawn.push(animal);
    }

    // Cat and dog each have three units of evidence, so both are drawn.
    assert!(drawn.contains(&"[cat]".to_owned()) && drawn.contains(&"[dog]".to_owned()), "drawn: {drawn:?}");
}

#[test]
fn a_draw_abstains_without_positive_evidence() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{refused} = ![tea]");
    assert_eq!(pangine.reference_concept("^~{refused}").unwrap(), None);
    must_ref(&mut pangine, "{choice} = x2[tea]![coffee]");
    for _ in 0..20 {
        assert_eq!(spell(&mut pangine, "^~{choice}"), "[tea]", "a value without positive evidence is never drawn");
    }
    must_ref(&mut pangine, "{only} = [tea]");
    assert_eq!(spell(&mut pangine, "^~{only}"), "[tea]");

    remember(&mut pangine, "candidates", &["[A]"]);
    remember(&mut pangine, "failed", &["[A]", "[A]"]);
    must_ref(&mut pangine, "{candidates} @ {choice}");
    must_ref(&mut pangine, "{failed} @ {failed-choice}");
    pangine.reference_concept("{choice} @-= {failed-choice}").unwrap();
    assert_eq!(pangine.reference_concept("^~{choice}").unwrap(), None);
    assert_eq!(pangine.reference_concept("^{choice}").unwrap(), None);
}

#[test]
fn the_documented_draws_repeat_from_seed_zero() {
    // The README draws from a plain value, then restarts the sequence.
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{choice} = x2[tea]x3[coffee]");
    assert_eq!(spell(&mut pangine, "^{choice}"), "[coffee]");
    let draws = |pangine: &mut Pangine, count: usize| (0..count).map(|_| spell(pangine, "^~{choice}")).collect::<Vec<_>>();
    assert_eq!(draws(&mut pangine, 3), ["[coffee]", "[tea]", "[coffee]"]);
    pangine.set_sample_seed(0);
    assert_eq!(draws(&mut pangine, 2), ["[coffee]", "[tea]"]);

    // The examples page and its browser check draw from a linked answer.
    let mut pangine = Pangine::new();
    remember(&mut pangine, "world", &WORLD);
    must_ref(&mut pangine, "{world} @ [morning]->{answer}");
    assert_eq!(spell(&mut pangine, "^~{answer}"), "[birds]");
    must_ref(&mut pangine, "{world} @ [morning]->{answer}");
    assert_eq!(spell(&mut pangine, "^~{answer}"), "[traffic]");
    assert_eq!(spell(&mut pangine, "${answer}"), "[traffic]");
}

fn remember(pangine: &mut Pangine, percept: &str, experiences: &[&str]) {
    for experience in experiences {
        must_ref(pangine, &format!("{{{percept}}} ~= {experience}"));
    }
}

// Draws repeatedly without collapsing the linked answer.
fn sample(pangine: &mut Pangine, projection: &str, count: usize) -> Vec<String> {
    let projection = must_ref(pangine, projection);
    let view = pangine.answer_view(&projection).expect("linked answer");
    (0..count)
        .map(|_| {
            let choice = view.sample(pangine).expect("a draw");
            pangine.format_concept(choice.selected(), false)
        })
        .collect()
}

fn assert_share(draws: &[String], value: &str, probability: f64) {
    let share = draws.iter().filter(|draw| *draw == value).count() as f64 / draws.len() as f64;
    assert!((share - probability).abs() < 0.025, "{value} was drawn {share} of the time, expected about {probability}");
}

fn rows(pangine: &mut Pangine, projection: &str) -> Vec<(String, i64)> {
    let projection = must_ref(pangine, projection);
    let view = pangine.answer_view(&projection).expect("linked answer");
    let possibilities = view.possibilities(pangine).expect("inspectable answer");
    possibilities.iter().map(|possibility| (pangine.format_concept(possibility.value(), false), possibility.strength().count())).collect()
}

fn spell(pangine: &mut Pangine, input: &str) -> String {
    let concept = must_ref(pangine, input);
    pangine.format_concept(&concept, false)
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
