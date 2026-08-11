#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Deterministic compositional grammar and semantic state engine.

mod engine;
mod relevance;

pub use engine::{
    Completion, CompletionEvidence, CompletionRemainder, CompletionRemainderSide, CompletionResult, ConceptId, ConceptKind, Pangine, ParseError, ParseResult,
    GLOBAL_PERCEPT_NAME,
};
pub use relevance::Relevance;
