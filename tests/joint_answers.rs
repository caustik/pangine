use pangine::{ConceptId, Pangine};

#[test]
fn question_outputs_project_one_shared_weighted_answer() {
    let mut pangine = weighted_animals();

    let rows = ask(&mut pangine);
    assert_eq!(rows, must_ref(&mut pangine, "{[cat]->[fish]}{[cat]->[milk]}{[dog]->[fish]}"));
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x8[cat]x7[dog]"));
    assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "x10[fish]x5[milk]"));

    let joint = must_ref(&mut pangine, "$(['animal']->['food'])");
    assert_eq!(joint, must_ref(&mut pangine, "x3{[cat]->[fish]}x5{[cat]->[milk]}x7{[dog]->[fish]}"));
    assert_eq!(must_ref(&mut pangine, "$(['animal']->['food'])"), joint, "projection must not change the answer state");
}

#[test]
fn linked_answer_operator_reveals_and_composes_with_the_shared_output_group() {
    let mut pangine = weighted_animals();
    ask(&mut pangine);

    let linked = must_ref(&mut pangine, "&['animal']");
    assert_eq!(linked, must_ref(&mut pangine, "['animal']->['food']"));
    assert_eq!(must_ref(&mut pangine, "&['food']"), linked);
    assert_eq!(must_ref(&mut pangine, "$(&['animal'])"), must_ref(&mut pangine, "x3{[cat]->[fish]}x5{[cat]->[milk]}x7{[dog]->[fish]}"));

    assert_eq!(must_ref(&mut pangine, "^(&['animal'])"), must_ref(&mut pangine, "{[dog]->[fish]}"));
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x7[dog]"));
    assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "x7[fish]"));
    assert_null(&mut pangine, "&['memory']");
}

#[test]
fn choosing_one_output_conditions_and_reprojects_the_others() {
    let mut pangine = weighted_animals();
    ask(&mut pangine);

    assert_eq!(must_ref(&mut pangine, "^['animal']"), must_ref(&mut pangine, "[cat]"));
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x8[cat]"));
    assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "x3[fish]x5[milk]"));
    assert_eq!(must_ref(&mut pangine, "$(['animal']->['food'])"), must_ref(&mut pangine, "x3{[cat]->[fish]}x5{[cat]->[milk]}"));

    assert_eq!(must_ref(&mut pangine, "^['food']"), must_ref(&mut pangine, "[milk]"));
    assert_eq!(must_ref(&mut pangine, "$(['animal']->['food'])"), must_ref(&mut pangine, "x5{[cat]->[milk]}"));
}

#[test]
fn collapse_order_and_simultaneous_collapse_can_choose_different_rows() {
    let mut animal_first = weighted_animals();
    ask(&mut animal_first);
    assert_eq!(must_ref(&mut animal_first, "^['animal']"), must_ref(&mut animal_first, "[cat]"));
    assert_eq!(must_ref(&mut animal_first, "^['food']"), must_ref(&mut animal_first, "[milk]"));

    let mut food_first = weighted_animals();
    ask(&mut food_first);
    assert_eq!(must_ref(&mut food_first, "^['food']"), must_ref(&mut food_first, "[fish]"));
    assert_eq!(must_ref(&mut food_first, "^['animal']"), must_ref(&mut food_first, "[dog]"));

    let mut together = weighted_animals();
    ask(&mut together);
    assert_eq!(must_ref(&mut together, "^(['animal']->['food'])"), must_ref(&mut together, "{[dog]->[fish]}"));
    assert_eq!(must_ref(&mut together, "$['animal']"), must_ref(&mut together, "x7[dog]"));
    assert_eq!(must_ref(&mut together, "$['food']"), must_ref(&mut together, "x7[fish]"));
}

#[test]
fn asking_again_for_a_complete_output_group_starts_a_new_answer_cycle() {
    let mut pangine = weighted_animals();
    ask(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "^['animal']"), must_ref(&mut pangine, "[cat]"));

    ask(&mut pangine);
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x8[cat]x7[dog]"));
    assert_eq!(must_ref(&mut pangine, "&['animal']"), must_ref(&mut pangine, "['animal']->['food']"));
}

#[test]
fn choosing_a_subset_reprojects_the_unselected_output() {
    let mut pangine = Pangine::new();
    experience_row(&mut pangine, "[Alice]->[cat]->[fish]", 4);
    experience_row(&mut pangine, "[Bob]->[cat]->[fish]", 3);
    experience_row(&mut pangine, "[Carol]->[dog]->[bone]", 6);
    must_ref(&mut pangine, "['memory'] @ ['person']->['animal']->['food']");

    assert_eq!(must_ref(&mut pangine, "^(['animal']->['food'])"), must_ref(&mut pangine, "{[cat]->[fish]}"));
    assert_eq!(must_ref(&mut pangine, "$['person']"), must_ref(&mut pangine, "x4[Alice]x3[Bob]"));
    assert_eq!(must_ref(&mut pangine, "^['person']"), must_ref(&mut pangine, "[Alice]"));
    assert_eq!(must_ref(&mut pangine, "$(['person']->['animal']->['food'])"), must_ref(&mut pangine, "x4{[Alice]->[cat]->[fish]}"));
}

