#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! An experimental language for writing experience as simple shapes and
//! asking questions of it. Pangine answers exactly from what it remembers, or
//! with graded answers composed from parts and generalized from similar cases,
//! and every answer keeps its sources.

mod engine;
mod relevance;

pub use engine::{
    Answer, AnswerChoice, AnswerPossibility, AnswerSource, AnswerSupport, AnswerView, Completion, CompletionEvidence, CompletionGrade, CompletionRemainder,
    CompletionRemainderSide, CompletionResult, ConceptConstructionError, ConceptId, ConceptKind, Pangine, ParseError, ParseResult, PerceptUpdateError,
    Probability, GLOBAL_PERCEPT_NAME,
};
pub use relevance::Relevance;
