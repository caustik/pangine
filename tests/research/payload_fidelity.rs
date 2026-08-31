//! Research check for lossless external text and structural field omission.
//!
//! Escaped syntax and the Rust API create the same opaque named Concepts.
//! Paths, JSON-shaped text, Unicode, delimiters, controls, and empty text all
//! round-trip without gaining application meaning. An absent optional field is
//! represented by omitting its relationship. `[]` remains no Concept and does
//! not become a field value.

use pangine::{ConceptId, Pangine};
use std::collections::BTreeSet;

#[test]
#[ignore = "research detail: external text is lossless while absent fields remain structural omissions rather than [] values"]
fn escaped_text_round_trips_while_absent_fields_remain_structural() {
    let mut source = Pangine::new();

    for payload in ["opaque value 42", "1700000000000", r"C:\Library\Track 01.wav", r#"{"revision":2}"#, "cafe\u{301}", "left]right", "", "line one\nline two"]
    {
        let concept = source.reference_name(payload);
        let formatted = source.format_concept(&concept, false);
        let mut destination = Pangine::new();
        let reparsed = must_ref(&mut destination, &formatted);

        assert_eq!(destination.get_name(&reparsed), Some(payload));
        assert_eq!(destination.format_concept(&reparsed, false), formatted);
    }

    let records = source.reference_percept("records");
    let unranked = must_ref(&mut source, r#"([row-unranked]->[payload]->["C:\\Library\\Track 01.wav"])"#);
    let ranked = must_ref(&mut source, r#"([row-ranked]->[payload]->["{\"revision\":2}"])([row-ranked]->[rank]->[five])"#);
    source.perform_experience(&records, Some(&unranked)).expect("unranked record");
    source.perform_experience(&records, Some(&ranked)).expect("ranked record");

    let row = source.reference_percept("row");
    let payload = source.reference_percept("payload-value");
    let required_question = must_ref(&mut source, "{row}->[payload]->{payload-value}");
    let required = source.complete_question(std::slice::from_ref(&records), &required_question).expect("valid required-field question");
    let required_rows = required
        .completions()
        .iter()
        .map(|completion| (bound_name(&source, completion.binding(&row), "row"), bound_name(&source, completion.binding(&payload), "payload")))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        required_rows,
        BTreeSet::from([("row-ranked".to_owned(), r#"{"revision":2}"#.to_owned()), ("row-unranked".to_owned(), r"C:\Library\Track 01.wav".to_owned()),])
    );

    let rank = source.reference_percept("rank-value");
    let rank_question = must_ref(&mut source, "{row}->[rank]->{rank-value}");
    let ranked_only = source.complete_question(std::slice::from_ref(&records), &rank_question).expect("valid optional-field question");
    assert_eq!(ranked_only.completions().len(), 1);
    assert_eq!(bound_name(&source, ranked_only.completions()[0].binding(&row), "ranked row"), "row-ranked");
    assert_eq!(bound_name(&source, ranked_only.completions()[0].binding(&rank), "rank"), "five");

    assert_eq!(source.reference_concept("[]").expect("null syntax"), None);
    assert!(source.reference_concept("[rank]->[]").is_err(), "no Concept cannot occupy an ordered field value");
}

fn bound_name(pangine: &Pangine, value: Option<&ConceptId>, field: &str) -> String {
    pangine.get_name(value.unwrap_or_else(|| panic!("missing {field} binding"))).unwrap_or_else(|| panic!("unnamed {field} binding")).to_owned()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
