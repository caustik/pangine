use pangine::{Pangine, ParseError};

#[test]
fn null_concepts_and_invalid_syntax_have_distinct_results() {
    let mut pangine = Pangine::new();

    assert_eq!(pangine.reference_concept("[]").unwrap(), None);
    assert_eq!(pangine.parse_script_text("[A];[];").unwrap(), None);

    assert!(matches!(
        pangine.reference_concept("[A"),
        Err(ParseError::InvalidSyntax)
    ));
    assert!(matches!(
        pangine.parse_script_text("[A];[B"),
        Err(ParseError::InvalidSyntax)
    ));
    for script in ["!", "[A]*", "[A]/", "[A]->"] {
        assert!(
            matches!(
                pangine.reference_concept(script),
                Err(ParseError::InvalidSyntax)
            ),
            "expected invalid syntax: {script}"
        );
    }
}

#[test]
fn parse_details_distinguish_invalid_syntax_from_a_null_result() {
    let mut pangine = Pangine::new();
    let mut null_details = Vec::new();
    let mut error_details = Vec::new();

    assert_eq!(
        pangine
            .parse_script_text_with_details("[]", &mut null_details)
            .unwrap(),
        None
    );
    assert!(matches!(
        pangine.parse_script_text_with_details("[A", &mut error_details),
        Err(ParseError::InvalidSyntax)
    ));

    assert_eq!(
        String::from_utf8(null_details).unwrap(),
        "ps> []\nps=   []\n"
    );
    assert_eq!(
        String::from_utf8(error_details).unwrap(),
        "ps> [A\nps!   invalid Pangine syntax\n"
    );
}
