//! Compares whole-value structural completion with descendant discovery.
//! The helper starts the existing matcher at the supplied value's root;
//! it adds no matching rules, public syntax, or source contribution policy.

use super::{
    routes_with_binding_origins, source_route_constraints, Completion, CompletionEvidence, CompletionEvidenceParts, CompletionResult, CompletionRoute,
    ConceptId, Pangine, QuestionSource,
};
use std::collections::BTreeSet;

#[path = "staged_inquiry.rs"]
mod staged_inquiry;

#[test]
#[ignore = "warning: whole-value completion binds each existing Concept kind without searching or evaluating the supplied value"]
fn whole_value_completion_keeps_names_references_structures_and_answers_intact() {
    let mut pangine = animals();
    must_ref(&mut pangine, "{input} = [current]");
    let input = pangine.reference_percept("input");
    let animal = pangine.reference_percept("animal");
    let original = pangine.linked_answer_value(&animal).expect("original Answer");
    let output = pangine.reference_percept("whole");
    let mut examples = ["[name]", "[\"\"]", "{input}", "[A]->[B]", "[A][B]", "x2([A]->[B])", "![A]", "&{animal}"]
        .into_iter()
        .map(|text| must_ref(&mut pangine, text))
        .collect::<Vec<_>>();
    examples.push(original.clone());

    for mut value in examples {
        for depth in 0..=4 {
            let result = complete_whole(&mut pangine, &value, &output);
            assert_eq!(result.completions().len(), 1);
            assert_eq!(result.completions()[0].binding(&output), Some(&value));
            assert_eq!(pangine.instantiate_completion(&output, &result.completions()[0]), Some(value.clone()));
            assert_eq!(result.completions()[0].evidence()[0].source_concept(), &value);
            assert!(result.completions()[0].evidence()[0].remainders().next().is_none());
            let label = pangine.reference_name(&format!("layer-{depth}"));
            value = pangine.compose_ordered(&[label, value]).expect("owned value").expect("nonempty wrapper");
        }
    }
    assert_eq!(pangine.linked_answer_value(&animal), Some(original));
    assert_eq!(pangine.get_value(&input), Some(must_ref(&mut pangine, "[current]")));
    assert_eq!(pangine.get_value(&output), None, "the comparison does not publish its binding");
}

#[test]
#[ignore = "warning: whole-value matching removes the repeated-anchor problem while record cardinality remains a separate boundary"]
fn whole_value_matching_controls_depth_without_giving_a_label_special_meaning() {
    let mut pangine = Pangine::new();
    let subject = must_ref(&mut pangine, "[here]->(([source]->[outer])([nested]->([here]->(([source]->[inner])([other]->[field])))))");
    let question = must_ref(&mut pangine, "[here]->(([source]->{source}){rest})");
    let whole = complete_whole(&mut pangine, &subject, &question);
    let recursive = pangine.complete_subject(&subject, &question).expect("recursive question");
    let output = pangine.reference_percept("source");
    assert_eq!(bound_values(&whole, &output), BTreeSet::from([must_ref(&mut pangine, "[outer]")]));
    assert_eq!(bound_values(&recursive, &output), BTreeSet::from([must_ref(&mut pangine, "[outer]"), must_ref(&mut pangine, "[inner]")]));

    let singleton = must_ref(&mut pangine, "[here]->([source]->[only])");
    assert!(complete_whole(&mut pangine, &singleton, &question).completions().is_empty());
    let exact_shape = must_ref(&mut pangine, "[here]->([source]->{source})");
    assert_eq!(bound_values(&complete_whole(&mut pangine, &singleton, &exact_shape), &output), BTreeSet::from([must_ref(&mut pangine, "[only]")]));
}

#[test]
#[ignore = "warning: explicit whole-value context composes through the existing Answer join for compound, reference, and Answer data"]
fn whole_inputs_compose_with_complete_alternatives_without_searching_their_contents() {
    let mut pangine = animals();
    must_ref(&mut pangine, "{settings} = [must-not-be-followed]");
    let animal = pangine.reference_percept("animal");
    let sound = pangine.reference_percept("sound");
    let context = pangine.reference_percept("context");
    let original = pangine.linked_answer_value(&animal).expect("original Answer value");
    let answer = pangine.answer_snapshot(&animal).expect("original Answer");
    let projection = must_ref(&mut pangine, "{animal}->{sound}->{context}");
    let mut inputs = ["[current]", "[current]->[quiet]", "[current][quiet]", "{settings}", "[input]->([input]->[nested])"]
        .into_iter()
        .map(|text| must_ref(&mut pangine, text))
        .collect::<Vec<_>>();
    inputs.push(original.clone());

    for value in inputs {
        let input = complete_whole(&mut pangine, &value, &context);
        let combined = pangine.join_completion_results(
            answer.result(),
            &BTreeSet::from([animal.clone(), sound.clone()]),
            &input,
            &BTreeSet::from([context.clone()]),
            &projection,
        );
        assert_eq!(combined.completions().len(), 2);
        for row in combined.completions() {
            assert_eq!(row.binding(&context), Some(&value));
            let expected = pangine
                .compose_ordered(&[row.binding(&animal).unwrap().clone(), row.binding(&sound).unwrap().clone(), value.clone()])
                .expect("owned row")
                .expect("nonempty row");
            assert_eq!(pangine.instantiate_completion(&projection, row), Some(expected));
            assert_eq!(row.evidence().len(), 2);
            assert!(row.evidence().iter().any(|fragment| fragment.source_concept() == &value));
        }
        assert_eq!(pangine.linked_answer_value(&animal), Some(original.clone()));
    }
}

