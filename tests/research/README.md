# Research tests

This directory contains bounded experiments and characterization oracles. They run in the ordinary test suite so their results remain reproducible, but they do not automatically define Pangine's accepted semantics.

A research test may intentionally capture a known limitation, compare unapproved representations, or use illustrative model constants. Passing means that the experiment still produces its documented result. Promotion into ordinary regression coverage requires a separate semantic decision and removal of the research qualification.

`unified_concept_state.rs` exercises current API behavior around direct root boundaries, repeated observations, finite induction crossover, and generic-match swamping. The corresponding test-only internal prototype lives under `src/engine/research/` because it needs access to matcher internals. That prototype also verifies that disjoint source-keyed partitions reduce to the same canonical support Concept as combined state, while characterizing duplicate partial delivery as unresolved structural multiplicity.
