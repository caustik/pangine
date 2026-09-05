use pangine::Pangine;

#[test]
fn question_results_and_projections_follow_literal_coefficient_composition() {
    for (subject, expected, row_count) in [
        ("x2[A]", "x3[A]", 2),
        ("![A]", "[]", 2),
        ("x2{input}", "x3{input}", 2),
        ("!{input}", "[]", 2),
        ("x2([fact]->[cat]->[eats])", "x3([fact]->[cat]->[eats])[cat][eats][fact]", 5),
    ] {
        let mut pangine = Pangine::new();
        let result = pangine.reference_concept(&format!("{subject} @ {{output}}")).expect("structural question");
        let projection = pangine.reference_concept("${output}").expect("linked projection");
        let expected_value = pangine.reference_concept(expected).expect("ordinary coefficient composition");
        assert_eq!(result, expected_value, "completed rows for {subject}");
        assert_eq!(projection, expected_value, "projection for {subject}");

        let output = pangine.reference_percept("output");
        let answer = pangine.answer_snapshot(&output).expect("the complete answer survives an empty projection");
        assert_eq!(answer.result().completions().len(), row_count, "the source and its inner members remain distinct complete possibilities");
        for value in [result, projection] {
            let spelling = value.as_ref().map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(value, false));
            assert_eq!(pangine.reference_concept(&spelling).expect("formatted value parses"), value, "{subject}: {spelling}");
        }

        let encoded = pangine.linked_answer_value(&output).expect("retained complete answer");
        let transport = pangine.format_concept(&encoded, false);
        let mut restored = Pangine::new();
        let transported = restored.reference_concept(&transport).expect("encoded answer parses").expect("nonempty answer value");
        assert!(restored.install_answer_value(&transported));
        let restored_output = restored.reference_percept("output");
        let restored_answer = restored.answer_snapshot(&restored_output).expect("restored complete answer");
        assert_eq!(restored_answer.result().completions().len(), row_count);
        assert_eq!(restored.get_value(&restored_output), restored.reference_concept(expected).expect("restored ordinary value"));
    }
}

#[test]
fn evaluating_coefficient_values_uses_the_same_union_as_literal_composition() {
    for (input, expression, expected) in
        [("x2[A]", "$({input}[B])", "x2[A][B]"), ("![A]", "$({input}[A])", "[]"), ("x2([A][B])", "$({input}([A][B]))", "x3([A][B])")]
    {
        let mut pangine = Pangine::new();
        pangine.reference_concept(&format!("{{input}} = {input}")).expect("assigned input");
        let result = pangine.reference_concept(expression).expect("evaluation");
        assert_eq!(result, pangine.reference_concept(expected).expect("literal composition"), "{expression} with {input}");
        let spelling = result.as_ref().map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(value, false));
        assert_eq!(pangine.reference_concept(&spelling).expect("formatted result parses"), result);
    }
}

#[test]
fn nested_results_remain_parseable_values_across_composition_and_evaluation() {
    for base in ["[A]", "{input}", "[A][B]", "[A]->[B]", "([key]->[A])([value]->{input})"] {
        for source_text in [
            base.to_owned(),
            format!("x2({base})"),
            format!("!({base})"),
            format!("({base})(x2({base}))"),
            format!("[outer]->({base})->[end]"),
            format!("([left]->({base}))([right]->x2({base}))"),
            format!("x3(([nested]->({base}))([other]->[B]))"),
        ] {
            let mut pangine = Pangine::new();
            pangine.reference_concept("{input} = [resolved]").expect("current input");
            let source = pangine.reference_concept(&source_text).expect("source expression");
            let rows = pangine.reference_concept(&format!("({source_text}) @ {{output}}")).expect("question expression");
            let projected = pangine.reference_concept("${output}").expect("projection");
            let evaluated = source.as_ref().and_then(|source| pangine.evaluate_concept(source));

            for (operation, value) in [("source", source), ("question", rows), ("projection", projected), ("evaluation", evaluated)] {
                let spelling = value.as_ref().map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(value, false));
                assert_eq!(pangine.reference_concept(&spelling).expect("formatted value parses"), value, "{operation} on {source_text}: {spelling}");
            }
        }
    }
}
