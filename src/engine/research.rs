//! Test-only research that needs access to engine internals.
//!
//! These modules are compiled only by the unit-test build. Passing a research
//! oracle makes an experiment reproducible; it does not promote the experiment
//! into Pangine's API or accepted semantic contract.

mod canonical_match_store;
mod question_candidate_index;
mod question_source_factorization;
mod unified_state_oracle;
