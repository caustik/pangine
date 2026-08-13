#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Deterministic compositional grammar and semantic state engine.

mod engine;
mod relevance;

pub use engine::{
    Completion, CompletionBindingOrigin, CompletionEvidence, CompletionOrderedStep, CompletionOrderedWindow, CompletionRemainder, CompletionRemainderSide,
    CompletionResult, CompletionRoute, ConceptConstructionError, ConceptId, ConceptKind, Pangine, ParseError, ParseResult, PerceptUpdateError,
    GLOBAL_PERCEPT_NAME,
};
pub use relevance::Relevance;