#[test]
#[ignore = "warning: local completion still admits question-supplied context; root scope alone does not establish satisfaction"]
fn whole_value_scope_does_not_turn_completion_into_a_proof_of_supplied_properties() {
    let mut pangine = Pangine::new();
    let source = must_ref(&mut pangine, "[item]->([red][round])");
    let question = must_ref(&mut pangine, "{object}->([red][round][heavy])");
    let answer = complete_whole(&mut pangine, &source, &question);
    assert_eq!(answer.completions().len(), 1);
    let row = &answer.completions()[0];
    assert_eq!(pangine.instantiate_completion(&question, row), Some(must_ref(&mut pangine, "[item]->([red][round][heavy])")));
    let remainder = row.evidence()[0].remainders().next().expect("question-supplied context");
    assert!(remainder.side() == super::CompletionRemainderSide::Question);
    assert_eq!(remainder.concept(), &must_ref(&mut pangine, "[heavy]"));
}

#[test]
#[ignore = "warning: the whole question is one structural pattern here, including an unordered collection of relationships"]
fn whole_record_matching_keeps_its_fields_at_the_stated_structural_level() {
    let mut pangine = Pangine::new();
    let subject = must_ref(&mut pangine, "([left]->[A])([right]->[B])([nested]->(([left]->[B])([right]->[A])))");
    let question = must_ref(&mut pangine, "([left]->{left})([right]->{right})");
    let whole = complete_whole(&mut pangine, &subject, &question);
    let recursive = pangine.complete_subject(&subject, &question).expect("recursive field question");
    let left = pangine.reference_percept("left");
    let right = pangine.reference_percept("right");
    assert_eq!(whole.completions().len(), 1);
    assert_eq!(whole.completions()[0].binding(&left), Some(&must_ref(&mut pangine, "[A]")));
    assert_eq!(whole.completions()[0].binding(&right), Some(&must_ref(&mut pangine, "[B]")));
    assert_eq!(whole.completions()[0].evidence().len(), 1);
    assert_eq!(bound_values(&recursive, &left), BTreeSet::from([must_ref(&mut pangine, "[A]"), must_ref(&mut pangine, "[B]")]));
    assert_eq!(bound_values(&recursive, &right), BTreeSet::from([must_ref(&mut pangine, "[A]"), must_ref(&mut pangine, "[B]")]));
}

#[test]
#[ignore = "warning: the whole-value result uses the existing Answer encoding without establishing a public operation"]
fn whole_value_answers_use_the_existing_transport_and_instantiation_paths() {
    use super::super::concept_answer::ConceptAnswer;

    for (source, question) in [
        ("[input]->{reference}", "{whole}"),
        ("[item]->([red][round])", "{object}->([red][round][heavy])"),
        ("([left]->[A])([right]->[B])([other]->[C])", "([left]->{left})([right]->{right})"),
    ] {
        let mut pangine = Pangine::new();
        let source = must_ref(&mut pangine, source);
        let question = must_ref(&mut pangine, question);
        let result = complete_whole(&mut pangine, &source, &question);
        let answer = ConceptAnswer::from_result(&pangine, &result);
        let encoded = answer.encode(&mut pangine);
        let text = pangine.format_concept(&encoded, false);
        let mut other = Pangine::new();
        let copied = must_ref(&mut other, &text);
        let decoded = ConceptAnswer::decode(&other, &copied).expect("ordinary Answer codec");
        let restored = decoded.to_result(&mut other).expect("restored result");
        let encoded_again = decoded.encode(&mut other);
        assert_eq!(other.format_concept(&encoded_again, false), text);
        assert_eq!(restored.completions().len(), result.completions().len());
        assert_eq!(formatted_rows(&mut pangine, &result), formatted_rows(&mut other, &restored));
    }
}

#[test]
#[ignore = "warning: matching direct union members handles absent, singleton, and larger field collections without an empty remainder"]
fn direct_member_inspection_handles_record_cardinality_without_filler_fields() {
    let mut pangine = Pangine::new();
    let question = must_ref(&mut pangine, "[source]->{source}");
    let output = pangine.reference_percept("source");
    for source in ["[]", "[source]->[outer]", "([source]->[outer])([result]->[done])", "([source]->[outer])([result]->[done])([nested]->([source]->[inner]))"] {
        let subject = pangine.reference_concept(source).expect("represented record");
        let result = complete_members(&mut pangine, subject.as_ref(), &question);
        let expected = subject.as_ref().map(|_| must_ref(&mut pangine, "[outer]")).into_iter().collect::<BTreeSet<_>>();
        assert_eq!(bound_values(&result, &output), expected);
        if let Some(subject) = subject {
            assert!(result.completions().iter().flat_map(|row| row.evidence()).all(|fragment| fragment.source_concept() == &subject));
        }
    }
}

