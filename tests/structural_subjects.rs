use pangine::{ConceptId, Pangine};

#[test]
fn direct_and_retained_subjects_preserve_references_at_every_nested_level() {
    let mut pangine = Pangine::new();
    let subject_slot = pangine.reference_percept("subject");
    let variable = pangine.reference_percept("variable");
    let captured = pangine.reference_percept("captured");
    let current = must_ref(&mut pangine, "[current]");
    let mut subject = variable.clone();
    let mut question = captured.clone();

    // The same source value can be questioned directly or retained as one
    // experience. Embedding it again does not change reference identity.
    for depth in 0..8 {
        assert!(pangine.set_percept_value(&variable, (depth % 2 == 0).then(|| current.clone())));
        assert!(pangine.set_percept_value(&subject_slot, Some(subject.clone())));
        let direct = pangine.complete_subject(&subject, &question).expect("owned structural subject");
        let retained = pangine.complete_question(std::slice::from_ref(&subject_slot), &question).expect("owned retained source");
        let [direct_row] = direct.completions() else {
            panic!("one direct completion at depth {depth}");
        };
        let [retained_row] = retained.completions() else {
            panic!("one retained completion at depth {depth}");
        };
        assert_eq!(direct_row.binding(&captured), Some(&variable));
        assert_eq!(direct_row.bindings().collect::<Vec<_>>(), retained_row.bindings().collect::<Vec<_>>());
        assert!(direct_row.evidence().iter().all(|evidence| evidence.source_percept().is_none() && evidence.source_concept() == &subject));
        assert!(retained_row.evidence().iter().all(|evidence| evidence.source_percept() == Some(&subject_slot) && evidence.source_concept() == &subject));

        let marker = pangine.reference_name(&format!("level-{depth}"));
        subject = pangine.compose_ordered(&[marker.clone(), subject]).expect("owned composition").expect("nested subject");
        question = pangine.compose_ordered(&[marker, question]).expect("owned composition").expect("nested question");
    }
}

#[test]
fn question_shapes_can_be_questioned_again_without_a_storage_step() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{memory} ~= [cat]->[purrs]");
    must_ref(&mut pangine, "{memory} @ {animal}->{sound}");
    let animal = pangine.reference_percept("animal");
    let original = pangine.linked_answer_value(&animal).expect("original answer");

    must_ref(&mut pangine, "(&{animal}) @ {left}->{right}");
    assert_eq!(must_ref(&mut pangine, "${left}"), animal.clone());
    assert_eq!(must_ref(&mut pangine, "${right}"), pangine.reference_percept("sound"));
    must_ref(&mut pangine, "($({left}->{right})) @ {left-again}->{right-again}");
    assert_eq!(must_ref(&mut pangine, "${left-again}"), animal.clone());
    assert_eq!(pangine.linked_answer_value(&animal), Some(original));
    assert_eq!(must_ref(&mut pangine, "${animal}"), must_ref(&mut pangine, "[cat]"));
}

#[test]
fn literal_and_evaluated_subjects_require_only_the_existing_evaluation_operation() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{input} = [first]");
    must_ref(&mut pangine, "([field]->{input}) @ [field]->{literal}");
    must_ref(&mut pangine, "($([field]->{input})) @ [field]->{evaluated}");
    assert_eq!(must_ref(&mut pangine, "${literal}"), pangine.reference_percept("input"));
    assert_eq!(must_ref(&mut pangine, "${evaluated}"), must_ref(&mut pangine, "[first]"));

    must_ref(&mut pangine, "{input} = [later]");
    assert_eq!(must_ref(&mut pangine, "${literal}"), pangine.reference_percept("input"));
    assert_eq!(must_ref(&mut pangine, "${evaluated}"), must_ref(&mut pangine, "[first]"));
    assert_eq!(must_ref(&mut pangine, "$${literal}"), must_ref(&mut pangine, "[later]"));
}

#[test]
fn foreign_structural_subjects_and_questions_are_still_rejected() {
    let mut pangine = Pangine::new();
    let mut foreign = Pangine::new();
    let subject = must_ref(&mut pangine, "[field]->{input}");
    let question = must_ref(&mut pangine, "[field]->{output}");
    let foreign_subject = must_ref(&mut foreign, "[field]->{input}");
    let foreign_question = must_ref(&mut foreign, "[field]->{output}");
    assert!(pangine.complete_subject(&foreign_subject, &question).is_none());
    assert!(pangine.complete_subject(&subject, &foreign_question).is_none());
    assert!(pangine.complete_selector(&foreign_subject, &question).is_none());
    assert!(pangine.complete_selector(&subject, &foreign_question).is_none());
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}")).unwrap_or_else(|| panic!("{input:?} was empty"))
}
