//! Entrypoint for API-level research tests.
//!
//! Keeping the experiment modules below `tests/research/` distinguishes them
//! from regression tests for accepted Pangine behavior while still exercising
//! them in the ordinary test suite.

#[path = "research/unified_concept_state.rs"]
mod unified_concept_state;

#[path = "research/classified_collections.rs"]
mod classified_collections;
