#![allow(dead_code, reason = "each integration test crate uses a different subset of the shared harness")]

use pangine::{ConceptId, Pangine, ParseError, Relevance};

macro_rules! pairs {
    ($($left:expr => $right:expr),+ $(,)?) => {
        [$(($left, $right)),+]
    };
}

pub(crate) use pairs;

#[derive(Default)]
pub struct PangineTest {
    pangine: Pangine,
}

impl PangineTest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn engine(&self) -> &Pangine {
        &self.pangine
    }

    pub fn engine_mut(&mut self) -> &mut Pangine {
        &mut self.pangine
    }

    #[track_caller]
    pub fn reference(&mut self, source: &str) -> Option<ConceptId> {
        self.pangine.reference_concept(source).unwrap_or_else(|error| panic!("failed to parse {source:?}: {error}"))
    }

    #[track_caller]
    pub fn concept(&mut self, source: &str) -> ConceptId {
        self.reference(source).unwrap_or_else(|| panic!("expected {source:?} to produce a concept"))
    }

    #[track_caller]
    pub fn exec<'a>(&mut self, sources: impl IntoIterator<Item = &'a str>) {
        for source in sources {
            self.concept(source);
        }
    }

    #[track_caller]
    pub fn assert_equivalent<'a>(&mut self, cases: impl IntoIterator<Item = (&'a str, &'a str)>) {
        for (index, (left_source, right_source)) in cases.into_iter().enumerate() {
            let left = self.reference(left_source);
            let right = self.reference(right_source);
            let left_formatted = self.format(left.as_ref());
            let right_formatted = self.format(right.as_ref());
            assert_eq!(
                left, right,
                "equivalence case {index} failed\n  left:  {left_source} => {left_formatted}\n  right: {right_source} => {right_formatted}"
            );
        }
    }

    #[track_caller]
    pub fn assert_distinct<'a>(&mut self, cases: impl IntoIterator<Item = (&'a str, &'a str)>) {
        for (index, (left_source, right_source)) in cases.into_iter().enumerate() {
            let left = self.reference(left_source);
            let right = self.reference(right_source);
            let left_formatted = self.format(left.as_ref());
            let right_formatted = self.format(right.as_ref());
            assert_ne!(
                left, right,
                "distinction case {index} failed\n  left:  {left_source} => {left_formatted}\n  right: {right_source} => {right_formatted}"
            );
        }
    }

    #[track_caller]
    pub fn assert_formats<'a>(&mut self, cases: impl IntoIterator<Item = (&'a str, &'a str)>) {
        for (index, (source, expected)) in cases.into_iter().enumerate() {
            let concept = self.reference(source);
            let actual = self.format(concept.as_ref());
            assert_eq!(actual, expected, "format case {index} failed for {source}");

            let reparsed = self.reference(&actual);
            assert_eq!(reparsed, concept, "formatted output did not round-trip for case {index}: {source} => {actual}");
        }
    }

    #[track_caller]
    pub fn assert_invalid<'a>(&mut self, sources: impl IntoIterator<Item = &'a str>) {
        for (index, source) in sources.into_iter().enumerate() {
            match self.pangine.reference_concept(source) {
                Err(ParseError::InvalidSyntax) => {}
                Err(error) => panic!("invalid-syntax case {index} returned the wrong error for {source:?}: {error}"),
                Ok(concept) => panic!("invalid-syntax case {index} unexpectedly parsed {source:?} as {}", self.format(concept.as_ref())),
            }
        }
    }

    #[track_caller]
    pub fn assert_null<'a>(&mut self, sources: impl IntoIterator<Item = &'a str>) {
        for (index, source) in sources.into_iter().enumerate() {
            let concept = self.reference(source);
            assert_eq!(concept, None, "null case {index} unexpectedly produced {} for {source:?}", self.format(concept.as_ref()));
        }
    }

    #[track_caller]
    pub fn assert_script_results<'a>(cases: impl IntoIterator<Item = (&'a str, &'a str)>) {
        for (index, (script, expected_source)) in cases.into_iter().enumerate() {
            let mut test = Self::new();
            let result = test.pangine.parse_script_text(script).unwrap_or_else(|error| panic!("script case {index} failed to parse: {error}"));
            let expected = test.reference(expected_source);
            assert_eq!(result, expected, "script case {index} returned {} instead of {}", test.format(result.as_ref()), test.format(expected.as_ref()));
        }
    }

    #[track_caller]
    pub fn assert_relevance<'a>(&mut self, cases: impl IntoIterator<Item = (&'a str, Relevance)>) {
        for (index, (source, expected)) in cases.into_iter().enumerate() {
            let concept = self.concept(source);
            let map = self.pangine.get_relevance_map(&concept);
            assert_eq!(map.len(), 1, "relevance case {index} expected one entry for {source}, got {map:?}");

            let actual = map[0].0;
            assert!(
                (actual.probability - expected.probability).abs() < 0.0001 && (actual.strength - expected.strength).abs() < 0.0001,
                "relevance case {index} failed for {source}: expected {expected:?}, got {actual:?}"
            );
        }
    }

    fn format(&self, concept: Option<&ConceptId>) -> String {
        concept.map_or_else(|| "[]".to_owned(), |concept| self.pangine.format_concept(concept, false))
    }
}
