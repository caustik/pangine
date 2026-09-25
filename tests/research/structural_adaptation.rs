//! Structural transformation-by-example comparison.
//!
//! A test-local anti-unifier produces Pangine templates from complete
//! before/after pairs. A small typed-term baseline receives the same examples.
//! Both are deliberately conservative and cover only syntactic structure; this
//! is a purpose probe, not proposed production induction semantics.

use pangine::{ConceptId, ConceptKind, Pangine};
use std::collections::BTreeMap;

type Example = (&'static str, &'static str);

struct Case {
    name: &'static str,
    left: Example,
    right: Example,
    input: &'static str,
    expected: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case {
        name: "carry and insert fixed structure",
        left: ("[project]->[alpha]->[format]->[v1]", "[project]->[alpha]->[format]->[v2]->[migrated-from]->[v1]"),
        right: ("[project]->[beta]->[format]->[v1]", "[project]->[beta]->[format]->[v2]->[migrated-from]->[v1]"),
        input: "[project]->[gamma]->[format]->[v1]",
        expected: Some("[project]->[gamma]->[format]->[v2]->[migrated-from]->[v1]"),
    },
    Case {
        name: "reorder two values",
        left: ("[pair]->[A]->[B]", "[pair]->[B]->[A]"),
        right: ("[pair]->[C]->[D]", "[pair]->[D]->[C]"),
        input: "[pair]->[E]->[F]",
        expected: Some("[pair]->[F]->[E]"),
    },
    Case {
        name: "delete one varying value",
        left: ("[record]->[A]->[discard-a]->[tail]", "[record]->[A]->[tail]"),
        right: ("[record]->[B]->[discard-b]->[tail]", "[record]->[B]->[tail]"),
        input: "[record]->[C]->[discard-c]->[tail]",
        expected: Some("[record]->[C]->[tail]"),
    },
    Case {
        name: "copy a nested subtree",
        left: ("[container]->([name]->[A])->[state]->[old]", "[container]->([name]->[A])->[state]->[new]->[copied]->([name]->[A])"),
        right: ("[container]->([name]->[B])->[state]->[old]", "[container]->([name]->[B])->[state]->[new]->[copied]->([name]->[B])"),
        input: "[container]->([name]->[C])->[state]->[old]",
        expected: Some("[container]->([name]->[C])->[state]->[new]->[copied]->([name]->[C])"),
    },
    Case {
        name: "reject an output-only difference",
        left: ("[project]->[alpha]->[format]->[v1]", "[project]->[alpha]->[format]->[v2]"),
        right: ("[project]->[beta]->[format]->[v1]", "[project]->[renamed-beta]->[format]->[v2]"),
        input: "[project]->[gamma]->[format]->[v1]",
        expected: None,
    },
    Case {
        name: "reject unequal source shapes",
        left: ("[pair]->[A]", "[out]->[A]"),
        right: ("[pair]->[B]->[C]", "[out]->[B]->[C]"),
        input: "[pair]->[D]",
        expected: None,
    },
    Case {
        name: "reject unordered values without an alignment rule",
        left: ("([name]->[A])([state]->[old])", "([name]->[A])([state]->[new])"),
        right: ("([name]->[B])([state]->[old])", "([name]->[B])([state]->[new])"),
        input: "([name]->[C])([state]->[old])",
        expected: None,
    },
];

#[test]
#[ignore = "warning: syntactic anti-unification is test-local comparison code, not Pangine semantics"]
fn concepts_and_typed_terms_transfer_and_abstain_on_the_same_matrix() {
    let mut pangine = Pangine::new();

    for case in CASES {
        let left = parse_example(&mut pangine, case.left);
        let right = parse_example(&mut pangine, case.right);
        let input = must_ref(&mut pangine, case.input);
        let expected = case.expected.map(|text| must_ref(&mut pangine, text));

        let induced = induce_concept_transformation(&mut pangine, (&left.0, &left.1), (&right.0, &right.1));
        let concept_result = apply_concept_transformation(&mut pangine, &induced, &input).map(|application| application.output);
        assert_eq!(concept_result, expected, "Pangine adapter: {}", case.name);

        let typed_left = typed_example(&pangine, &left);
        let typed_right = typed_example(&pangine, &right);
        let typed = induce_typed_transformation(typed_left.clone(), typed_right.clone());
        let typed_input = TypedTerm::from_concept(&pangine, &input).expect("typed input");
        let typed_expected = expected.as_ref().map(|value| TypedTerm::from_concept(&pangine, value).expect("typed expected output"));
        let typed_result = apply_typed_transformation(&typed, &typed_input);
        assert_eq!(typed_result.as_ref().map(|application| &application.output), typed_expected.as_ref(), "typed baseline: {}", case.name);
        assert_eq!(typed.sources, [typed_left, typed_right]);
    }
}