#[test]
#[ignore = "warning: direct members are a union view, preserve coefficient wrappers, and do not evaluate a Percept"]
fn direct_members_preserve_the_existing_value_and_coefficient_boundaries() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{reference} = [current]");
    let reference = pangine.reference_percept("reference");
    let output = pangine.reference_percept("member");
    assert_eq!(bound_values(&complete_members(&mut pangine, Some(&reference), &output), &output), BTreeSet::from([reference.clone()]));

    let subject = must_ref(&mut pangine, "x2([source]->[weighted])([source]->[plain])([nested]->([source]->[inner]))");
    let question = must_ref(&mut pangine, "[source]->{member}");
    let ordinary = complete_members(&mut pangine, Some(&subject), &question);
    assert_eq!(bound_values(&ordinary, &output), BTreeSet::from([must_ref(&mut pangine, "[plain]")]));
    let question = must_ref(&mut pangine, "x2([source]->{member})");
    let weighted = complete_members(&mut pangine, Some(&subject), &question);
    assert_eq!(bound_values(&weighted, &output), BTreeSet::from([must_ref(&mut pangine, "[weighted]")]));
    assert_eq!(weighted.completions()[0].evidence()[0].source_concept(), &subject);
    assert_eq!(pangine.get_value(&reference), Some(must_ref(&mut pangine, "[current]")));
}

// Test-only whole-value completion: match the entire question at the root and
// use the same default source route as a normal direct source's root view.
// No wrapper label, new Concept kind, or source contribution is introduced.
fn complete_whole(pangine: &mut Pangine, subject: &ConceptId, question: &ConceptId) -> CompletionResult {
    let source = QuestionSource::from_subject(subject.clone());
    let root_routes = BTreeSet::from([CompletionRoute::default()]);
    complete_view(pangine, &source, subject, question, &root_routes)
}

// This uses the existing union decomposition, which treats other Concept kinds
// as single members. Coefficients remain part of the matched member. The
// complete containing source is retained; members do not become new sources.
fn complete_members(pangine: &mut Pangine, subject: Option<&ConceptId>, question: &ConceptId) -> CompletionResult {
    let Some(subject) = subject else {
        return CompletionResult::from_parts(question.clone(), Vec::new());
    };
    let source = QuestionSource::from_subject(subject.clone());
    let mut rows = Vec::new();
    for (amount, member) in pangine.relevance_entries(subject).expect("owned value") {
        let matched = pangine.compose_union(&[(amount, member)]).expect("owned member").expect("nonempty member");
        let mut route = CompletionRoute::default();
        if matched != *subject && Pangine::is_grouped_entry(&matched) {
            route.selected_entries.insert(subject.clone(), matched.clone());
        }
        let result = complete_view(pangine, &source, &matched, question, &BTreeSet::from([route]));
        rows.extend(result.completions);
    }
    CompletionResult::from_parts(question.clone(), rows)
}

fn complete_view(
    pangine: &mut Pangine,
    source: &QuestionSource,
    matched: &ConceptId,
    question: &ConceptId,
    source_routes: &BTreeSet<CompletionRoute>,
) -> CompletionResult {
    let mut rows = Vec::new();
    for completion in pangine.source_view_completions(matched, question) {
        let routes = routes_with_binding_origins(source_routes, &completion.binding_paths);
        let evidence = CompletionEvidence::from_parts(CompletionEvidenceParts {
            contribution: source.relevance,
            source: source.clone(),
            clause: question.clone(),
            matched: matched.clone(),
            source_route_products: source_route_constraints(&routes, &BTreeSet::new()),
            routes,
            assignment: completion.assignment.clone(),
            remainders: completion.remainders,
            adjusted_outputs: BTreeSet::new(),
        });
        rows.push(Completion::from_parts(completion.assignment, vec![evidence]));
    }
    CompletionResult::from_parts(question.clone(), rows)
}

fn formatted_rows(pangine: &mut Pangine, result: &CompletionResult) -> BTreeSet<String> {
    result
        .completions()
        .iter()
        .map(|row| {
            let value = pangine.instantiate_completion(result.question(), row).expect("complete result");
            pangine.format_concept(&value, false)
        })
        .collect()
}

fn bound_values(result: &CompletionResult, output: &ConceptId) -> BTreeSet<ConceptId> {
    result.completions().iter().filter_map(|row| row.binding(output).cloned()).collect()
}

fn animals() -> Pangine {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "{memory} ~= [cat]->[purrs]");
    must_ref(&mut pangine, "{memory} ~= [dog]->[barks]");
    must_ref(&mut pangine, "{memory} @ {animal}->{sound}");
    pangine
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected a nonempty Concept for {input:?}"))
}