#[test]
fn assigning_one_output_detaches_it_from_later_collapse() {
    let mut pangine = weighted_animals();
    ask(&mut pangine);

    must_ref(&mut pangine, "['animal'] = [bird]");
    assert_eq!(must_ref(&mut pangine, "^['food']"), must_ref(&mut pangine, "[fish]"));
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "[bird]"));
    assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "x10[fish]"));
    assert_null(&mut pangine, "&['animal']");
    assert_eq!(must_ref(&mut pangine, "&['food']"), must_ref(&mut pangine, "['food']"));
}

#[test]
fn detaching_one_output_hides_only_the_answer_shapes_that_contain_it() {
    let mut pangine = Pangine::new();
    experience_in(&mut pangine, "meals", "[cat]->[eats]->[fish]", 1);
    experience_in(&mut pangine, "meals", "[dog]->[eats]->[bone]", 1);
    experience_in(&mut pangine, "homes", "[cat]->[lives-in]->[house]", 1);
    experience_in(&mut pangine, "homes", "[dog]->[lives-in]->[yard]", 1);
    must_ref(&mut pangine, "['meals'] @ ['animal']->[eats]->['food']");
    must_ref(&mut pangine, "['homes'] @ ['animal']->[lives-in]->['home']");

    must_ref(&mut pangine, "['food'] = [detached]");
    assert_eq!(must_ref(&mut pangine, "&['animal']"), must_ref(&mut pangine, "['animal']->[lives-in]->['home']"));
    assert_null(&mut pangine, "&['food']");
}

#[test]
fn reusing_a_linked_output_extends_one_shared_answer() {
    let mut pangine = Pangine::new();
    experience_in(&mut pangine, "meals", "[cat]->[eats]->[fish]", 1);
    experience_in(&mut pangine, "meals", "[dog]->[eats]->[bone]", 1);
    experience_in(&mut pangine, "homes", "[cat]->[lives-in]->[house]", 1);
    experience_in(&mut pangine, "homes", "[dog]->[lives-in]->[yard]", 1);

    must_ref(&mut pangine, "['meals'] @ ['animal']->[eats]->['food']");
    let extension = must_ref(&mut pangine, "['homes'] @ ['animal']->[lives-in]->['home']");
    assert_eq!(extension, must_ref(&mut pangine, "(([cat]->[eats]->[fish])([cat]->[lives-in]->[house]))(([dog]->[eats]->[bone])([dog]->[lives-in]->[yard]))"));
    assert_eq!(must_ref(&mut pangine, "&['food']"), must_ref(&mut pangine, "(['animal']->[eats]->['food'])(['animal']->[lives-in]->['home'])"));
    assert_eq!(
        must_ref(&mut pangine, "$(&['animal'])"),
        must_ref(&mut pangine, "x2(([cat]->[eats]->[fish])([cat]->[lives-in]->[house]))x2(([dog]->[eats]->[bone])([dog]->[lives-in]->[yard]))")
    );

    assert_eq!(must_ref(&mut pangine, "^['animal']"), must_ref(&mut pangine, "[cat]"));
    assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "[fish]"));
    assert_eq!(must_ref(&mut pangine, "$['home']"), must_ref(&mut pangine, "[house]"));
}

#[test]
fn incompatible_extension_leaves_every_existing_value_and_answer_untouched() {
    let mut pangine = Pangine::new();
    experience_in(&mut pangine, "meals", "[cat]->[eats]->[fish]", 1);
    experience_in(&mut pangine, "homes", "[bird]->[lives-in]->[nest]", 1);
    must_ref(&mut pangine, "['meals'] @ ['animal']->[eats]->['food']");
    must_ref(&mut pangine, "['home'] = [old-home]");
    let linked_before = must_ref(&mut pangine, "&['animal']");
    let rows_before = must_ref(&mut pangine, "$(&['animal'])");

    assert_null(&mut pangine, "['homes'] @ ['animal']->[lives-in]->['home']");
    assert_eq!(must_ref(&mut pangine, "&['animal']"), linked_before);
    assert_eq!(must_ref(&mut pangine, "$(&['animal'])"), rows_before);
    assert_eq!(must_ref(&mut pangine, "$['home']"), must_ref(&mut pangine, "[old-home]"));
    assert_null(&mut pangine, "&['home']");
}