#[test]
#[ignore = "warning: induced-template provenance and ranking remain application state"]
fn a_novel_output_uses_pangine_matching_but_not_answer_provenance() {
    let mut pangine = Pangine::new();
    let left = parse_example(&mut pangine, CASES[0].left);
    let right = parse_example(&mut pangine, CASES[0].right);
    let induced = induce_concept_transformation(&mut pangine, (&left.0, &left.1), (&right.0, &right.1));

    assert_eq!(pangine.format_concept(&induced.input, false), "[project]->{induced-0}->[format]->[v1]");
    assert_eq!(pangine.format_concept(&induced.output, false), "[project]->{induced-0}->[format]->[v2]->[migrated-from]->[v1]");
    assert_eq!(induced.sources, [left.clone(), right.clone()], "training provenance is adapter state");

    let input = must_ref(&mut pangine, CASES[0].input);
    let application = apply_concept_transformation(&mut pangine, &induced, &input).expect("bound output");
    assert_eq!(application.output, must_ref(&mut pangine, CASES[0].expected.unwrap()));
    assert_eq!(application.bindings, BTreeMap::from([(pangine.reference_percept("induced-0"), must_ref(&mut pangine, "[gamma]"))]));
    assert!(application.match_evidence.iter().all(|source| source == &input));
    assert!(!application.match_evidence.contains(&left.0));
    assert!(!application.match_evidence.contains(&right.0));
}

struct ConceptTransformation {
    input: ConceptId,
    output: ConceptId,
    sources: [(ConceptId, ConceptId); 2],
}

struct ConceptApplication {
    output: ConceptId,
    bindings: BTreeMap<ConceptId, ConceptId>,
    match_evidence: Vec<ConceptId>,
}

fn induce_concept_transformation(pangine: &mut Pangine, left: (&ConceptId, &ConceptId), right: (&ConceptId, &ConceptId)) -> ConceptTransformation {
    let mut substitutions = BTreeMap::new();
    let input = anti_unify_concepts(pangine, left.0, right.0, &mut substitutions);
    let output = anti_unify_concepts(pangine, left.1, right.1, &mut substitutions);
    ConceptTransformation { input, output, sources: [(left.0.clone(), left.1.clone()), (right.0.clone(), right.1.clone())] }
}

fn apply_concept_transformation(pangine: &mut Pangine, transformation: &ConceptTransformation, input: &ConceptId) -> Option<ConceptApplication> {
    let matched = pangine.complete_subject(input, &transformation.input)?;
    let [completion] = matched.completions() else {
        return None;
    };
    let output = pangine.instantiate_completion(&transformation.output, completion)?;
    let bindings = completion.bindings().map(|(percept, value)| (percept.clone(), value.clone())).collect();
    let match_evidence = completion.evidence().iter().map(|evidence| evidence.source_concept().clone()).collect();
    Some(ConceptApplication { output, bindings, match_evidence })
}

