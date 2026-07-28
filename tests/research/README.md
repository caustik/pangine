# Research tests

This directory contains bounded experiments and characterization oracles. They run in the ordinary test suite so their results remain reproducible, but they do not automatically define Pangine's accepted semantics.

A research test may intentionally capture a known limitation, compare unapproved representations, or use illustrative model constants. Passing means that the experiment still produces its documented result. Promotion into ordinary regression coverage requires a separate semantic decision and removal of the research qualification.

`unified_concept_state.rs` exercises current API behavior around direct root boundaries, structural repetition, finite induction crossover, and generic-match swamping. The corresponding test-only internal prototype lives under `src/engine/research/` because it needs access to matcher internals. Its question-support experiments verify that disjoint source-keyed partitions reduce to the same canonical support Concept as combined state. They also distinguish literal candidate support from a generic three-way path-mass tie and verify that unequal nested policy updates both remain available until correction semantics are explicitly defined, while keeping the proposed numeric evaluator provisional.

`revision_state.rs` compares flat-record, tagged-delivery, and direct-root revision models using the exact recursive Observation closures produced by Pangine's current experience traversal. Direct roots preserve add-only behavior and shared recursive records better than flat tombstones, but the revision markers do not satisfy the stronger distributed question requirement. Independently derived active views do not reduce to the view of merged state, losing a revision partition can reactivate superseded information, and reversing a mistaken correction creates a cycle instead of undoing the link. These are research results, not production state or grammar.

The source, matched-context, candidate encoding and its idempotent reduction are structural findings. The finite crossover, equal-source treatment, background, concentration, specificity order, and predictive distributions are assumed evaluator policies. Named Concepts do not acquire numeric, ordinal, missing-value, reliability, independence, or domain meaning from their spelling, so these tests cannot establish those semantics.

The research entrypoint also retains a compact counterexample showing that signed `Relevance::add` is not associative and therefore cannot serve as a partition-independent coefficient.
