# Pangine research warning checks

These ignored checks preserve a few unresolved behaviors without making their exact outputs permanent Pangine semantics.

Run them explicitly with:

```sh
cargo test --test research --release -- --ignored
```

The files are intentionally small:

- `matcher_boundaries.rs` keeps four open matcher questions: ordered nesting, valid `@` subjects, unseen wholes assembled from partial experience, and the unresolved correlation between an exact enclosing ordered entry and one nested descendant.
- `question_support.rs` records how direct Percept-member weight is currently projected into output coefficients.
- `decision_fallback.rs` records the placeholder positive filter and canonical tie rule behind `^`.

Accepted current completion capabilities belong in the ordinary suite at `pangine/tests/completion_questions.rs`. That file covers explicit multi-relation questions, Rule 110, residuals, correlated rows, coefficient-bearing sources, grouped entries, occurrence-aware ordered paths, result closure, and separate Bayesian-shaped evidence factors.

The former completion-projection, symbolic-annotation, and represented-reduction frameworks were removed after their durable conclusions were summarized in `design/pangine-research.md`. They were large test-local interpreters, not production Pangine behavior.

An ignored check failing after a deliberate experiment is a prompt to review the example. It is not automatic proof that the new behavior is wrong.