fn anti_unify_concepts(
    pangine: &mut Pangine,
    left: &ConceptId,
    right: &ConceptId,
    substitutions: &mut BTreeMap<(ConceptId, ConceptId), ConceptId>,
) -> ConceptId {
    if left == right {
        return left.clone();
    }
    if let (Some(left), Some(right)) = (pangine.get_ordered_components(left), pangine.get_ordered_components(right)) {
        if left.len() == right.len() {
            let components = left.iter().zip(&right).map(|(left, right)| anti_unify_concepts(pangine, left, right, substitutions)).collect::<Vec<_>>();
            return pangine.compose_ordered(&components).expect("owned generalized components").expect("nonempty generalized sequence");
        }
    }
    let next = substitutions.len();
    substitutions.entry((left.clone(), right.clone())).or_insert_with(|| pangine.reference_percept(&format!("induced-{next}"))).clone()
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TypedTerm {
    Atom(String),
    Ordered(Vec<TypedTerm>),
    Unordered(Vec<TypedTerm>),
    Hole(usize),
}

impl TypedTerm {
    fn from_concept(pangine: &Pangine, concept: &ConceptId) -> Option<Self> {
        match pangine.concept_kind(concept)? {
            ConceptKind::Named(name) => Some(Self::Atom(name.clone())),
            ConceptKind::Ordered { components } => {
                Some(Self::Ordered(components.iter().map(|component| Self::from_concept(pangine, component)).collect::<Option<_>>()?))
            }
            ConceptKind::Unordered => Some(Self::Unordered(
                pangine.get_relevance_map(concept).into_iter().map(|(_, member)| Self::from_concept(pangine, &member)).collect::<Option<_>>()?,
            )),
            ConceptKind::Percept { .. } => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TypedTransformation {
    input: TypedTerm,
    output: TypedTerm,
    sources: [(TypedTerm, TypedTerm); 2],
}

struct TypedApplication {
    output: TypedTerm,
    _bindings: BTreeMap<usize, TypedTerm>,
}

fn induce_typed_transformation(left: (TypedTerm, TypedTerm), right: (TypedTerm, TypedTerm)) -> TypedTransformation {
    let mut substitutions = BTreeMap::new();
    let input = anti_unify_terms(&left.0, &right.0, &mut substitutions);
    let output = anti_unify_terms(&left.1, &right.1, &mut substitutions);
    TypedTransformation { input, output, sources: [left, right] }
}

fn apply_typed_transformation(transformation: &TypedTransformation, input: &TypedTerm) -> Option<TypedApplication> {
    let mut bindings = BTreeMap::new();
    bind_term(&transformation.input, input, &mut bindings)?;
    let output = instantiate_term(&transformation.output, &bindings)?;
    Some(TypedApplication { output, _bindings: bindings })
}

fn anti_unify_terms(left: &TypedTerm, right: &TypedTerm, substitutions: &mut BTreeMap<(TypedTerm, TypedTerm), usize>) -> TypedTerm {
    if left == right {
        return left.clone();
    }
    if let (TypedTerm::Ordered(left), TypedTerm::Ordered(right)) = (left, right) {
        if left.len() == right.len() {
            return TypedTerm::Ordered(left.iter().zip(right).map(|(left, right)| anti_unify_terms(left, right, substitutions)).collect());
        }
    }
    let next = substitutions.len();
    TypedTerm::Hole(*substitutions.entry((left.clone(), right.clone())).or_insert(next))
}

fn bind_term(pattern: &TypedTerm, input: &TypedTerm, bindings: &mut BTreeMap<usize, TypedTerm>) -> Option<()> {
    match (pattern, input) {
        (TypedTerm::Hole(hole), input) => match bindings.get(hole) {
            Some(bound) if bound != input => None,
            Some(_) => Some(()),
            None => {
                bindings.insert(*hole, input.clone());
                Some(())
            }
        },
        (TypedTerm::Atom(pattern), TypedTerm::Atom(input)) if pattern == input => Some(()),
        (TypedTerm::Ordered(pattern), TypedTerm::Ordered(input)) if pattern.len() == input.len() => {
            pattern.iter().zip(input).try_for_each(|(pattern, input)| bind_term(pattern, input, bindings))
        }
        (TypedTerm::Unordered(pattern), TypedTerm::Unordered(input)) if pattern == input => Some(()),
        _ => None,
    }
}

fn instantiate_term(template: &TypedTerm, bindings: &BTreeMap<usize, TypedTerm>) -> Option<TypedTerm> {
    match template {
        TypedTerm::Hole(hole) => bindings.get(hole).cloned(),
        TypedTerm::Atom(_) => Some(template.clone()),
        TypedTerm::Ordered(components) => {
            Some(TypedTerm::Ordered(components.iter().map(|component| instantiate_term(component, bindings)).collect::<Option<_>>()?))
        }
        TypedTerm::Unordered(members) => Some(TypedTerm::Unordered(members.iter().map(|member| instantiate_term(member, bindings)).collect::<Option<_>>()?)),
    }
}

fn parse_example(pangine: &mut Pangine, example: Example) -> (ConceptId, ConceptId) {
    (must_ref(pangine, example.0), must_ref(pangine, example.1))
}

fn typed_example(pangine: &Pangine, example: &(ConceptId, ConceptId)) -> (TypedTerm, TypedTerm) {
    (TypedTerm::from_concept(pangine, &example.0).expect("typed example input"), TypedTerm::from_concept(pangine, &example.1).expect("typed example output"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
