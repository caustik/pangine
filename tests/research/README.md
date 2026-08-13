# Pangine research warning checks

These ignored checks preserve unresolved comparisons without making their exact outputs permanent Pangine behavior.

Run them explicitly with:

```sh
cargo test --test research --release -- --ignored
cargo test --lib source_state_copy --release -- --ignored
```

The second command runs internal engine probes. They copy direct Percept source state in test code only; they do not add public snapshot syntax.

Research programs that compare independent views copy a `$` result into a detached Percept before using `^`. Directly choosing a question output now conditions every output linked to that question.

## File index

- `application_choice.rs` compares two application-side rules with Pangine's current additive result. The rules can reject a larger total or abstain, but the fixtures do not establish that the application should own decisions.
- `decision_contract.rs` compares addition, multiplication, rescaling, ties, and distinct source histories. It keeps the information a future decision contract may need without choosing one formula.
- `decision_fallback.rs` records the current positive filter and canonical tie rule behind `^`.
- `decision_record.rs` compares saved totals, complete rows, evaluated values, and unchanged source Percepts. Each preserves a different part of an old decision.
- `interface_percepts.rs` exercises complete Rust input groups, assigned-input experience capture, output delivery, and a queued later cycle. It adds no callback registry, event loop, or LLM adapter.
- `joint_answer_relevance.rs` keeps the current source-deduplication rule visible without treating additive integer support as the final Relevance model.
- `matcher_boundaries.rs` keeps four open matcher questions: ordered nesting, valid `@` subjects, unseen wholes assembled from partial experience, and enclosing-entry correlation.
- `question_support.rs` records how direct Percept-member weights currently reach output coefficients.
- `represented_choice.rs` is the detailed decision corpus. It covers experience-shaped amounts, replaceable state, represented context and stance, question order, provenance, live source references, and saved records without host-side scoring.
- `row_choice.rs` shows that collapsing complete rows into totals can discard information needed by some decisions.
- `src/engine/research/source_state_copy.rs` compares value copies, live references, direct source-state copies, and represented version scopes. The behavior is test-only and does not choose a public lifecycle.

Accepted behavior belongs in ordinary tests:

- `tests/completion_questions.rs` covers the current structural evaluator and correlated results.
- `tests/joint_answers.rs` covers visible shared answer shapes, answer extension, conditioning, subset choice, order effects, and answer-state detachment.
- `tests/percept_integration.rs` covers grouped input validation, assigned-input capture, stable experience, and the Rust-input-to-Pangine-output cycle.

The former completion-projection, symbolic-annotation, and represented-reduction frameworks were removed after their conclusions were summarized in `design/pangine-research.md`. They were test-local interpreters, not production Pangine behavior.

An ignored check failing after a deliberate experiment is a prompt to review the example. It is not automatic proof that the new behavior is wrong.
