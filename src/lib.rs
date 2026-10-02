#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Deterministic compositional grammar and semantic state engine.

mod engine;
mod relevance;

pub use engine::{
    Answer, AnswerChoice, AnswerPossibility, AnswerSource, AnswerSupport, AnswerView, Completion, CompletionEvidence, CompletionGrade, CompletionRemainder,
    CompletionRemainderSide, CompletionResult, ConceptConstructionError, ConceptId, ConceptKind, Pangine, ParseError, ParseResult, PerceptUpdateError,
    Probability, GLOBAL_PERCEPT_NAME,
};
pub use relevance::Relevance;