#[test]
fn a_detached_output_does_not_constrain_a_later_extension_through_its_linked_sibling() {
    let mut pangine = Pangine::new();
    experience_in(&mut pangine, "meals", "[cat]->[eats]->[fish]", 1);
    experience_in(&mut pangine, "meals", "[dog]->[eats]->[bone]", 1);
    experience_in(&mut pangine, "owners", "[fish]->[belongs-to]->[bird]", 1);
    experience_in(&mut pangine, "owners", "[bone]->[belongs-to]->[wolf]", 1);
    must_ref(&mut pangine, "['meals'] @ ['animal']->[eats]->['food']");
    must_ref(&mut pangine, "['animal'] = [detached]");

    let extension = must_ref(&mut pangine, "['owners'] @ ['food']->[belongs-to]->['animal']");
    assert_eq!(extension, must_ref(&mut pangine, "{[bone]->[belongs-to]->[wolf]}{[fish]->[belongs-to]->[bird]}"));
    assert_eq!(must_ref(&mut pangine, "&['food']"), must_ref(&mut pangine, "['food']->[belongs-to]->['animal']"));
    assert_eq!(must_ref(&mut pangine, "$(['food']->['animal'])"), must_ref(&mut pangine, "x2{[bone]->[wolf]}x2{[fish]->[bird]}"));
}

#[test]
fn one_question_can_combine_two_existing_shared_answers() {
    let mut pangine = Pangine::new();
    experience_in(&mut pangine, "meals", "[cat]->[eats]->[fish]", 1);
    experience_in(&mut pangine, "meals", "[dog]->[eats]->[bone]", 1);
    experience_in(&mut pangine, "weather", "[house]->[has-weather]->[warm]", 1);
    experience_in(&mut pangine, "weather", "[yard]->[has-weather]->[cold]", 1);
    experience_in(&mut pangine, "homes", "[cat]->[lives-in]->[house]", 1);
    experience_in(&mut pangine, "homes", "[dog]->[lives-in]->[yard]", 1);

    must_ref(&mut pangine, "['meals'] @ ['animal']->[eats]->['food']");
    must_ref(&mut pangine, "['weather'] @ ['home']->[has-weather]->['weather']");
    must_ref(&mut pangine, "['homes'] @ ['animal']->[lives-in]->['home']");

    assert_eq!(
        must_ref(&mut pangine, "&['weather']"),
        must_ref(&mut pangine, "(['animal']->[eats]->['food'])(['home']->[has-weather]->['weather'])(['animal']->[lives-in]->['home'])")
    );
    assert_eq!(
        must_ref(&mut pangine, "$(&['animal'])"),
        must_ref(
            &mut pangine,
            "x3(([cat]->[eats]->[fish])([house]->[has-weather]->[warm])([cat]->[lives-in]->[house]))x3(([dog]->[eats]->[bone])([yard]->[has-weather]->[cold])([dog]->[lives-in]->[yard]))"
        )
    );
}

#[test]
fn sequential_extension_keeps_equal_values_at_different_source_positions_separate() {
    let mut pangine = Pangine::new();
    let source = "[A]->[r]->[M]->[s]->[D]->[gap]->[X]->[r]->[M]->[s]->[E]";
    must_ref(&mut pangine, &format!("{source} @ ['start']->[r]->['middle']"));
    must_ref(&mut pangine, &format!("{source} @ ['middle']->[s]->['end']"));

    assert_eq!(must_ref(&mut pangine, "$(['start']->['middle']->['end'])"), must_ref(&mut pangine, "{[A]->[M]->[D]}{[X]->[M]->[E]}"));
}

#[test]
fn separate_questions_keep_separate_answer_states() {
    let mut pangine = weighted_animals();
    ask(&mut pangine);
    must_ref(&mut pangine, "['other'] ~= [red]->[round]");
    must_ref(&mut pangine, "['other'] ~= [blue]->[square]");
    must_ref(&mut pangine, "['other'] @ ['color']->['shape']");

    let shapes = must_ref(&mut pangine, "$['shape']");
    assert_eq!(must_ref(&mut pangine, "^['animal']"), must_ref(&mut pangine, "[cat]"));
    assert_eq!(must_ref(&mut pangine, "$['shape']"), shapes);
}

fn weighted_animals() -> Pangine {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "[cat]->[fish]", 3);
    experience(&mut pangine, "[cat]->[milk]", 5);
    experience(&mut pangine, "[dog]->[fish]", 7);
    pangine
}

fn experience(pangine: &mut Pangine, row: &str, amount: usize) {
    experience_row(pangine, row, amount);
}

fn experience_row(pangine: &mut Pangine, row: &str, amount: usize) {
    experience_in(pangine, "memory", row, amount);
}

fn experience_in(pangine: &mut Pangine, percept: &str, row: &str, amount: usize) {
    for _ in 0..amount {
        must_ref(pangine, &format!("['{percept}'] ~= {row}"));
    }
}

fn ask(pangine: &mut Pangine) -> ConceptId {
    must_ref(pangine, "['memory'] @ ['animal']->['food']")
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}

fn assert_null(pangine: &mut Pangine, input: &str) {
    assert!(pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}")).is_none());
}
