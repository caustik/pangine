//! Warning check for the current payload transport boundary.
//!
//! Parser-safe names and digit sequences round-trip as opaque named Concepts.
//! Paths, JSON-shaped text, non-ASCII text, delimiter-bearing text, and an
//! explicit absent field value do not yet have a lossless syntax form. This
//! fixture records that gap without selecting escaping, scalar types, or null
//! semantics.

use pangine::{ConceptId, Pangine};

#[test]
#[ignore = "warning: parser-safe atoms round trip, while arbitrary text and explicit absent field values still need a representation"]
fn parser_safe_atoms_round_trip_while_external_payloads_remain_unrepresentable() {
    let mut source = Pangine::new();

    for payload in ["opaque value 42", "1700000000000"] {
        let concept = must_ref(&mut source, &format!("[{payload}]"));
        let formatted = source.format_concept(&concept, false);
        let mut destination = Pangine::new();
        let reparsed = must_ref(&mut destination, &formatted);

        assert_eq!(destination.get_name(&reparsed), Some(payload));
        assert_eq!(destination.format_concept(&reparsed, false), formatted);
    }

    for payload in [r"C:\Library\Track 01.wav", r#"{"revision":2}"#, "cafe\u{301}", "left]right"] {
        let input = format!("[{payload}]");
        assert!(source.reference_concept(&input).is_err(), "unexpected lossless syntax value for {payload:?}");
    }

    assert_eq!(source.reference_concept("[]").expect("null syntax"), None);
    assert!(source.reference_concept("[field]->[]").is_err(), "null cannot currently occupy an ordered field value");
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
