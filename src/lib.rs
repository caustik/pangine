#![forbid(unsafe_code)]

//! Deterministic compositional grammar and semantic state engine.

mod engine;
mod relevance;

pub use engine::{ConceptId, ConceptKind, Pangine, ParseError, ParseResult, GLOBAL_PERCEPT_NAME};
pub use relevance::Relevance;
