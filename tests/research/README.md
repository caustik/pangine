# Research tests

This directory contains bounded experiments and characterization oracles. They run in the ordinary test suite so their results remain reproducible, but they do not automatically define Pangine's accepted semantics.

A research test may intentionally capture a known limitation, compare unapproved representations, or use illustrative model constants. Passing means that the experiment still produces its documented result. Promotion into ordinary regression coverage requires a separate semantic decision and removal of the research qualification.

`unified_concept_state.rs` exercises current API behavior around direct root boundaries, structural repetition, finite induction crossover, and generic-match swamping. The corresponding test-only internal prototype lives under `src/engine/research/` because it needs access to matcher internals. Its question-support experiments verify that disjoint source-keyed partitions reduce to the same canonical support Concept as combined state. They also distinguish literal candidate support from a generic three-way path-mass tie and verify that unequal nested policy updates both remain available until correction semantics are explicitly defined, while keeping the proposed numeric evaluator provisional.

The research entrypoint also retains a compact counterexample showing that signed `Relevance::add` is not associative and therefore cannot serve as a partition-independent coefficient.
